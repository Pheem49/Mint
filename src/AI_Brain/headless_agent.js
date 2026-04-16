/**
 * Mint Headless Agent
 * Runs Mint's background features (like Proactive Suggestions) without a GUI.
 */

const { exec } = require('child_process');
const { getSystemInfo } = require('../System/system_info');
const { readConfig } = require('../System/config_manager');
const systemEvents = require('../System/system_events');
const taskManager = require('../System/task_manager');
const { executeAutonomousTask } = require('./autonomous_brain');

// ANSI Colors for console
const colors = {
    reset: "\x1b[0m",
    bright: "\x1b[1m",
    mint: "\x1b[38;5;121m",
    gray: "\x1b[90m"
};

let isProcessingTask = false;

async function startAgent() {
    console.log(`\n${colors.mint}${colors.bright}[Mint-Agent] Background agent started.${colors.reset}`);
    console.log(`${colors.gray}[Mint-Agent] Monitoring system events and task queue...${colors.reset}\n`);

    // Initialize System Monitoring
    systemEvents.startMonitoring();

    // Listen for Battery Events
    systemEvents.on('low-battery', (level) => {
        sendNotification(
            "⚠️ แบตเตอรี่ใกล้หมดแล้วนะคะ", 
            `ตอนนี้แบตเตอรี่เหลือ ${level}% แล้วค่ะ อย่าลืมชาร์จแบตนะค๊า ✨`
        );
    });

    // Listen for Network Events
    systemEvents.on('connection-change', (isOnline) => {
        const title = isOnline ? "✅ เชื่อมต่อสำเร็จ" : "❌ การเชื่อมต่อขาดหาย";
        const msg = isOnline ? "มิ้นท์เชื่อมต่ออินเทอร์เน็ตได้แล้วค่ะ! ✨" : "มิ้นท์ไม่เห็นสัญญาณอินเทอร์เน็ตเลยนะคะ";
        sendNotification(title, msg);
    });

    // Send a startup notification to let the user know the agent is alive
    sendNotification("Mint Agent", "มิ้นท์ประจำการอยู่เบื้องหลังแล้วนะค๊า! 🛡️✨");
    
    // Polling Loop for Tasks and Health
    setInterval(async () => {
        await checkTaskQueue();
        
        try {
            const info = await getSystemInfo();
            // Heartbeat logic
        } catch (err) {}
    }, 15000); // Check every 15 seconds
}

async function checkTaskQueue() {
    if (isProcessingTask) return;

    const task = taskManager.getPendingTask();
    if (!task) return;

    isProcessingTask = true;
    console.log(`\n${colors.mint}[Agent] Picking up task: ${task.description}${colors.reset}`);
    
    taskManager.updateTask(task.id, { status: 'running' });
    sendNotification("🚀 เริ่มทำงานให้แล้วนะคะ", `กำลังดำเนินการ: ${task.description}`);

    try {
        const result = await executeAutonomousTask(task.description, (progress) => {
            console.log(`${colors.gray}[Progress] ${progress}${colors.reset}`);
            // Send periodic progress notifications if important
            if (progress.includes('เสนอให้รันคำสั่ง')) {
                sendNotification("💡 มิ้นท์มีข้อแนะนำค่ะ", progress);
            }
        });

        taskManager.updateTask(task.id, { status: 'completed', result });
        sendNotification("✅ งานเสร็จเรียบร้อยแล้วค่ะ!", result);
        console.log(`\n${colors.mint}[Agent] Task completed successfully.${colors.reset}`);

    } catch (err) {
        console.error('[Agent] Task execution failed:', err);
        taskManager.updateTask(task.id, { status: 'failed', result: err.message });
        sendNotification("❌ เกิดข้อผิดพลาดในการทำงาน", err.message);
    } finally {
        isProcessingTask = false;
    }
}

/**
 * Sends a system-level notification using notify-send (Linux Pop!_OS)
 */
async function sendNotification(title, message) {
    // Check if notify-send exists before trying to use it
    const hasNotifySend = await new Promise(resolve => {
        exec('which notify-send', (err) => resolve(!err));
    });

    if (!hasNotifySend) {
        console.log(`${colors.gray}[Agent Info]${colors.reset} Notification suppressed (notify-send not found). Install with: sudo apt install libnotify-bin`);
        console.log(`${colors.mint}[Agent Noti]${colors.reset} ${title}: ${message}`);
        return;
    }

    const iconPath = require('path').join(__dirname, '../../assets/icon.png');
    const cmd = `notify-send "${title}" "${message}" -i "${iconPath}" -a "Mint AI"`;
    
    exec(cmd, (err) => {
        if (err) {
            console.error('[Mint-Agent] Failed to send notification:', err.message);
        } else {
            console.log(`${colors.mint}[Agent Noti]${colors.reset} ${title}: ${message}`);
        }
    });
}

module.exports = { startAgent };
