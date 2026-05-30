import { execFile  } from 'child_process'

function execFilePromise(command: string, args: string[]): Promise<void> {
    return new Promise<void>((resolve, reject) => {
        execFile(command, args, (error) => {
            if (error) {
                reject(error);
                return;
            }
            resolve();
        });
    });
}

async function tryCommands(commands) {
    let lastError = null;
    for (const { command, args } of commands) {
        try {
            await execFilePromise(command, args);
            return true;
        } catch (error) {
            lastError = error;
        }
    }

    if (lastError) {
        console.error(`exec error: ${lastError}`);
    }
    return false;
}

async function openApp(target) {
    if (!target) return;

    if (process.platform === 'win32') {
        await execFilePromise('cmd.exe', ['/c', 'start', '', target]).catch((error) => {
            console.error(`exec error: ${error}`);
        });
        return;
    }

    if (process.platform === 'darwin') {
        if (!target.includes('/')) {
            await tryCommands([
                { command: 'open', args: ['-X', '-a', target] },
                { command: 'open', args: ['-a', target] }
            ]);
        } else {
            await execFilePromise('open', [target]).catch((error) => {
                console.error(`exec error: ${error}`);
            });
        }
        return;
    }

    const tLower = target.toLowerCase();
    const tCapitalized = target.charAt(0).toUpperCase() + target.slice(1).toLowerCase();

    if (target.includes('/')) {
        await execFilePromise('xdg-open', [target]).catch((error) => {
            console.error(`exec error: ${error}`);
        });
        return;
    }

    await tryCommands([
        { command: 'gtk-launch', args: [target] },
        { command: 'gtk-launch', args: [tLower] },
        { command: 'gtk-launch', args: [tCapitalized] },
        { command: 'gtk-launch', args: [`com.${tLower}app.${tCapitalized}`] },
        { command: 'gtk-launch', args: [`com.${tLower}.${tCapitalized}`] },
        { command: target, args: [] },
        { command: tLower, args: [] },
        { command: 'flatpak', args: ['run', target] },
        { command: 'flatpak', args: ['run', `com.${tLower}app.${tCapitalized}`] },
        { command: 'flatpak', args: ['run', `com.${tLower}.${tCapitalized}`] },
        { command: 'flatpak', args: ['run', `com.${tLower}.Browser`] },
        { command: 'flatpak', args: ['run', `com.${tLower}.${target}`] },
        { command: 'flatpak', args: ['run', 'com.valvesoftware.Steam'] },
        { command: 'flatpak', args: ['run', 'net.lutris.Lutris'] },
        { command: 'snap', args: ['run', tLower] }
    ]);
}

export { openApp  }
