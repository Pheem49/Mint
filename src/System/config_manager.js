const fs = require('fs');
const path = require('path');
const { app } = require('electron');

const CONFIG_PATH = path.join(app.getPath('userData'), 'mint-config.json');

const DEFAULT_CONFIG = {
    theme: 'dark',
    accentColor: '#8b5cf6',
    systemTextColor: '#f8fafc',
    customBgStart: '#0f172a',
    customBgEnd: '#1e1b4b',
    customPanelBg: '#1e293b',
    apiKey: '',
    geminiModel: 'gemini-3.1-flash-lite-preview',
    language: 'th-TH',
    automationBrowser: 'chromium',
    proactiveInterval: 60,   // seconds between screen captures
    proactiveCooldown: 120,   // seconds minimum between actual suggestions
    aiProvider: 'gemini',
    ollamaModel: 'llama3:latest'
};

function readConfig() {
    try {
        if (!fs.existsSync(CONFIG_PATH)) {
            writeConfig(DEFAULT_CONFIG);
            return DEFAULT_CONFIG;
        }
        const raw = fs.readFileSync(CONFIG_PATH, 'utf-8');
        return { ...DEFAULT_CONFIG, ...JSON.parse(raw) };
    } catch (err) {
        console.error('readConfig error:', err);
        return DEFAULT_CONFIG;
    }
}

function writeConfig(config) {
    try {
        fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2), 'utf-8');
        return { success: true };
    } catch (err) {
        console.error('writeConfig error:', err);
        return { success: false, message: err.message };
    }
}

module.exports = { readConfig, writeConfig, CONFIG_PATH };
