const fs = require('fs');
const path = require('path');
const os = require('os');

let app;
try {
    const electron = require('electron');
    app = electron.app;
} catch (e) {
    app = null;
}

const CONFIG_DIR = path.join(os.homedir(), '.config', 'mint');
const MINT_DIR = path.join(os.homedir(), '.mint');

if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
}

const CHAT_HISTORY_PATH = path.join(CONFIG_DIR, 'mint-chat-history.json');

// Migration Logic: Consolidate from Electron userData or old ~/.mint to ~/.config/mint
if (!fs.existsSync(CHAT_HISTORY_PATH)) {
    const electronUserData = app && app.getPath ? path.join(app.getPath('userData'), 'mint-chat-history.json') : null;
    const legacyPath = path.join(MINT_DIR, 'mint-chat-history.json');

    if (electronUserData && fs.existsSync(electronUserData)) {
        try {
            fs.copyFileSync(electronUserData, CHAT_HISTORY_PATH);
            console.log('[History] Migrated chat history from Electron userData');
        } catch (e) { console.error('[History] Migration from Electron failed:', e); }
    } else if (fs.existsSync(legacyPath)) {
        try {
            fs.copyFileSync(legacyPath, CHAT_HISTORY_PATH);
            console.log('[History] Migrated chat history from ~/.mint');
        } catch (e) { console.error('[History] Migration from ~/.mint failed:', e); }
    }
}

function readChatHistory() {
    try {
        if (!fs.existsSync(CHAT_HISTORY_PATH)) {
            return [];
        }

        const raw = fs.readFileSync(CHAT_HISTORY_PATH, 'utf-8');
        const parsed = JSON.parse(raw);
        return Array.isArray(parsed) ? parsed : [];
    } catch (err) {
        console.error('readChatHistory error:', err);
        return [];
    }
}

function writeChatHistory(history) {
    try {
        const safeHistory = Array.isArray(history) ? history : [];
        fs.writeFileSync(CHAT_HISTORY_PATH, JSON.stringify(safeHistory, null, 2), 'utf-8');
        return { success: true };
    } catch (err) {
        console.error('writeChatHistory error:', err);
        return { success: false, message: err.message };
    }
}

function clearChatHistory() {
    return writeChatHistory([]);
}

module.exports = {
    CHAT_HISTORY_PATH,
    readChatHistory,
    writeChatHistory,
    clearChatHistory
};
