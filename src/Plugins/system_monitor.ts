/**
 * Mint System Monitor Plugin
 * --------------------------
 * Provides real-time system statistics for the host machine.
 * Uses standard Linux commands (uptime, free, df) for lightweight monitoring.
 */

import { exec  } from 'child_process'
import { promisify  } from 'util'
const execAsync = promisify(exec);
import * as os from 'os'

async function getStats() {
    try {
        const [uptime, free, df] = await Promise.all([
            execAsync('uptime -p'),
            execAsync('free -h'),
            execAsync('df -h / --output=pcent,avail')
        ]);

        // Parse Memory
        const memLines = free.stdout.split('\n');
        const memLine = memLines.find(l => l.startsWith('Mem:')) || '';
        const memParts = memLine.split(/\s+/).filter(Boolean);
        const memUsed = memParts[2] || 'Unknown';
        const memTotal = memParts[1] || 'Unknown';

        // Parse Disk
        const diskLines = df.stdout.trim().split('\n');
        const diskLine = diskLines[1] || '';
        const [diskPercent, diskAvail] = diskLine.trim().split(/\s+/);

        const cpuLoad = os.loadavg()[0].toFixed(2);
        const cpuCores = os.cpus().length;

        let report = `📊 **System Health Report**\n`;
        report += `⏱️ **Uptime:** ${uptime.stdout.trim()}\n`;
        report += `💻 **CPU Load:** ${cpuLoad} (on ${cpuCores} cores)\n`;
        report += `🧠 **Memory:** ${memUsed} / ${memTotal} used\n`;
        report += `💽 **Disk (/):** ${diskAvail} available (${diskPercent} full)`;

        return report;
    } catch (err) {
        return `❌ Error fetching system stats: ${err.message}`;
    }
}

const plugin = {
    name: 'system_monitor',
    description: 'Provides system statistics like CPU load, memory usage, disk space, and uptime. Target can be "stats", "cpu", "memory", or "disk".',

    async execute(target: any) {
        const cmd = (target || 'stats').toLowerCase().trim();
        
        switch (cmd) {
            case 'stats':
            case 'health':
                return await getStats();
            case 'cpu':
                return `💻 **CPU Load (1m):** ${os.loadavg()[0].toFixed(2) }\nCores: ${os.cpus().length}\nModel: ${os.cpus()[0].model}`;
            case 'memory':
            case 'ram':
                const { stdout: mem } = await execAsync('free -h');
                return `🧠 **Memory Status:**\n\`\`\`\n${mem}\`\`\``;
            case 'disk':
                const { stdout: disk } = await execAsync('df -h /');
                return `💽 **Disk Status:**\n\`\`\`\n${disk}\`\`\``;
            default:
                return await getStats();
        }
    }
};

export = plugin;
