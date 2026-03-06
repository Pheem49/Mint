const { exec } = require('child_process');

function openApp(target) {
    if (!target) return;

    let cmd = '';
    if (process.platform === 'win32') {
        cmd = `start "" "${target}"`;
    } else if (process.platform === 'darwin') {
        if (!target.includes('/')) {
            cmd = `open -X -a "${target}" || open -a "${target}"`;
        } else {
            cmd = `open "${target}"`;
        }
    } else {
        cmd = `xdg-open "${target}"`;
        if (!target.includes('/')) {
            cmd = `gtk-launch ${target} || ${target}`;
        }
    }

    exec(cmd, (error) => {
        if (error) {
            console.error(`exec error: ${error}`);
            if (process.platform !== 'win32') {
                exec(target, (err2) => {
                    if (err2) console.error("Fallback exec failed:", err2);
                });
            }
        }
    });
}

module.exports = { openApp };
