
import { requireOptional  } from '../System/optional_require'
import { handleChat  } from '../AI_Brain/Gemini_API'

class DiscordBridge {
    [key: string]: any;
    constructor(token) {
        this.token = token;
        const { Client, GatewayIntentBits, Partials } = requireOptional(
            'discord.js',
            'npm install discord.js'
        );
        this.client = new Client({
            intents: [
                GatewayIntentBits.Guilds,
                GatewayIntentBits.GuildMessages,
                GatewayIntentBits.MessageContent,
                GatewayIntentBits.DirectMessages
            ],
            partials: [Partials.Channel]
        });
    }

    async connect() {
        this.client.on('ready', () => {
            console.log(`[Discord Bridge] Logged in as ${this.client.user.tag}!`);
        });

        this.client.on('messageCreate', async (message) => {
            if (message.author.bot) return;
            const isDM = !message.guild;
            const isMentioned = message.mentions.has(this.client.user);

            if (isDM || isMentioned) {
                try {
                    let cleanContent = message.content;
                    if (isMentioned) {
                        cleanContent = message.content
                            .replace(`<@!${this.client.user.id}>`, '')
                            .replace(`<@${this.client.user.id}>`, '')
                            .trim();
                    }
                    if (!cleanContent) return;
                    await message.channel.sendTyping();
                    const result = await handleChat(cleanContent);
                    if (result && result.response) {
                        await message.reply(result.response);
                    }
                } catch (err) {
                    console.error('[Discord Bridge] Error processing message:', err);
                    await message.reply('ขออภัยค่ะ เกิดข้อผิดพลาดบางอย่างในการประมวลผลข้อความ');
                }
            }
        });

        await this.client.login(this.token);
    }

    async disconnect() {
        if (this.client) {
            await this.client.destroy();
        }
    }
}

export default DiscordBridge
