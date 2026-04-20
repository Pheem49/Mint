const SystemAutomation = require('../src/System/system_automation');

async function test() {
    console.log("Testing getSystemInfo()...");
    try {
        const info = await SystemAutomation.getSystemInfo("");
        console.log("Result:\n", info);
    } catch (err) {
        console.error("Error:", err);
    }
}

test();
