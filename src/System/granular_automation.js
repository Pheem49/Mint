const { execFile, spawnSync } = require('child_process');
const { screen } = require('electron');

function commandExists(command) {
    const lookup = process.platform === 'win32' ? 'where' : 'which';
    const result = spawnSync(lookup, [command], { encoding: 'utf8', shell: false });
    return result.status === 0;
}

function run(command, args = []) {
    return new Promise((resolve, reject) => {
        execFile(command, args, (err, stdout, stderr) => {
            if (err) {
                err.stderr = stderr;
                reject(err);
                return;
            }
            resolve(stdout);
        });
    });
}

function unsupported(feature) {
    throw new Error(`${feature} is not supported on ${process.platform} by the current input automation provider.`);
}

function escapePowerShellSingleQuoted(value) {
    return String(value || '').replace(/'/g, "''");
}

function keyToMacKey(key) {
    const value = String(key || '').trim();
    const map = {
        Enter: 'return',
        Return: 'return',
        Escape: 'escape',
        Esc: 'escape',
        Space: 'space',
        Backspace: 'delete',
        Delete: 'forward delete',
        Tab: 'tab'
    };
    return map[value] || value;
}

class GranularAutomation {
    constructor() {
        this.screenWidth = 1920;
        this.screenHeight = 1080;
        this.updateScreenSize();
    }

    updateScreenSize() {
        try {
            const primaryDisplay = screen.getPrimaryDisplay();
            if (primaryDisplay && primaryDisplay.size) {
                this.screenWidth = primaryDisplay.size.width;
                this.screenHeight = primaryDisplay.size.height;
            }
        } catch (_) {
            // Electron screen can be unavailable in CLI-only contexts.
        }
    }

    scaleX(x) {
        return Math.round((Number(x) / 1000) * this.screenWidth);
    }

    scaleY(y) {
        return Math.round((Number(y) / 1000) * this.screenHeight);
    }

    provider() {
        if (process.platform === 'darwin') return macProvider;
        if (process.platform === 'win32') return windowsProvider;
        return linuxProvider;
    }

    mouseMove(x, y) {
        return this.provider().mouseMove(this.scaleX(x), this.scaleY(y));
    }

    mouseClick(x, y, button = 1) {
        return this.provider().mouseClick(this.scaleX(x), this.scaleY(y), button);
    }

    typeText(text) {
        return this.provider().typeText(String(text || ''));
    }

    keyTap(key) {
        return this.provider().keyTap(String(key || ''));
    }
}

const linuxProvider = {
    mouseMove: (x, y) => run('xdotool', ['mousemove', String(x), String(y)]),
    mouseClick: (x, y, button = 1) => run('xdotool', ['mousemove', String(x), String(y), 'click', String(button)]),
    typeText: (text) => run('xdotool', ['type', text]),
    keyTap: (key) => run('xdotool', ['key', key])
};

const macProvider = {
    mouseMove(x, y) {
        if (!commandExists('cliclick')) return unsupported('Mouse move');
        return run('cliclick', [`m:${x},${y}`]);
    },
    mouseClick(x, y) {
        if (!commandExists('cliclick')) return unsupported('Mouse click');
        return run('cliclick', [`c:${x},${y}`]);
    },
    typeText(text) {
        if (commandExists('cliclick')) return run('cliclick', [`t:${text}`]);
        return run('osascript', ['-e', `tell application "System Events" to keystroke ${JSON.stringify(text)}`]);
    },
    keyTap(key) {
        const macKey = keyToMacKey(key);
        if (commandExists('cliclick')) return run('cliclick', [`kp:${macKey}`]);
        if (macKey.length === 1) {
            return run('osascript', ['-e', `tell application "System Events" to keystroke ${JSON.stringify(macKey)}`]);
        }
        return unsupported('Special key tap without cliclick');
    }
};

const windowsProvider = {
    mouseMove(x, y) {
        const script = `[void][Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms'); [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(${x}, ${y})`;
        return run('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script]);
    },
    mouseClick(x, y, button = 1) {
        const down = Number(button) === 2 ? '0x0008' : '0x0002';
        const up = Number(button) === 2 ? '0x0010' : '0x0004';
        const script = [
            "Add-Type -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool SetCursorPos(int X,int Y); [DllImport(\"user32.dll\")] public static extern void mouse_event(int dwFlags,int dx,int dy,int dwData,int dwExtraInfo);' -Name NativeMouse -Namespace Mint;",
            `[Mint.NativeMouse]::SetCursorPos(${x}, ${y}) | Out-Null;`,
            `[Mint.NativeMouse]::mouse_event(${down},0,0,0,0);`,
            `[Mint.NativeMouse]::mouse_event(${up},0,0,0,0);`
        ].join(' ');
        return run('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script]);
    },
    typeText(text) {
        const safe = escapePowerShellSingleQuoted(text);
        const script = `Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('${safe}')`;
        return run('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script]);
    },
    keyTap(key) {
        const safe = escapePowerShellSingleQuoted(key);
        const script = `Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{${safe}}')`;
        return run('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script]);
    }
};

const instance = new GranularAutomation();
instance._providers = { linuxProvider, macProvider, windowsProvider, commandExists };

module.exports = instance;
