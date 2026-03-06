const os = require('os');

/**
 * ดึงข้อมูล RAM, CPU, เวลาปัจจุบัน
 */
function getSystemInfo() {
    const totalRAM = os.totalmem();
    const freeRAM = os.freemem();
    const usedRAM = totalRAM - freeRAM;
    const ramPercent = ((usedRAM / totalRAM) * 100).toFixed(1);

    const now = new Date();
    const timeStr = now.toLocaleTimeString('th-TH', { hour: '2-digit', minute: '2-digit' });
    const dateStr = now.toLocaleDateString('th-TH', { year: 'numeric', month: 'long', day: 'numeric' });

    const cpuModel = os.cpus()[0]?.model || 'Unknown CPU';
    const cpuCores = os.cpus().length;
    const platform = os.platform();
    const hostname = os.hostname();

    return {
        ram: {
            total: (totalRAM / 1024 / 1024 / 1024).toFixed(2) + ' GB',
            used: (usedRAM / 1024 / 1024 / 1024).toFixed(2) + ' GB',
            free: (freeRAM / 1024 / 1024 / 1024).toFixed(2) + ' GB',
            percent: ramPercent + '%'
        },
        cpu: {
            model: cpuModel,
            cores: cpuCores
        },
        time: timeStr,
        date: dateStr,
        platform,
        hostname
    };
}

/**
 * ดึงข้อมูลอากาศจาก wttr.in (ไม่ต้อง API key)
 * @param {string} city - ชื่อเมือง เช่น Bangkok
 */
async function getWeather(city = 'Bangkok') {
    try {
        const response = await fetch(`https://wttr.in/${encodeURIComponent(city)}?format=3&lang=th`);
        if (!response.ok) throw new Error('Weather fetch failed');
        const text = await response.text();
        return { success: true, data: text.trim() };
    } catch (err) {
        console.error('getWeather error:', err);
        return { success: false, data: 'ไม่สามารถดึงข้อมูลอากาศได้ในขณะนี้' };
    }
}

module.exports = { getSystemInfo, getWeather };
