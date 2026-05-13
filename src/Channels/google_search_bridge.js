const axios = require('axios');

class GoogleSearchBridge {
    constructor(credentials) {
        this.apiKey = credentials.apiKey;
        this.cx = credentials.cx; // Custom Search Engine ID
    }

    async search(query) {
        if (!this.apiKey || !this.cx) {
            throw new Error('Google Search API Key and CX are required.');
        }

        try {
            const response = await axios.get('https://www.googleapis.com/customsearch/v1', {
                params: {
                    key: this.apiKey,
                    cx: this.cx,
                    q: query,
                    num: 5
                }
            });

            const items = response.data.items || [];
            return items.map(item => ({
                title: item.title,
                snippet: item.snippet,
                link: item.link
            }));
        } catch (err) {
            throw new Error(err.response && err.response.data && err.response.data.error 
                ? `Google Search API Error: ${err.response.data.error.message}` 
                : `Google Search Failed: ${err.message}`);
        }
    }
}

module.exports = GoogleSearchBridge;
