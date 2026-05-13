const { Telegraf } = require('telegraf');
const { handleChat } = require('../AI_Brain/Gemini_API');

class TelegramBridge {
    constructor(token) {
        this.token = token;
        this.bot = new Telegraf(token);
    }

    async connect() {
        this.bot.start((ctx) => ctx.reply('สวัสดีค่ะ! มิ้นท์พร้อมช่วยเหลือคุณใน Telegram แล้วนะคะ ✨'));

        this.bot.on('text', async (ctx) => {
            try {
                // Show typing status
                await ctx.sendChatAction('typing');

                const message = ctx.message.text;
                if (!message) return;

                // Send to Mint AI Brain
                const result = await handleChat(message);

                // Reply to user
                if (result && result.response) {
                    await ctx.reply(result.response);
                }
            } catch (err) {
                console.error('[Telegram Bridge] Error processing message:', err);
                await ctx.reply('ขออภัยค่ะ เกิดข้อผิดพลาดบางอย่างในการประมวลผลข้อความ');
            }
        });

        this.bot.launch();
        console.log('[Telegram Bridge] Bot started!');

        // Enable graceful stop
        process.once('SIGINT', () => this.bot.stop('SIGINT'));
        process.once('SIGTERM', () => this.bot.stop('SIGTERM'));
    }

    async disconnect() {
        if (this.bot) {
            await this.bot.stop();
        }
    }
}

module.exports = TelegramBridge;
