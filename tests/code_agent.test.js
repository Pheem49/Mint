/**
 * Tests: code_agent helpers
 */

jest.mock('@google/genai', () => ({
    GoogleGenAI: jest.fn()
}));

jest.mock('axios', () => ({}));

jest.mock('../src/System/config_manager', () => ({
    readConfig: jest.fn(() => ({})),
    getAvailableProviders: jest.fn(() => ['ollama', 'gemini'])
}));

jest.mock('../src/CLI/code_session_memory', () => ({
    readWorkspaceSession: jest.fn(() => ({
        summary: '',
        lastTask: '',
        lastVerification: '',
        updatedAt: null
    })),
    writeWorkspaceSession: jest.fn()
}));

const fs = require('fs');
const path = require('path');
const os = require('os');

describe('code_agent helpers', () => {
    test('extractJson recovers JSON embedded in surrounding text', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const parsed = _helpers.extractJson('note\n{"action":"finish","input":{"summary":"ok"}}\nthanks');
        expect(parsed.action).toBe('finish');
        expect(parsed.input.summary).toBe('ok');
    });

    test('selectSupportedCodeProvider falls back away from unsupported code providers', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const selected = _helpers.selectSupportedCodeProvider(
            { aiProvider: 'ollama' },
            ['ollama', 'openai', 'gemini']
        );
        expect(selected).toBe('openai');
    });

    test('selectSupportedCodeProvider keeps configured supported provider when available', () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const selected = _helpers.selectSupportedCodeProvider(
            { aiProvider: 'anthropic' },
            ['anthropic', 'gemini']
        );
        expect(selected).toBe('anthropic');
    });

    test('findPaths can locate directories by partial name', async () => {
        const { _helpers } = require('../src/CLI/code_agent');
        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-code-agent-'));
        const targetDir = path.join(tempDir, 'projects', 'xidaidai');
        fs.mkdirSync(targetDir, { recursive: true });

        try {
            const result = await _helpers.findPaths(tempDir, 'xidaidai', 'dir');
            expect(result).toContain('[dir] projects/xidaidai');
        } finally {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });
});
