/**
 * Tests: chat_router routing helpers
 */

jest.mock('@google/genai', () => ({
    GoogleGenAI: jest.fn()
}));

jest.mock('../src/CLI/code_agent', () => ({
    executeCodeTask: jest.fn(),
    _helpers: {
        selectSupportedCodeProvider: jest.fn(() => 'gemini')
    }
}));

jest.mock('../src/System/config_manager', () => ({
    readConfig: jest.fn(() => ({})),
    getAvailableProviders: jest.fn(() => ['gemini'])
}));

describe('chat_router helpers', () => {
    test('recognizes direct folder open request as chat task', () => {
        const { _helpers } = require('../src/CLI/chat_router');
        expect(_helpers.isDirectFilesystemActionRequest('เปิดโฟลเดอร์ xidaidai ให้หน่อย')).toBe(true);
    });

    test('does not classify direct folder open request as code intent', () => {
        const { _helpers } = require('../src/CLI/chat_router');
        const route = _helpers.detectCodeIntentHeuristic('open folder xidaidai', process.cwd());
        expect(route).toBe(false);
    });

    test('treats small file-related request as normal chat', () => {
        const { _helpers } = require('../src/CLI/chat_router');
        expect(_helpers.isLargeCodeTaskRequest('ดูไฟล์ package.json ให้หน่อย', process.cwd())).toBe(false);
    });

    test('treats short assistant mention as normal chat', async () => {
        const { detectCodeIntent, _helpers } = require('../src/CLI/chat_router');
        expect(_helpers.detectCodeIntentHeuristic('มิ้น', process.cwd())).toBe(false);
        await expect(detectCodeIntent('มิ้น', process.cwd(), [])).resolves.toMatchObject({
            route: 'chat'
        });
    });

    test('treats substantial project fix request as code task', () => {
        const { _helpers } = require('../src/CLI/chat_router');
        expect(_helpers.isLargeCodeTaskRequest('fix the failing tests in this project and verify the result', process.cwd())).toBe(true);
    });
});
