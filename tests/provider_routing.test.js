/**
 * Tests: Gemini_API provider routing helpers
 */

jest.mock('@google/genai', () => ({
    GoogleGenAI: jest.fn(() => ({
        chats: {
            create: jest.fn(() => ({
                sendMessage: jest.fn(),
                sendMessageStream: jest.fn(),
                getHistory: jest.fn(async () => [])
            }))
        },
        models: {
            generateContent: jest.fn()
        }
    }))
}));

jest.mock('../src/System/chat_history_manager', () => ({
    readChatHistory: jest.fn(() => []),
    writeChatHistory: jest.fn(),
    clearChatHistory: jest.fn()
}));

jest.mock('../src/System/config_manager', () => ({
    readConfig: jest.fn(() => ({})),
    getAvailableProviders: jest.fn(() => ['ollama', 'gemini'])
}));

jest.mock('../src/Plugins/plugin_manager', () => ({
    loadPlugins: jest.fn(),
    getPromptDescriptions: jest.fn(() => '')
}));

jest.mock('../src/Plugins/mcp_manager', () => ({
    getAllTools: jest.fn(() => [])
}));

jest.mock('../src/AI_Brain/memory_store', () => ({
    getUserContext: jest.fn(() => ''),
    getCachedResponse: jest.fn(),
    recordInteraction: jest.fn(),
    cacheResponse: jest.fn()
}));

jest.mock('../src/AI_Brain/agent_orchestrator', () => ({
    getCurrentAgent: jest.fn(() => ({ name: 'Mint Default', instruction: 'default' }))
}));

jest.mock('../src/CLI/workspace_manager', () => ({
    getWorkspaceByPath: jest.fn(() => null)
}));

describe('Gemini_API provider routing helpers', () => {
    test('prioritizes configured provider, then falls back to available providers', () => {
        const geminiApi = require('../src/AI_Brain/Gemini_API');
        const order = geminiApi._helpers.getProviderAttemptOrder({
            aiProvider: 'openai',
            openaiApiKey: 'key',
            localApiBaseUrl: 'http://localhost:1234/v1'
        });

        expect(order[0]).toBe('openai');
        expect(order).toContain('ollama');
    });
});
