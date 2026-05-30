const os = require('os');
const { getSystemInfo } = require('../System/system_info');

/**
 * System Metrics Plugin — Provides real-time hardware stats to Gemini
 */
module.exports = {
    name: 'system_metrics',
    description: 'Get real-time system metrics like CPU usage, RAM, and uptime. Instruction can be "all", "ram", "cpu", or "uptime".',
    
    async execute(instruction) {
        const info = getSystemInfo();
        const uptimeMin = Math.floor(os.uptime() / 60);
        const uptimeHours = (uptimeMin / 60).toFixed(1);
        
        const inst = (instruction || 'all').toLowerCase();

        if (inst.includes('ram')) {
            return `ความจำเครื่อง (RAM): ใช้ไป ${info.ram.used} จากทั้งหมด ${info.ram.total} (${info.ram.percent})`;
        } 
        if (inst.includes('cpu')) {
            return `หน่วยประมวลผล (CPU): ${info.cpu.model} มีทั้งหมด ${info.cpu.cores} คอร์`;
        }
        if (inst.includes('uptime')) {
            return `เปิดเครื่องมาแล้ว: ${uptimeMin} นาที (${uptimeHours} ชั่วโมง)`;
        }

        // Default: Return basic summary in Thai for Mint's personality
        return `สรุปสถานะระบบ: RAM ใช้ไป ${info.ram.percent}, CPU ${info.cpu.cores} Cores, เปิดเครื่องมาแล้ว ${uptimeMin} นาทีค่ะ ✨`;
    }
};
