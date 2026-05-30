import * as fs from 'fs'
import * as os from 'os'
import * as path from 'path'
import { readConfig  } from './config_manager'

const TIERS = Object.freeze({
    SAFE: 'safe',
    APPROVAL: 'approval',
    DANGEROUS: 'dangerous',
    BLOCKED: 'blocked'
});

const BLOCKED_COMMAND_PATTERNS = [
    { pattern: /\brm\s+(-[^\s]*r[^\s]*f|-rf|-fr)\b/, reason: 'recursive force delete' },
    { pattern: /\bgit\s+reset\s+--hard\b/, reason: 'destructive git reset' },
    { pattern: /\bgit\s+checkout\s+--\b/, reason: 'destructive git checkout path restore' },
    { pattern: /\bgit\s+clean\b.*\s-[^\s]*f/, reason: 'destructive git clean' },
    { pattern: /\bmkfs(?:\.\w+)?\b/, reason: 'filesystem formatting' },
    { pattern: /\bdd\s+.*\bof=\/dev\//, reason: 'raw disk write' },
    { pattern: />\s*\/dev\/(?:sd|nvme|hd|mapper)/, reason: 'write redirection to block device' },
    { pattern: /\b(shutdown|reboot|poweroff|halt)\b/, reason: 'system power command' },
    { pattern: /\bsudo\b/, reason: 'privilege escalation' },
    { pattern: /\bchmod\s+-R\s+777\b/, reason: 'unsafe recursive permissions' },
    { pattern: /\bchown\s+-R\b/, reason: 'unsafe recursive ownership change' },
    { pattern: /\bcurl\b.*\|\s*(sh|bash|zsh)\b/, reason: 'remote script piping' },
    { pattern: /\bwget\b.*\|\s*(sh|bash|zsh)\b/, reason: 'remote script piping' }
];

const DANGEROUS_ACTIONS = new Set([
    'delete_file',
    'system_automation'
]);

const SAFE_ACTIONS = new Set([
    'open_url',
    'search',
    'open_app',
    'open_file',
    'open_folder',
    'find_path',
    'clipboard_write',
    'learn_file',
    'learn_folder',
    'system_info',
    'mcp_tool',
    'mouse_move',
    'mouse_click',
    'type_text',
    'key_tap',
    'plugin',
    'web_automation',
    'create_folder'
]);

const DANGEROUS_SYSTEM_COMMANDS = new Set(['shutdown', 'restart', 'reboot', 'poweroff', 'sleep']);

function normalizeCommand(command) {
    return String(command || '').replace(/\s+/g, ' ').trim();
}

function expandHome(targetPath) {
    const value = String(targetPath || '');
    if (value === '~') return os.homedir();
    if (value.startsWith('~/')) return path.join(os.homedir(), value.slice(2));
    return value;
}

function normalizeRootList(paths) {
    return (Array.isArray(paths) ? paths : [])
        .filter(Boolean)
        .map((entry) => path.resolve(expandHome(entry)));
}

function getPolicy(config = readConfig()) {
    const enabled = config.safetyEnabled !== false;
    const fallbackRead = [
        os.homedir(),
        process.cwd(),
        path.join(os.homedir(), 'Desktop'),
        path.join(os.homedir(), 'Documents'),
        path.join(os.homedir(), 'Downloads'),
        path.join(os.homedir(), 'Pictures'),
        path.join(os.homedir(), 'Music'),
        path.join(os.homedir(), 'Videos')
    ];
    const fallbackWrite = [
        os.homedir(),
        process.cwd(),
        path.join(os.homedir(), 'Desktop'),
        path.join(os.homedir(), 'Documents'),
        path.join(os.homedir(), 'Downloads'),
        path.join(os.homedir(), 'Pictures'),
        path.join(os.homedir(), 'Music'),
        path.join(os.homedir(), 'Videos')
    ];

    return {
        enabled,
        sandboxMode: ['off', 'prefer', 'enforce'].includes(config.sandboxMode) ? config.sandboxMode : 'prefer',
        sandboxCommand: config.sandboxCommand || (process.platform === 'darwin' ? 'sandbox-exec' : process.platform === 'linux' ? 'bwrap' : ''),
        allowedReadPaths: normalizeRootList(config.allowedReadPaths && config.allowedReadPaths.length ? config.allowedReadPaths : fallbackRead),
        allowedWritePaths: normalizeRootList(config.allowedWritePaths && config.allowedWritePaths.length ? config.allowedWritePaths : fallbackWrite),
        blockedPaths: normalizeRootList(config.blockedPaths || []),
        blockedFileNames: new Set(Array.isArray(config.blockedFileNames) ? config.blockedFileNames : ['.env', 'id_rsa', 'id_ed25519'])
    };
}

function isPathWithin(root, targetPath) {
    const relative = path.relative(root, targetPath);
    return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function resolveCapabilityPath(targetPath, options: any = {}) {
    if (!targetPath) throw new Error('Target path is required.');

    const expanded = expandHome(targetPath);
    if (path.isAbsolute(expanded)) return path.resolve(expanded);

    const firstPart = String(expanded).split(/[/\\]/)[0];
    const commonHomeFolders = new Set(['Desktop', 'Documents', 'Downloads', 'Pictures', 'Music', 'Videos']);
    if (commonHomeFolders.has(firstPart)) {
        return path.resolve(os.homedir(), expanded);
    }

    const base = options.defaultBase || process.cwd();
    return path.resolve(base, expanded);
}

function assertPathCapability(targetPath, capability = 'read', options: any = {}) {
    const policy = getPolicy(options.config);
    const resolved = resolveCapabilityPath(targetPath, options);

    if (!policy.enabled) return resolved;

    if (policy.blockedFileNames.has(path.basename(resolved))) {
        throw new Error(`Blocked ${capability} access to sensitive file name: ${path.basename(resolved)}`);
    }

    const blockedRoot = policy.blockedPaths.find((root) => isPathWithin(root, resolved));
    if (blockedRoot) {
        throw new Error(`Blocked ${capability} access to protected path: ${resolved}`);
    }

    const allowedRoots = capability === 'write' ? policy.allowedWritePaths : policy.allowedReadPaths;
    const allowed = allowedRoots.some((root) => isPathWithin(root, resolved));
    if (!allowed) {
        throw new Error(`Path ${capability} denied by capability policy: ${resolved}`);
    }

    return resolved;
}

function getAllowedRoots(capability = 'read', options: any = {}) {
    const policy = getPolicy(options.config);
    return capability === 'write' ? policy.allowedWritePaths : policy.allowedReadPaths;
}

function classifyShellCommand(command) {
    const normalized = normalizeCommand(command);
    if (!normalized) {
        return { tier: TIERS.BLOCKED, reason: 'empty shell command' };
    }

    for (const rule of BLOCKED_COMMAND_PATTERNS) {
        if (rule.pattern.test(normalized)) {
            return { tier: TIERS.BLOCKED, reason: rule.reason };
        }
    }

    return { tier: TIERS.APPROVAL, reason: 'shell command requires approval' };
}

function assertShellCommandAllowed(command) {
    const result = classifyShellCommand(command);
    if (result.tier === TIERS.BLOCKED) {
        throw new Error(`Blocked unsafe command (${result.reason}): ${command}`);
    }
    return result;
}

function classifyAction(action: any = {}) {
    const type = action.type || 'none';
    if (type === 'none') return { tier: TIERS.SAFE, reason: 'no-op action' };

    if (type === 'system_automation') {
        const command = String(action.target || '').split(':')[0];
        if (DANGEROUS_SYSTEM_COMMANDS.has(command)) {
            return { tier: TIERS.DANGEROUS, reason: `system automation command '${command}'` };
        }
        return { tier: TIERS.APPROVAL, reason: 'system automation requires approval' };
    }

    if (DANGEROUS_ACTIONS.has(type)) {
        return { tier: TIERS.DANGEROUS, reason: `${type} can affect user data or system state` };
    }

    if (SAFE_ACTIONS.has(type)) {
        return { tier: TIERS.SAFE, reason: 'allowed action' };
    }

    return { tier: TIERS.APPROVAL, reason: 'unknown action requires approval' };
}

function assertActionAllowed(action, options: any = {}) {
    const classification: any = classifyAction(action);
    const allowDangerous = options.allowDangerous === true;
    const allowApproval = options.allowApproval === true;

    if (classification.tier === TIERS.BLOCKED) {
        throw new Error(`Blocked action (${classification.reason}): ${action.type}`);
    }

    if (classification.tier === TIERS.DANGEROUS && !allowDangerous) {
        throw new Error(`Dangerous action requires explicit permission (${classification.reason}): ${action.type}`);
    }

    if (classification.tier === TIERS.APPROVAL && !allowApproval) {
        throw new Error(`Action requires approval (${classification.reason}): ${action.type}`);
    }

    return classification;
}

function resolveWithinRoot(root, targetPath) {
    if (!root) throw new Error('Root path is required.');
    if (!targetPath) throw new Error('Target path is required.');

    const resolvedRoot = path.resolve(root);
    const expandedTarget = String(targetPath).startsWith('~/')
        ? path.join(os.homedir(), String(targetPath).slice(2))
        : targetPath;
    const resolvedTarget = path.resolve(resolvedRoot, expandedTarget);
    const relative = path.relative(resolvedRoot, resolvedTarget);

    if (relative.startsWith('..') || path.isAbsolute(relative)) {
        throw new Error(`Path is outside allowed root: ${targetPath}`);
    }

    return resolvedTarget;
}

function appendActionLog(entry, options: any = {}) {
    const logPath = options.logPath || path.join(os.homedir(), '.config', 'mint', 'action-log.jsonl');
    const payload = {
        time: new Date().toISOString(),
        ...entry
    };

    try {
        fs.mkdirSync(path.dirname(logPath), { recursive: true });
        fs.appendFileSync(logPath, `${JSON.stringify(payload)}\n`, 'utf8');
    } catch (error) {
        if (process.env.MINT_DEBUG === '1') {
            console.error('[Safety] Failed to append action log:', error.message);
        }
    }

    return payload;
}

export { TIERS,
    getPolicy,
    getAllowedRoots,
    classifyShellCommand,
    assertShellCommandAllowed,
    classifyAction,
    assertActionAllowed,
    assertPathCapability,
    resolveCapabilityPath,
    resolveWithinRoot,
    appendActionLog
 }
