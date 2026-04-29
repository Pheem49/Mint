const { exec } = require('child_process');
const { screen } = require('electron');

/**
 * GranularAutomation handles low-level OS input via xdotool.
 * It uses a normalized coordinate system (0-1000).
 */
class GranularAutomation {
    constructor() {
        this.screenWidth = 1920; // Default fallback
        this.screenHeight = 1080;
        this.updateScreenSize();
    }

    updateScreenSize() {
        try {
            // In Electron main process, we can use the screen module
            const primaryDisplay = screen.getPrimaryDisplay();
            if (primaryDisplay && primaryDisplay.size) {
                this.screenWidth = primaryDisplay.size.width;
                this.screenHeight = primaryDisplay.size.height;
                console.log(`[Automation] Screen detected: ${this.screenWidth}x${this.screenHeight}`);
            }
        } catch (e) {
            // Fallback for CLI or cases where screen module is unavailable
            exec('xdpyinfo | grep dimensions', (err, stdout) => {
                if (!err && stdout) {
                    const match = stdout.match(/(\d+)x(\d+) pixels/);
                    if (match) {
                        this.screenWidth = parseInt(match[1]);
                        this.screenHeight = parseInt(match[2]);
                        console.log(`[Automation] Screen detected via xdpyinfo: ${this.screenWidth}x${this.screenHeight}`);
                    }
                }
            });
        }
    }

    scaleX(x) {
        return Math.round((x / 1000) * this.screenWidth);
    }

    scaleY(y) {
        return Math.round((y / 1000) * this.screenHeight);
    }

    run(command) {
        return new Promise((resolve, reject) => {
            exec(command, (err, stdout, stderr) => {
                if (err) {
                    console.error(`[Automation] xdotool error: ${stderr}`);
                    reject(err);
                } else {
                    resolve(stdout);
                }
            });
        });
    }

    async mouseMove(x, y) {
        const sx = this.scaleX(x);
        const sy = this.scaleY(y);
        console.log(`[Automation] Moving mouse to ${sx}, ${sy}`);
        return this.run(`xdotool mousemove ${sx} ${sy}`);
    }

    async mouseClick(x, y, button = 1) {
        const sx = this.scaleX(x);
        const sy = this.scaleY(y);
        console.log(`[Automation] Clicking ${button} at ${sx}, ${sy}`);
        // move first then click to be safe
        return this.run(`xdotool mousemove ${sx} ${sy} click ${button}`);
    }

    async typeText(text) {
        console.log(`[Automation] Typing: ${text}`);
        // Escape double quotes for shell
        const escaped = text.replace(/"/g, '\\"');
        return this.run(`xdotool type "${escaped}"`);
    }

    async keyTap(key) {
        console.log(`[Automation] Key tap: ${key}`);
        return this.run(`xdotool key "${key}"`);
    }
}

module.exports = new GranularAutomation();
