const os = require('os');
const fs = require('fs');

function readFirstExisting(paths) {
    for (const filePath of paths) {
        try {
            const value = fs.readFileSync(filePath, 'utf8').trim();
            if (value && value !== 'None' && value !== 'To be filled by O.E.M.') {
                return value;
            }
        } catch (_) {}
    }
    return '';
}

function getLinuxDistro() {
    try {
        const content = fs.readFileSync('/etc/os-release', 'utf8');
        const values = {};
        content.split('\n').forEach(line => {
            const match = line.match(/^([A-Z_]+)=(.*)$/);
            if (match) {
                values[match[1]] = match[2].replace(/^"|"$/g, '');
            }
        });
        return values.PRETTY_NAME || values.NAME || '';
    } catch (_) {
        return '';
    }
}

function getMachineModel() {
    if (os.platform() !== 'linux') {
        return {
            vendor: '',
            product: os.hostname(),
            version: '',
            board: '',
            display: os.hostname()
        };
    }

    const vendor = readFirstExisting([
        '/sys/devices/virtual/dmi/id/sys_vendor',
        '/sys/class/dmi/id/sys_vendor'
    ]);
    const product = readFirstExisting([
        '/sys/devices/virtual/dmi/id/product_name',
        '/sys/class/dmi/id/product_name'
    ]);
    const version = readFirstExisting([
        '/sys/devices/virtual/dmi/id/product_version',
        '/sys/class/dmi/id/product_version'
    ]);
    const board = readFirstExisting([
        '/sys/devices/virtual/dmi/id/board_name',
        '/sys/class/dmi/id/board_name'
    ]);

    return {
        vendor,
        product,
        version,
        board,
        display: [vendor, product, version].filter(Boolean).join(' ') || board || os.hostname()
    };
}

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
    const distro = platform === 'linux' ? getLinuxDistro() : '';
    const hostname = os.hostname();
    const machine = getMachineModel();

    return {
        machine,
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
        distro,
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
