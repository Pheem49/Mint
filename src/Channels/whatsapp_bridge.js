const { Client, LocalAuth } = require('whatsapp-web.js');
const qrcode = require('qrcode-terminal');
const { handleChat } = require('../AI_Brain/Gemini_API');

class WhatsappBridge {
    constructor() {
        this.client = new Client({
            authStrategy: new LocalAuth({
                dataPath: require('path').join(require('os').homedir(), '.config', 'mint', 'whatsapp-session')
            }),
            puppeteer: {
                args: ['--no-sandbox']
            }
        });
    }

    async connect() {
        this.client.on('qr', (qr) => {
            console.log('[WhatsApp Bridge] Scan this QR code to login:');
            qrcode.generate(qr, { small: true });
        });

        this.client.on('ready', () => {
            console.log('[WhatsApp Bridge] Client is ready!');
        });

        this.client.on('message', async (msg) => {
            try {
                // Ignore messages from groups unless mentioned (simple implementation)
                const chat = await msg.getChat();
                if (chat.isGroup) {
                    // For groups, we could add a mention check here if desired
                    return;
                }

                const result = await handleChat(msg.body);
                if (result && result.response) {
                    await msg.reply(result.response);
                }
            } catch (err) {
                console.error('[WhatsApp Bridge] Error processing message:', err);
            }
        });

        await this.client.initialize();
    }

    async disconnect() {
        if (this.client) {
            await this.client.destroy();
        }
    }
}

module.exports = WhatsappBridge;
