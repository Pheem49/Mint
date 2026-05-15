const { execFile } = require('child_process');
const pkg = require('../../package.json');

const NPM_COMMAND = process.platform === 'win32' ? 'npm.cmd' : 'npm';
const DEFAULT_AUTO_UPDATE_INTERVAL_HOURS = 24;

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

function parseVersion(version) {
    return String(version || '')
        .trim()
        .replace(/^v/, '')
        .split('-')[0]
        .split('.')
        .map((part) => Number.parseInt(part, 10) || 0);
}

function compareVersions(a, b) {
    const left = parseVersion(a);
    const right = parseVersion(b);
    const length = Math.max(left.length, right.length, 3);

    for (let i = 0; i < length; i++) {
        const l = left[i] || 0;
        const r = right[i] || 0;
        if (l > r) return 1;
        if (l < r) return -1;
    }
    return 0;
}

function normalizeNpmVersionOutput(output) {
    const trimmed = String(output || '').trim();
    if (!trimmed) return '';

    try {
        const parsed = JSON.parse(trimmed);
        if (typeof parsed === 'string') return parsed;
    } catch (err) {
        // npm may return plain text depending on config/version.
    }

    return trimmed.replace(/^['"]|['"]$/g, '');
}

function getAutoUpdateIntervalMs(config = {}) {
    const hours = Number(config.autoUpdateCheckIntervalHours);
    const safeHours = Number.isFinite(hours) && hours > 0
        ? hours
        : DEFAULT_AUTO_UPDATE_INTERVAL_HOURS;
    return safeHours * 60 * 60 * 1000;
}

function shouldRunAutoUpdate(config = {}, now = Date.now()) {
    if (config.enableAutoUpdate === false) return false;

    const lastCheck = Date.parse(config.lastUpdateCheckAt || '');
    if (!Number.isFinite(lastCheck)) return true;

    return now - lastCheck >= getAutoUpdateIntervalMs(config);
}

async function getLatestVersion(packageName = pkg.name) {
    const { stdout } = await execFilePromise(NPM_COMMAND, ['view', packageName, 'version', '--json'], {
        maxBuffer: 1024 * 1024
    });
    return normalizeNpmVersionOutput(stdout);
}

async function installLatest(packageName = pkg.name, options = {}) {
    const args = ['install', '-g', `${packageName}@latest`];
    if (options.dryRun) {
        args.push('--dry-run');
    }

    return await execFilePromise(NPM_COMMAND, args, {
        maxBuffer: 1024 * 1024 * 8
    });
}

function formatUpdateError(error) {
    const detail = [error.stderr, error.stdout, error.message].filter(Boolean).join('\n').trim();
    if (/EACCES|permission denied|Access is denied/i.test(detail)) {
        return [
            'Update failed because npm does not have permission to modify the global install directory.',
            `Run manually: npm install -g ${pkg.name}@latest`,
            'If your npm global packages require sudo, run that command with sudo.'
        ].join('\n');
    }

    if (/E404|404 Not Found|not in this registry/i.test(detail)) {
        return [
            `Could not find ${pkg.name} on the npm registry.`,
            'Publish the package first, or update Mint from the source/release channel you installed from.'
        ].join('\n');
    }

    return `Update failed: ${detail || 'Unknown npm error'}`;
}

async function runUpdate(options = {}) {
    const currentVersion = pkg.version;
    let latestVersion = '';

    try {
        latestVersion = await getLatestVersion(pkg.name);
    } catch (error) {
        return {
            status: 'error',
            currentVersion,
            latestVersion,
            message: formatUpdateError(error)
        };
    }

    if (!latestVersion) {
        return {
            status: 'error',
            currentVersion,
            latestVersion: '',
            message: 'Could not determine the latest Mint version from npm.'
        };
    }

    const comparison = compareVersions(currentVersion, latestVersion);
    if (comparison >= 0) {
        return {
            status: 'current',
            currentVersion,
            latestVersion,
            message: `Mint is already up to date (${currentVersion}).`
        };
    }

    if (options.checkOnly) {
        return {
            status: 'available',
            currentVersion,
            latestVersion,
            message: `Mint ${latestVersion} is available. Current version: ${currentVersion}.`
        };
    }

    try {
        await installLatest(pkg.name, { dryRun: options.dryRun });
        return {
            status: options.dryRun ? 'dry-run' : 'updated',
            currentVersion,
            latestVersion,
            message: options.dryRun
                ? `Dry run complete. Mint would update from ${currentVersion} to ${latestVersion}.`
                : `Mint updated from ${currentVersion} to ${latestVersion}. Restart mint to use the new version.`
        };
    } catch (error) {
        return {
            status: 'error',
            currentVersion,
            latestVersion,
            message: formatUpdateError(error)
        };
    }
}

async function runStartupAutoUpdate(config, writeConfig, options = {}) {
    const now = options.now || Date.now();
    if (!shouldRunAutoUpdate(config, now)) {
        return {
            status: 'skipped',
            message: 'Auto-update check skipped by cooldown.'
        };
    }

    if (typeof writeConfig === 'function') {
        writeConfig({
            ...config,
            lastUpdateCheckAt: new Date(now).toISOString()
        });
    }

    return await runUpdate({ checkOnly: false });
}

module.exports = {
    compareVersions,
    getLatestVersion,
    installLatest,
    normalizeNpmVersionOutput,
    runUpdate,
    runStartupAutoUpdate,
    shouldRunAutoUpdate,
    _private: {
        parseVersion,
        formatUpdateError,
        getAutoUpdateIntervalMs
    }
};
