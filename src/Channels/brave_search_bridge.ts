const axios = require('axios');

class BraveSearchBridge {
    constructor(credentials) {
        this.apiKey = credentials.apiKey;
    }

    async search(query) {
        if (!this.apiKey) {
            throw new Error('Brave Search API Key is required.');
        }

        try {
            const response = await axios.get('https://api.search.brave.com/res/v1/web/search', {
                params: { q: query, count: 5 },
                headers: {
                    'Accept': 'application/json',
                    'Accept-Encoding': 'gzip',
                    'X-Subscription-Token': this.apiKey
                }
            });

            const results = response.data.web ? response.data.web.results : [];
            return results.map(item => ({
                title: item.title,
                snippet: item.description,
                link: item.url
            }));
        } catch (err) {
            throw new Error(`Brave Search Failed: ${err.message}`);
        }
    }
}

module.exports = BraveSearchBridge;
