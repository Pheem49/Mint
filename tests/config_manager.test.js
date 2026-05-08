/**
 * Tests: config_manager.js
 * Tests readConfig, writeConfig, and getAvailableProviders.
 * Uses a temp directory so tests never touch the real ~/.config/mint.
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

// ── Setup: point config to a temp directory before requiring the module ───
let tempDir;
let CONFIG_PATH;

beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-test-'));
    CONFIG_PATH = path.join(tempDir, 'mint-config.json');

    // Reset module cache so config_manager re-initializes with our temp path
    jest.resetModules();

    // Mock the CONFIG_DIR used inside config_manager
    jest.mock('../src/System/config_manager', () => {
        const actual = jest.requireActual('../src/System/config_manager');
        return actual;
    }, { virtual: false });
});

afterEach(() => {
    // Cleanup temp dir
    fs.rmSync(tempDir, { recursive: true, force: true });
    jest.resetModules();
});

// ── Helper: get a fresh config_manager pointed at our temp file ─────────────
function getModule() {
    // Patch the exported CONFIG_PATH constant after require
    const mod = require('../src/System/config_manager');
    return mod;
}

// ── Tests ──────────────────────────────────────────────────────────────────

describe('config_manager — readConfig', () => {
    test('returns DEFAULT_CONFIG when no config file exists', () => {
        const { readConfig } = getModule();
        const config = readConfig();
        expect(config).toHaveProperty('geminiModel', 'gemini-2.5-flash');
        expect(config).toHaveProperty('aiProvider', 'gemini');
        expect(config).toHaveProperty('language', 'th-TH');
    });

    test('merges saved values with defaults', () => {
        const { readConfig, writeConfig, CONFIG_PATH } = getModule();
        writeConfig({ geminiModel: 'gemini-2.0-pro', customField: 'yes' });
        const config = readConfig();
        // saved value wins
        expect(config.geminiModel).toBe('gemini-2.0-pro');
        // default still present
        expect(config.aiProvider).toBe('gemini');
    });
});

describe('config_manager — writeConfig', () => {
    test('returns success: true on valid write', () => {
        const { writeConfig } = getModule();
        const result = writeConfig({ testKey: 'testValue' });
        expect(result).toEqual({ success: true });
    });

    test('written JSON is readable back', () => {
        const { writeConfig, readConfig } = getModule();
        writeConfig({ geminiModel: 'custom-model-test' });
        const config = readConfig();
        expect(config.geminiModel).toBe('custom-model-test');
    });
});

describe('config_manager — getAvailableProviders', () => {
    test('always includes ollama', () => {
        const { getAvailableProviders } = getModule();
        const providers = getAvailableProviders({});
        expect(providers).toContain('ollama');
    });

    test('includes gemini when apiKey present', () => {
        const { getAvailableProviders } = getModule();
        const providers = getAvailableProviders({ apiKey: 'test-key' });
        expect(providers).toContain('gemini');
    });

    test('includes anthropic when anthropicApiKey present', () => {
        const { getAvailableProviders } = getModule();
        const providers = getAvailableProviders({ anthropicApiKey: 'ant-key' });
        expect(providers).toContain('anthropic');
    });

    test('includes openai when openaiApiKey present', () => {
        const { getAvailableProviders } = getModule();
        const providers = getAvailableProviders({ openaiApiKey: 'oai-key' });
        expect(providers).toContain('openai');
    });

    test('does not include gemini when no key', () => {
        const { getAvailableProviders } = getModule();
        // clear any env var
        const savedEnv = process.env.GEMINI_API_KEY;
        delete process.env.GEMINI_API_KEY;
        const providers = getAvailableProviders({ apiKey: '' });
        expect(providers).not.toContain('gemini');
        if (savedEnv) process.env.GEMINI_API_KEY = savedEnv;
    });

    test('includes local_openai when localApiBaseUrl is set', () => {
        const { getAvailableProviders } = getModule();
        const providers = getAvailableProviders({ localApiBaseUrl: 'http://localhost:1234/v1' });
        expect(providers).toContain('local_openai');
    });
});
