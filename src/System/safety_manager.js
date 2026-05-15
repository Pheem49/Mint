const fs = require('fs');
const os = require('os');
const path = require('path');

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

function classifyAction(action = {}) {
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

function assertActionAllowed(action, options = {}) {
    const classification = classifyAction(action);
    const allowDangerous = options.allowDangerous === true;

    if (classification.tier === TIERS.BLOCKED) {
        throw new Error(`Blocked action (${classification.reason}): ${action.type}`);
    }

    if (classification.tier === TIERS.DANGEROUS && !allowDangerous) {
        throw new Error(`Dangerous action requires explicit permission (${classification.reason}): ${action.type}`);
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

function appendActionLog(entry, options = {}) {
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

module.exports = {
    TIERS,
    classifyShellCommand,
    assertShellCommandAllowed,
    classifyAction,
    assertActionAllowed,
    resolveWithinRoot,
    appendActionLog
};
