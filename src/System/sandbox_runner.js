const { execFile, spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const safetyManager = require('./safety_manager');

function commandExists(command) {
    const lookup = process.platform === 'win32' ? 'where' : 'which';
    const result = spawnSync(lookup, [command], { encoding: 'utf8', shell: false });
    return result.status === 0;
}

function uniqueExistingRoots(roots) {
    return Array.from(new Set((roots || [])
        .filter(Boolean)
        .map((root) => path.resolve(root))
        .filter((root) => fs.existsSync(root))));
}

function buildBubblewrapArgs(command, options = {}) {
    const cwd = path.resolve(options.cwd || process.cwd());
    const readRoots = uniqueExistingRoots(safetyManager.getAllowedRoots('read'));
    const writeRoots = uniqueExistingRoots(safetyManager.getAllowedRoots('write'));
    const writableSet = new Set(writeRoots);
    const bindRoots = uniqueExistingRoots([cwd, ...readRoots, ...writeRoots]);

    const args = [
        '--die-with-parent',
        '--proc', '/proc',
        '--dev', '/dev',
        '--tmpfs', '/tmp',
        '--ro-bind', '/usr', '/usr',
        '--ro-bind', '/bin', '/bin',
        '--ro-bind', '/etc', '/etc'
    ];

    for (const libPath of ['/lib', '/lib64']) {
        if (fs.existsSync(libPath)) args.push('--ro-bind', libPath, libPath);
    }

    const parentDirs = new Set();
    for (const root of bindRoots) {
        let current = path.dirname(root);
        while (current && current !== path.dirname(current)) {
            parentDirs.add(current);
            current = path.dirname(current);
        }
    }
    for (const dir of Array.from(parentDirs).sort((a, b) => a.length - b.length)) {
        if (!['/usr', '/bin', '/etc', '/lib', '/lib64'].includes(dir)) {
            args.push('--dir', dir);
        }
    }

    for (const root of bindRoots) {
        const flag = writableSet.has(root) || root === cwd ? '--bind' : '--ro-bind';
        args.push(flag, root, root);
    }

    args.push('--chdir', cwd, 'bash', '-lc', command);
    return args;
}

function escapeSandboxProfileString(value) {
    return String(value || '').replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

function buildMacSandboxProfile(options = {}) {
    const cwd = path.resolve(options.cwd || process.cwd());
    const readRoots = uniqueExistingRoots(safetyManager.getAllowedRoots('read'));
    const writeRoots = uniqueExistingRoots(safetyManager.getAllowedRoots('write'));
    const allowedRead = uniqueExistingRoots([
        cwd,
        '/bin',
        '/sbin',
        '/usr',
        '/System',
        '/Library',
        ...readRoots,
        ...writeRoots
    ]);
    const allowedWrite = uniqueExistingRoots([cwd, ...writeRoots]);

    const readRules = allowedRead.map((root) => `  (subpath "${escapeSandboxProfileString(root)}")`).join('\n');
    const writeRules = allowedWrite.map((root) => `  (subpath "${escapeSandboxProfileString(root)}")`).join('\n');

    return [
        '(version 1)',
        '(deny default)',
        '(allow process*)',
        '(allow sysctl-read)',
        '(allow signal (target self))',
        '(allow file-read-metadata)',
        '(allow file-read*',
        readRules,
        ')',
        '(allow file-write*',
        writeRules,
        `  (subpath "${escapeSandboxProfileString(os.tmpdir())}")`,
        ')'
    ].join('\n');
}

function getShellInvocation(command) {
    if (process.platform === 'win32') {
        return {
            command: 'powershell.exe',
            args: ['-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', command]
        };
    }
    return {
        command: 'bash',
        args: ['-lc', command]
    };
}

function execFilePromise(command, args, options = {}) {
    return new Promise((resolve, reject) => {
        execFile(command, args, options, (error, stdout, stderr) => {
            if (error) {
                error.stdout = stdout;
                error.stderr = stderr;
                reject(error);
                return;
            }
            resolve({ stdout, stderr });
        });
    });
}

async function runShell(command, options = {}) {
    const policy = safetyManager.getPolicy();
    safetyManager.assertShellCommandAllowed(command);

    const cwd = path.resolve(options.cwd || process.cwd());
    const execOptions = {
        cwd,
        maxBuffer: options.maxBuffer || 1024 * 1024 * 4,
        env: options.env || process.env
    };

    if (!policy.enabled || policy.sandboxMode === 'off') {
        const shell = getShellInvocation(command);
        return execFilePromise(shell.command, shell.args, execOptions);
    }

    const sandboxCommand = policy.sandboxCommand || 'bwrap';
    if (process.platform === 'linux' && commandExists(sandboxCommand)) {
        return execFilePromise(sandboxCommand, buildBubblewrapArgs(command, { cwd }), execOptions);
    }

    if (process.platform === 'darwin' && commandExists('sandbox-exec')) {
        return execFilePromise('sandbox-exec', ['-p', buildMacSandboxProfile({ cwd }), 'bash', '-lc', command], execOptions);
    }

    if (policy.sandboxMode === 'enforce') {
        const hint = process.platform === 'darwin'
            ? "macOS sandbox-exec is not available."
            : process.platform === 'win32'
                ? 'Windows sandbox provider is not configured. Use WSL/containers or set sandboxMode to prefer.'
                : `Sandbox command '${sandboxCommand}' is not available.`;
        throw new Error(`Sandbox is enforced but no sandbox provider could run. ${hint}`);
    }

    safetyManager.appendActionLog({
        source: options.source || 'sandbox_runner',
        action: 'sandbox_fallback',
        sandboxCommand,
        platform: process.platform,
        cwd
    });
    const shell = getShellInvocation(command);
    return execFilePromise(shell.command, shell.args, execOptions);
}

module.exports = {
    runShell,
    buildBubblewrapArgs,
    buildMacSandboxProfile,
    getShellInvocation,
    commandExists
};
