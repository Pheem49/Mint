jest.mock('@google/genai', () => ({
    GoogleGenAI: jest.fn()
}));

jest.mock('axios', () => ({
    post: jest.fn()
}));

jest.mock('../dist/src/System/config_manager', () => ({
    getAvailableProviders: jest.fn((config = {}) => {
        const providers = [];
        if (config.anthropicApiKey) providers.push('anthropic');
        if (config.openaiApiKey) providers.push('openai');
        if (config.apiKey) providers.push('gemini');
        if (config.localApiBaseUrl) providers.push('local_openai');
        providers.push('ollama');
        return providers;
    })
}));

describe('provider_adapter', () => {
    test('builds a provider order from one shared policy', () => {
        const adapter = require('../dist/src/AI_Brain/provider_adapter');
        const order = adapter.getProviderAttemptOrder({
            aiProvider: 'openai',
            openaiApiKey: 'key',
            apiKey: 'gemini-key',
            localApiBaseUrl: 'http://localhost:1234/v1'
        }, {
            supported: ['gemini', 'openai', 'local_openai'],
            priority: ['gemini', 'openai', 'local_openai']
        });

        expect(order[0]).toBe('openai');
        expect(order).toEqual(['openai', 'gemini', 'local_openai']);
    });

    test('uses common model resolution for every mode', () => {
        const adapter = require('../dist/src/AI_Brain/provider_adapter');
        expect(adapter.getProviderModel('anthropic', {})).toBe('claude-3-5-sonnet-latest');
        expect(adapter.getProviderModel('openai', { openaiModel: 'gpt-test' })).toBe('gpt-test');
        expect(adapter.getProviderModel('ollama', {})).toBe('llama3:latest');
    });

    test('normalizes multi-image and audio chat content for providers', () => {
        const adapter = require('../dist/src/AI_Brain/provider_adapter');
        const content = {
            text: 'analyze',
            imageDataUris: [
                'data:image/png;base64,aaa',
                'data:image/jpeg;base64,bbb'
            ],
            audioDataUri: 'data:audio/webm;base64,ccc'
        };

        const geminiParts = adapter._helpers.contentToGeminiParts(content);
        expect(geminiParts).toHaveLength(4);
        expect(geminiParts[1].inlineData.mimeType).toBe('image/png');
        expect(geminiParts[3].inlineData.mimeType).toBe('audio/webm');

        const openaiContent = adapter._helpers.contentToOpenAIContent(content);
        expect(openaiContent).toHaveLength(3);
        expect(openaiContent[1].image_url.url).toContain('image/png');

        const ollamaMessage = adapter._helpers.contentToOllamaMessage(content);
        expect(ollamaMessage.images).toEqual(['aaa', 'bbb']);
    });
});
