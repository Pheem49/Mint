const { readConfig } = require('./config_manager');
const path = require('path');
const fs = require('fs');

class BridgeManager {
    constructor() {
        this.bridges = new Map();
        this.channelsDir = path.join(__dirname, '..', 'Channels');
        
        if (!fs.existsSync(this.channelsDir)) {
            fs.mkdirSync(this.channelsDir, { recursive: true });
        }
    }

    async init() {
        const config = readConfig();
        console.log('[BridgeManager] Initializing messaging bridges...');

        // Load Discord Bridge
        if (config.enableDiscordBridge && config.discordBotToken) {
            await this.startBridge('discord', config.discordBotToken);
        }

        // Load Telegram Bridge
        if (config.enableTelegramBridge && config.telegramBotToken) {
            await this.startBridge('telegram', config.telegramBotToken);
        }

        // Load Slack Bridge
        if (config.enableSlackBridge && config.slackBotToken && config.slackAppToken) {
            await this.startBridge('slack', { botToken: config.slackBotToken, appToken: config.slackAppToken });
        }

        // Load LINE Bridge
        if (config.enableLineBridge && config.lineChannelAccessToken && config.lineChannelSecret) {
            await this.startBridge('line', { accessToken: config.lineChannelAccessToken, secret: config.lineChannelSecret, port: config.lineWebhookPort });
        }

        // Load WhatsApp Bridge
        if (config.enableWhatsappBridge) {
            await this.startBridge('whatsapp', null);
        }
    }

    async startBridge(type, credentials) {
        try {
            const bridgePath = path.join(this.channelsDir, `${type}_bridge.js`);
            if (!fs.existsSync(bridgePath)) {
                console.error(`[BridgeManager] Bridge file not found: ${bridgePath}`);
                return;
            }

            const BridgeClass = require(bridgePath);
            const bridge = new BridgeClass(credentials);
            await bridge.connect();
            this.bridges.set(type, bridge);
            console.log(`[BridgeManager] ${type.toUpperCase()} bridge connected successfully.`);
        } catch (err) {
            console.error(`[BridgeManager] Failed to start ${type} bridge:`, err.message);
        }
    }

    async shutdown() {
        for (const [type, bridge] of this.bridges.entries()) {
            try {
                await bridge.disconnect();
                console.log(`[BridgeManager] ${type.toUpperCase()} bridge disconnected.`);
            } catch (err) {
                console.error(`[BridgeManager] Error disconnecting ${type} bridge:`, err.message);
            }
        }
        this.bridges.clear();
    }
}

module.exports = new BridgeManager();
