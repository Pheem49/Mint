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
const LEGACY_DIR = path.join(os.homedir(), '.mint');

if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
}

// Migration: If old .mint exists but new .config/mint is empty, move files
if (fs.existsSync(LEGACY_DIR) && fs.readdirSync(CONFIG_DIR).length === 0) {
    try {
        const files = fs.readdirSync(LEGACY_DIR);
        for (const file of files) {
            fs.copyFileSync(path.join(LEGACY_DIR, file), path.join(CONFIG_DIR, file));
        }
        console.log('[Config] Migrated settings from ~/.mint to ~/.config/mint');
    } catch (e) {
        console.error('[Config] Migration failed:', e);
    }
}

const CONFIG_PATH = path.join(CONFIG_DIR, 'mint-config.json');

const DEFAULT_CONFIG = {
    theme: 'dark',
    accentColor: '#8b5cf6',
    systemTextColor: '#f8fafc',
    customBgStart: '#0f172a',
    customBgEnd: '#1e1b4b',
    customPanelBg: '#1e293b',
    apiKey: '',
    geminiModel: 'gemini-2.5-flash',
    language: 'th-TH',
    automationBrowser: 'chromium',
    proactiveInterval: 60,   // seconds between screen captures
    proactiveCooldown: 120,   // seconds minimum between actual suggestions
    aiProvider: 'gemini',
    ollamaModel: 'llama3:latest',
    enableVoiceReply: true,
    enableCustomWorkflows: true,
    ttsProvider: 'google',
    ttsVolume: 1.0,
    ttsSpeed: 1.0,
    ttsPitch: 1.0,
    pluginSpotifyEnabled: true,
    pluginCalendarEnabled: false,
    pluginDiscordEnabled: false,
    showDesktopWidget: true,
    mcpServers: {},
    anthropicApiKey: '',
    openaiApiKey: '',
    anthropicModel: 'claude-3-5-sonnet-latest',
    openaiModel: 'gpt-4o'
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
