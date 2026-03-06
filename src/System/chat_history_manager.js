const fs = require('fs');
const path = require('path');
const { app } = require('electron');

const CHAT_HISTORY_PATH = path.join(app.getPath('userData'), 'mint-chat-history.json');

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
