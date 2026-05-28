/**
 * Tests: Chat mode uses the shared provider adapter.
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
    readConfig: jest.fn(() => ({
        aiProvider: 'openai',
        openaiApiKey: 'key',
        openaiModel: 'gpt-test'
    })),
    getAvailableProviders: jest.fn(() => ['openai', 'gemini'])
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
    cacheResponse: jest.fn(),
    clearConversationScopedProfile: jest.fn(),
    searchInteractions: jest.fn(() => [])
}));

jest.mock('../src/AI_Brain/agent_orchestrator', () => ({
    getCurrentAgent: jest.fn(() => ({ name: 'Mint Default', instruction: 'default' }))
}));

jest.mock('../src/CLI/workspace_manager', () => ({
    getWorkspaceByPath: jest.fn(() => null)
}));

jest.mock('../src/AI_Brain/provider_adapter', () => {
    const AgentProviderClient = jest.fn();
    return {
        AgentProviderClient,
        getProviderAttemptOrder: jest.fn(() => ['openai', 'gemini']),
        getProviderModel: jest.fn((provider) => provider === 'openai' ? 'gpt-test' : 'gemini-test')
    };
});

describe('Gemini_API handleChat provider adapter integration', () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    test('routes chat through AgentProviderClient and persists provider metadata', async () => {
        const providerAdapter = require('../src/AI_Brain/provider_adapter');
        const chatHistory = require('../src/System/chat_history_manager');
        const sendMessage = jest.fn(async () => JSON.stringify({
            response: 'adapter response',
            action: { type: 'none', target: '' }
        }));
        providerAdapter.AgentProviderClient.mockImplementation(function MockClient(options) {
            this.provider = options.provider;
            this.providerOrder = options.providerOrder;
            this.lastSuccessfulProvider = 'openai';
            this.sendMessage = sendMessage;
            this.getUsageSummary = jest.fn(() => [{ provider: 'openai', model: 'gpt-test', requests: 1 }]);
        });

        const { handleChat } = require('../src/AI_Brain/Gemini_API');
        const result = await handleChat('hello');

        expect(providerAdapter.AgentProviderClient).toHaveBeenCalledWith(expect.objectContaining({
            provider: 'openai',
            providerOrder: ['openai', 'gemini'],
            responseMimeType: 'application/json'
        }));
        expect(sendMessage).toHaveBeenCalledWith(expect.objectContaining({
            text: expect.stringContaining('hello')
        }));
        expect(result.response).toBe('adapter response');
        expect(result.providerInfo).toEqual(expect.objectContaining({
            provider: 'openai',
            model: 'gpt-test'
        }));
        expect(chatHistory.writeChatHistory).toHaveBeenCalledWith(expect.arrayContaining([
            expect.objectContaining({ role: 'model', providerInfo: expect.objectContaining({ provider: 'openai' }) })
        ]));
    });

    test('keeps Chat Mode restrictive and Agent Mode action-oriented', () => {
        const { _helpers } = require('../src/AI_Brain/Gemini_API');

        expect(_helpers.buildActionModeInstruction({ assistantMode: 'chat' })).toContain('Chat Mode');
        expect(_helpers.buildActionModeInstruction({ assistantMode: 'chat' })).toContain('only when the latest message explicitly asks');
        expect(_helpers.buildActionModeInstruction({ assistantMode: 'agent' })).toContain('Desktop Agent Mode');
        expect(_helpers.buildActionModeInstruction({ assistantMode: 'agent' })).toContain('Choose exactly one action');
    });
});
