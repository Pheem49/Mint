'use strict';

const { requireOptional } = require('../System/optional_require');
const { handleChat } = require('../AI_Brain/Gemini_API');

class WhatsappBridge {
    constructor() {
        // Dynamic require — only loads if user has installed whatsapp-web.js
        const { Client, LocalAuth } = requireOptional(
            'whatsapp-web.js',
            'npm install whatsapp-web.js qrcode-terminal'
        );
        this._qrcode = requireOptional('qrcode-terminal', 'npm install qrcode-terminal');
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
            this._qrcode.generate(qr, { small: true });
        });

        this.client.on('ready', () => {
            console.log('[WhatsApp Bridge] Client is ready!');
        });

        this.client.on('message', async (msg) => {
            try {
                const chat = await msg.getChat();
                if (chat.isGroup) return;
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
