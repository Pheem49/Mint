const path = require('path');
const fs = require('fs');

console.log('--- Testing Bridge Loading ---');
const channelsDir = path.join(__dirname, '../src/Channels');
const bridges = ['discord', 'telegram', 'slack', 'line', 'whatsapp'];

bridges.forEach(b => {
    const p = path.join(channelsDir, `${b}_bridge.js`);
    if (fs.existsSync(p)) {
        try {
            const Bridge = require(p);
            console.log(`✅ ${b}_bridge.js: Loaded successfully (Class: ${Bridge.name})`);
        } catch (e) {
            console.log(`❌ ${b}_bridge.js: Failed to load - ${e.message}`);
        }
    } else {
        console.log(`❌ ${b}_bridge.js: File NOT FOUND at ${p}`);
    }
});

console.log('\n--- Testing Config Manager Fields ---');
const { readConfig, writeConfig } = require('../src/System/config_manager');
const config = readConfig();
const testFields = [
    'telegramBotToken', 'enableTelegramBridge',
    'discordBotToken', 'enableDiscordBridge',
    'slackBotToken', 'slackAppToken', 'enableSlackBridge',
    'lineChannelAccessToken', 'lineChannelSecret', 'enableLineBridge',
    'enableWhatsappBridge'
];

testFields.forEach(f => {
    if (f in config) {
        console.log(`✅ Config field "${f}": Found (Value: ${config[f]})`);
    } else {
        console.log(`❌ Config field "${f}": MISSING`);
    }
});

console.log('\n--- Testing Data Path Unification ---');
const { CHAT_HISTORY_PATH } = require('../src/System/chat_history_manager');
const { CONFIG_PATH } = require('../src/System/config_manager');
const memoryStore = require('../src/AI_Brain/memory_store');

console.log(`Chat History Path: ${CHAT_HISTORY_PATH}`);
console.log(`Config Path: ${CONFIG_PATH}`);
// We can't easily call getDbPath if it's not exported, but we can check the logic.
// In memory_store.js, getDbPath is internal, let's see if we can check it.
// I'll just check if they are in the same parent directory.

const historyDir = path.dirname(CHAT_HISTORY_PATH);
const configDir = path.dirname(CONFIG_PATH);

if (historyDir === configDir && historyDir.includes('.config/mint')) {
    console.log(`✅ Data paths unified in: ${historyDir}`);
} else {
    console.log(`❌ Path mismatch! History: ${historyDir}, Config: ${configDir}`);
}
