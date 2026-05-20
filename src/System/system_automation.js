const { execFile } = require('child_process');
const fs = require('fs');

function execPromise(command, args = []) {
    return new Promise((resolve, reject) => {
        execFile(command, args, (error, stdout, stderr) => {
            if (error) {
                error.stderr = stderr;
                reject(error);
                return;
            }
            resolve(String(stdout || '').trim());
        });
    });
}

function unsupported(feature) {
    throw new Error(`${feature} is not supported on ${process.platform} by the current automation provider.`);
}

function clampPercent(percent) {
    const value = Number(percent);
    if (!Number.isFinite(value)) return 50;
    return Math.max(0, Math.min(100, Math.round(value)));
}

const linuxProvider = {
    async setVolume(percent) {
        const value = clampPercent(percent);
        try {
            await execPromise('amixer', ['-D', 'pulse', 'sset', 'Master', `${value}%`]);
            return `Volume set to ${value}%`;
        } catch (_) {
            await execPromise('pactl', ['set-sink-volume', '@DEFAULT_SINK@', `${value}%`]);
            return `Volume set to ${value}%`;
        }
    },
    async mute() {
        try {
            await execPromise('amixer', ['-D', 'pulse', 'sset', 'Master', 'toggle']);
            return 'Volume toggled (mute/unmute)';
        } catch (_) {
            await execPromise('pactl', ['set-sink-mute', '@DEFAULT_SINK@', 'toggle']);
            return 'Volume toggled (mute/unmute)';
        }
    },
    async setBrightness(percent) {
        const value = clampPercent(percent);
        try {
            await execPromise('brightnessctl', ['set', `${value}%`]);
            return `Brightness set to ${value}%`;
        } catch (_) {
            await execPromise('xbacklight', ['-set', String(value)]);
            return `Brightness set to ${value}%`;
        }
    },
    sleep: () => execPromise('systemctl', ['suspend']),
    restart: () => execPromise('systemctl', ['reboot']),
    shutdown: () => execPromise('systemctl', ['poweroff']),
    async minimizeAll() {
        await execPromise('xdotool', ['key', 'Super+d']);
        return 'Minimized all windows';
    },
    async getSystemInfo() {
        try {
            const osInfo = await execPromise('lsb_release', ['-ds']);
            const kernel = await execPromise('uname', ['-r']);
            const arch = await execPromise('uname', ['-m']);
            return `Operating System: ${osInfo}\nKernel: ${kernel}\nArchitecture: ${arch}`;
        } catch (_) {
            const osRelease = fs.existsSync('/etc/os-release') ? fs.readFileSync('/etc/os-release', 'utf8') : '';
            const prettyName = (osRelease.match(/^PRETTY_NAME="?([^"\n]+)"?/m) || [])[1] || 'Linux';
            const kernel = await execPromise('uname', ['-r']);
            const arch = await execPromise('uname', ['-m']);
            return `Operating System: ${prettyName}\nKernel: ${kernel}\nArchitecture: ${arch}`;
        }
    }
};

const macProvider = {
    async setVolume(percent) {
        const value = clampPercent(percent);
        await execPromise('osascript', ['-e', `set volume output volume ${value}`]);
        return `Volume set to ${value}%`;
    },
    async mute() {
        await execPromise('osascript', ['-e', 'set volume output muted not (output muted of (get volume settings))']);
        return 'Volume toggled (mute/unmute)';
    },
    setBrightness: () => unsupported('Brightness control'),
    sleep: () => execPromise('osascript', ['-e', 'tell application "System Events" to sleep']),
    restart: () => execPromise('osascript', ['-e', 'tell application "System Events" to restart']),
    shutdown: () => execPromise('osascript', ['-e', 'tell application "System Events" to shut down']),
    async minimizeAll() {
        await execPromise('osascript', ['-e', 'tell application "System Events" to keystroke "h" using {command down, option down}']);
        return 'Hid visible applications';
    },
    async getSystemInfo() {
        const product = await execPromise('sw_vers', ['-productName']);
        const version = await execPromise('sw_vers', ['-productVersion']);
        const build = await execPromise('sw_vers', ['-buildVersion']);
        const arch = await execPromise('uname', ['-m']);
        return `Operating System: ${product} ${version} (${build})\nArchitecture: ${arch}`;
    }
};

const windowsProvider = {
    setVolume: () => unsupported('Volume control'),
    mute: () => unsupported('Mute control'),
    setBrightness: () => unsupported('Brightness control'),
    sleep: () => execPromise('rundll32.exe', ['powrprof.dll,SetSuspendState', '0,1,0']),
    restart: () => execPromise('shutdown.exe', ['/r', '/t', '0']),
    shutdown: () => execPromise('shutdown.exe', ['/s', '/t', '0']),
    async minimizeAll() {
        await execPromise('powershell.exe', [
            '-NoProfile',
            '-NonInteractive',
            '-Command',
            '$shell = New-Object -ComObject Shell.Application; $shell.MinimizeAll()'
        ]);
        return 'Minimized all windows';
    },
    async getSystemInfo() {
        const caption = await execPromise('powershell.exe', [
            '-NoProfile',
            '-NonInteractive',
            '-Command',
            '(Get-CimInstance Win32_OperatingSystem).Caption'
        ]);
        const version = await execPromise('cmd.exe', ['/c', 'ver']);
        const arch = process.arch;
        return `Operating System: ${caption}\nVersion: ${version}\nArchitecture: ${arch}`;
    }
};

function getProvider(platform = process.platform) {
    if (platform === 'darwin') return macProvider;
    if (platform === 'win32') return windowsProvider;
    return linuxProvider;
}

const SystemAutomation = {
    setVolume: (percent) => getProvider().setVolume(percent),
    mute: () => getProvider().mute(),
    setBrightness: (percent) => getProvider().setBrightness(percent),
    sleep: () => getProvider().sleep(),
    restart: () => getProvider().restart(),
    shutdown: () => getProvider().shutdown(),
    minimizeAll: () => getProvider().minimizeAll(),
    async getSystemInfo(target = '') {
        if (!target) return getProvider().getSystemInfo();
        return `System info for ${target} is not yet implemented.`;
    },
    _providers: {
        linux: linuxProvider,
        darwin: macProvider,
        win32: windowsProvider,
        getProvider
    }
};

module.exports = SystemAutomation;
