
import { requireOptional  } from '../System/optional_require'
import { handleChat  } from '../AI_Brain/Gemini_API'

class TelegramBridge {
    [key: string]: any;
    constructor(token) {
        this.token = token;
        const { Telegraf } = requireOptional('telegraf', 'npm install telegraf');
        this.bot = new Telegraf(token);
    }

    async connect() {
        this.bot.start((ctx) => ctx.reply('สวัสดีค่ะ! มิ้นท์พร้อมช่วยเหลือคุณใน Telegram แล้วนะคะ ✨'));

        this.bot.on('text', async (ctx) => {
            try {
                await ctx.sendChatAction('typing');
                const message = ctx.message.text;
                if (!message) return;
                const result = await handleChat(message);
                if (result && result.response) await ctx.reply(result.response);
            } catch (err) {
                console.error('[Telegram Bridge] Error processing message:', err);
                await ctx.reply('ขออภัยค่ะ เกิดข้อผิดพลาดบางอย่างในการประมวลผลข้อความ');
            }
        });

        this.bot.launch();
        console.log('[Telegram Bridge] Bot started!');

        process.once('SIGINT', () => this.bot.stop('SIGINT'));
        process.once('SIGTERM', () => this.bot.stop('SIGTERM'));
    }

    async disconnect() {
        if (this.bot) await this.bot.stop();
    }
}

export default TelegramBridge
