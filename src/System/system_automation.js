const { exec } = require('child_process');

/**
 * Executes a shell command and returns a promise.
 */
function execPromise(command) {
    return new Promise((resolve, reject) => {
        exec(command, (error, stdout, stderr) => {
            if (error) {
                reject(error);
                return;
            }
            resolve(stdout.trim());
        });
    });
}

/**
 * Linux System Automation Logic
 */
const SystemAutomation = {
    // Volume Control (Using amixer / pulseaudio)
    async setVolume(percent) {
        // Try amixer first (common on many distros)
        try {
            await execPromise(`amixer -D pulse sset Master ${percent}%`);
            return `Volume set to ${percent}%`;
        } catch (e) {
            try {
                await execPromise(`pactl set-sink-volume @DEFAULT_SINK@ ${percent}%`);
                return `Volume set to ${percent}%`;
            } catch (err) {
                throw new Error("Failed to set volume. amixer or pactl not found.");
            }
        }
    },

    async mute() {
        try {
            await execPromise(`amixer -D pulse sset Master toggle`);
            return "Volume toggled (mute/unmute)";
        } catch (e) {
            await execPromise(`pactl set-sink-mute @DEFAULT_SINK@ toggle`);
            return "Volume toggled (mute/unmute)";
        }
    },

    // Brightness Control (Using brightnessctl or xbacklight)
    async setBrightness(percent) {
        try {
            // brightnessctl is modern and common on Wayland/X11
            await execPromise(`brightnessctl set ${percent}%`);
            return `Brightness set to ${percent}%`;
        } catch (e) {
            try {
                await execPromise(`xbacklight -set ${percent}`);
                return `Brightness set to ${percent}%`;
            } catch (err) {
                throw new Error("Failed to set brightness. brightnessctl or xbacklight not found.");
            }
        }
    },

    // Power Management
    async sleep() {
        return execPromise('systemctl suspend');
    },

    async restart() {
        return execPromise('systemctl reboot');
    },

    async shutdown() {
        return execPromise('systemctl poweroff');
    },

    // Window Management (Minimal implementation using xdotool if available)
    async minimizeAll() {
        try {
            await execPromise('xdotool key Super+d');
            return "Minimized all windows";
        } catch (e) {
            throw new Error("xdotool not found. Cannot perform window management.");
        }
    },

    // System Information
    async getSystemInfo(target = "") {
        // If target is empty, return OS info
        if (!target) {
            try {
                // Try lsb_release first
                const osInfo = await execPromise('lsb_release -ds');
                const kernel = await execPromise('uname -r');
                const arch = await execPromise('uname -m');
                return `Operating System: ${osInfo}\nKernel: ${kernel}\nArchitecture: ${arch}`;
            } catch (e) {
                try {
                    // Fallback to /etc/os-release
                    const osInfo = await execPromise('grep PRETTY_NAME /etc/os-release | cut -d\'"\' -f2');
                    const kernel = await execPromise('uname -r');
                    const arch = await execPromise('uname -m');
                    return `Operating System: ${osInfo}\nKernel: ${kernel}\nArchitecture: ${arch}`;
                } catch (err) {
                    return "Could not retrieve OS information.";
                }
            }
        }
        
        // Handle weather or other info if target is provided
        // For now, let's just return a placeholder or handle it if needed
        return `System info for ${target} is not yet implemented.`;
    }
};

module.exports = SystemAutomation;
