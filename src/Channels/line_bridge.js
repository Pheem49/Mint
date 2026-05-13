const line = require('@line/bot-sdk');
const express = require('express');
const { handleChat } = require('../AI_Brain/Gemini_API');

class LineBridge {
    constructor(credentials) {
        this.config = {
            channelAccessToken: credentials.accessToken,
            channelSecret: credentials.secret,
        };
        this.port = credentials.port || 3000;
        this.client = new line.messagingApi.MessagingApiClient({
            channelAccessToken: credentials.accessToken
        });
        this.app = express();
    }

    async connect() {
        this.app.post('/callback', line.middleware(this.config), (req, res) => {
            Promise
                .all(req.body.events.map(event => this.handleEvent(event)))
                .then((result) => res.json(result))
                .catch((err) => {
                    console.error('[LINE Bridge] Error:', err);
                    res.status(500).end();
                });
        });

        this.server = this.app.listen(this.port, () => {
            console.log(`[LINE Bridge] Listening for webhooks on port ${this.port}`);
            console.log(`[LINE Bridge] Webhook URL should be: <YOUR_PUBLIC_URL>/callback`);
        });
    }

    async handleEvent(event) {
        if (event.type !== 'message' || event.message.type !== 'text') {
            return Promise.resolve(null);
        }

        try {
            const result = await handleChat(event.message.text);
            if (result && result.response) {
                return this.client.replyMessage({
                    replyToken: event.replyToken,
                    messages: [{ type: 'text', text: result.response }],
                });
            }
        } catch (err) {
            console.error('[LINE Bridge] Error processing event:', err);
        }
    }

    async disconnect() {
        if (this.server) {
            this.server.close();
        }
    }
}

module.exports = LineBridge;
