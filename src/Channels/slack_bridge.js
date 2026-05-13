const { App } = require('@slack/bolt');
const { handleChat } = require('../AI_Brain/Gemini_API');

class SlackBridge {
    constructor(credentials) {
        this.app = new App({
            token: credentials.botToken,
            appToken: credentials.appToken,
            socketMode: true
        });
    }

    async connect() {
        this.app.event('app_mention', async ({ event, say }) => {
            try {
                const text = event.text.replace(/<@.*?>/g, '').trim();
                if (!text) return;

                const result = await handleChat(text);
                if (result && result.response) {
                    await say(result.response);
                }
            } catch (err) {
                console.error('[Slack Bridge] Error processing app_mention:', err);
            }
        });

        this.app.event('message', async ({ event, say }) => {
            // Only respond in DMs
            if (event.channel_type === 'im') {
                try {
                    const result = await handleChat(event.text);
                    if (result && result.response) {
                        await say(result.response);
                    }
                } catch (err) {
                    console.error('[Slack Bridge] Error processing message:', err);
                }
            }
        });

        await this.app.start();
        console.log('[Slack Bridge] App started in Socket Mode!');
    }

    async disconnect() {
        if (this.app) {
            await this.app.stop();
        }
    }
}

module.exports = SlackBridge;
