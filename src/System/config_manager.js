const fs = require('fs');
const path = require('path');
const { app } = require('electron');

const CONFIG_PATH = path.join(app.getPath('userData'), 'mint-config.json');

const DEFAULT_CONFIG = {
    theme: 'dark',
    accentColor: '#8b5cf6',
    apiKey: '',
    language: 'th-TH'
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
