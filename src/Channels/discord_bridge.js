const { Client, GatewayIntentBits, Partials } = require('discord.js');
const { handleChat } = require('../AI_Brain/Gemini_API');

class DiscordBridge {
    constructor(token) {
        this.token = token;
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
            // Ignore bot messages
            if (message.author.bot) return;

            // Handle DMs or Mentions
            const isDM = !message.guild;
            const isMentioned = message.mentions.has(this.client.user);

            if (isDM || isMentioned) {
                try {
                    // Clean up the message if it's a mention
                    let cleanContent = message.content;
                    if (isMentioned) {
                        cleanContent = message.content.replace(`<@!${this.client.user.id}>`, '').replace(`<@${this.client.user.id}>`, '').trim();
                    }

                    if (!cleanContent) return;

                    // Show typing indicator
                    await message.channel.sendTyping();

                    // Send to Mint AI Brain
                    const result = await handleChat(cleanContent);

                    // Reply to user
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

module.exports = DiscordBridge;
