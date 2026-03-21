const { exec } = require('child_process');
const EventEmitter = require('events');

class SystemEvents extends EventEmitter {
    constructor() {
        super();
        this.lastBatteryLevel = null;
        this.lastConnectionStatus = null;
        this.checkInterval = 60000; // 1 minute
        this.isMonitoring = false;
    }

    startMonitoring() {
        if (this.isMonitoring) return;
        this.isMonitoring = true;
        this.check();
        this.timer = setInterval(() => this.check(), this.checkInterval);
    }

    stopMonitoring() {
        if (this.timer) clearInterval(this.timer);
        this.isMonitoring = false;
    }

    async check() {
        await this.checkBattery();
        await this.checkNetwork();
    }

    async checkBattery() {
        try {
            // Linux: upower -i $(upower -e | grep 'BAT') | grep -E "percentage"
            const cmd = "upower -i $(upower -e | grep 'BAT') | grep -E 'percentage' | awk '{print $2}' | tr -d '%'";
            const output = await this.execPromise(cmd);
            const level = parseInt(output);
            
            if (isNaN(level)) return;

            // Notify if battery is low (below 20%) and has dropped
            if (level <= 20 && (this.lastBatteryLevel === null || this.lastBatteryLevel > 20)) {
                this.emit('low-battery', level);
            }
            
            this.lastBatteryLevel = level;
        } catch (err) {
            // Ignore if upower fails (e.g. desktop)
        }
    }

    async checkNetwork() {
        try {
            // Check internet connection
            const online = await this.execPromise("ping -c 1 8.8.8.8 > /dev/null && echo 'online' || echo 'offline'");
            const isOnline = online === 'online';

            if (this.lastConnectionStatus !== null && this.lastConnectionStatus !== isOnline) {
                this.emit('connection-change', isOnline);
            }

            this.lastConnectionStatus = isOnline;
        } catch (err) {
            // Ignore ping errors
        }
    }

    execPromise(command) {
        return new Promise((resolve) => {
            exec(command, (error, stdout) => {
                if (error) {
                    resolve('');
                    return;
                }
                resolve(stdout.trim());
            });
        });
    }
}

module.exports = new SystemEvents();
