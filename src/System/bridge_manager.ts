import { readConfig } from './config_manager'

// Static imports to support Vite packaging
const Bridges: Record<string, any> = {
    discord: () => require('../Channels/discord_bridge'),
    telegram: () => require('../Channels/telegram_bridge'),
    slack: () => require('../Channels/slack_bridge'),
    line: () => require('../Channels/line_bridge'),
    whatsapp: () => require('../Channels/whatsapp_bridge')
}

class BridgeManager {
    [key: string]: any;
    bridges: Map<string, any>

    constructor() {
        this.bridges = new Map()
    }

    async init() {
        const config = readConfig()
        console.log('[BridgeManager] Initializing messaging bridges...')

        // Load Discord Bridge
        if (config.enableDiscordBridge && config.discordBotToken) {
            await this.startBridge('discord', config.discordBotToken)
        }

        // Load Telegram Bridge
        if (config.enableTelegramBridge && config.telegramBotToken) {
            await this.startBridge('telegram', config.telegramBotToken)
        }

        // Load Slack Bridge
        if (config.enableSlackBridge && config.slackBotToken && config.slackAppToken) {
            await this.startBridge('slack', { botToken: config.slackBotToken, appToken: config.slackAppToken })
        }

        // Load LINE Bridge
        if (config.enableLineBridge && config.lineChannelAccessToken && config.lineChannelSecret) {
            await this.startBridge('line', { accessToken: config.lineChannelAccessToken, secret: config.lineChannelSecret, port: config.lineWebhookPort })
        }

        // Load WhatsApp Bridge
        if (config.enableWhatsappBridge) {
            await this.startBridge('whatsapp', null)
        }
    }

    async startBridge(type: string, credentials: any) {
        try {
            const loadBridge = Bridges[type]
            if (!loadBridge) {
                console.error(`[BridgeManager] Bridge type not supported: ${type}`)
                return
            }

            const BridgeClass = loadBridge()
            const bridge = new BridgeClass(credentials)
            await bridge.connect()
            this.bridges.set(type, bridge)
            console.log(`[BridgeManager] ${type.toUpperCase()} bridge connected successfully.`)
        } catch (err: any) {
            console.error(`[BridgeManager] Failed to start ${type} bridge:`, err.message)
        }
    }

    async shutdown() {
        for (const [type, bridge] of this.bridges.entries()) {
            try {
                await bridge.disconnect()
                console.log(`[BridgeManager] ${type.toUpperCase()} bridge disconnected.`)
            } catch (err: any) {
                console.error(`[BridgeManager] Error disconnecting ${type} bridge:`, err.message)
            }
        }
        this.bridges.clear()
    }
}

const bridgeManager = new BridgeManager()
export default bridgeManager
