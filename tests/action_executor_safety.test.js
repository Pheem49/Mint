jest.mock('electron', () => ({
    clipboard: {
        writeText: jest.fn()
    }
}));

jest.mock('../src/Automation_Layer/file_operations', () => ({
    createFolder: jest.fn(() => ({ success: true })),
    openFile: jest.fn(async () => true),
    deleteFile: jest.fn(async () => ({ success: true })),
    findPath: jest.fn(() => ({ success: false, message: 'not found', matches: [] }))
}));

jest.mock('../src/Automation_Layer/open_app', () => ({
    openApp: jest.fn()
}));

jest.mock('../src/Automation_Layer/open_website', () => ({
    openWebsite: jest.fn(),
    openSearch: jest.fn()
}));

jest.mock('../src/Automation_Layer/browser_automation', () => ({
    performWebAutomation: jest.fn(async () => 'done')
}));

jest.mock('../src/AI_Brain/knowledge_base', () => ({
    indexFile: jest.fn(async () => 'indexed'),
    indexFolder: jest.fn(async () => 'indexed')
}));

jest.mock('../src/Plugins/plugin_manager', () => ({
    executePlugin: jest.fn()
}));

jest.mock('../src/Plugins/mcp_manager', () => ({
    callTool: jest.fn()
}));

jest.mock('../src/System/granular_automation', () => ({
    mouseMove: jest.fn(),
    mouseClick: jest.fn(),
    typeText: jest.fn(),
    keyTap: jest.fn()
}));

jest.mock('../src/System/system_automation', () => ({
    shutdown: jest.fn(),
    restart: jest.fn(),
    sleep: jest.fn(),
    setVolume: jest.fn(),
    mute: jest.fn(),
    setBrightness: jest.fn(),
    minimizeAll: jest.fn()
}));

describe('action_executor safety integration', () => {
    test('blocks dangerous delete actions unless explicitly allowed', async () => {
        const { executeAction } = require('../src/System/action_executor');
        await expect(executeAction({ type: 'delete_file', target: 'notes.txt' })).rejects.toThrow(/Dangerous action/);
    });

    test('allows dangerous actions with explicit permission flag', async () => {
        const { executeAction } = require('../src/System/action_executor');
        await expect(executeAction({ type: 'delete_file', target: 'notes.txt' }, { allowDangerous: true })).resolves.toBeUndefined();
    });
});
