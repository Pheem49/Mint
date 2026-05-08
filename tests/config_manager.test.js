/**
 * Tests: config_manager.js
 * Tests readConfig, writeConfig, and getAvailableProviders.
 *
 * Isolation strategy: spy on os.homedir() BEFORE requiring config_manager
 * so CONFIG_DIR and CONFIG_PATH point to a temp directory, never touching
 * the real ~/.config/mint/mint-config.json.
 */

const fs   = require('fs');
const path = require('path');
const os   = require('os');

let tempDir;

beforeEach(() => {
    // 1. Reset module registry so config_manager re-initialises fresh each test
    jest.resetModules();

    // 2. Create an isolated temp dir for this test
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-cfg-test-'));

    // 3. Mock os.homedir() BEFORE requiring config_manager.
    //    config_manager computes CONFIG_DIR = os.homedir() + '/.config/mint'
    //    at load time, so the spy must be active when the module first loads.
    jest.spyOn(os, 'homedir').mockReturnValue(tempDir);
});

afterEach(() => {
    jest.restoreAllMocks();
    jest.resetModules();
    fs.rmSync(tempDir, { recursive: true, force: true });
});

// Helper — always gets a fresh, isolated instance of config_manager
function getModule() {
    return require('../src/System/config_manager');
}

// ── readConfig ─────────────────────────────────────────────────────────────

describe('config_manager — readConfig', () => {
    test('returns DEFAULT_CONFIG when no config file exists', () => {
        const { readConfig } = getModule();
        const config = readConfig();
        expect(config).toHaveProperty('geminiModel', 'gemini-2.5-flash');
        expect(config).toHaveProperty('aiProvider', 'gemini');
        expect(config).toHaveProperty('language', 'th-TH');
    });

    test('merges saved values with defaults (saved value wins)', () => {
        const { readConfig, writeConfig } = getModule();
        writeConfig({ geminiModel: 'gemini-2.0-pro' });
        const config = readConfig();
        expect(config.geminiModel).toBe('gemini-2.0-pro');
        // Default fields still present
        expect(config.aiProvider).toBe('gemini');
        expect(config.language).toBe('th-TH');
    });

    test('missing keys in saved file are filled by defaults', () => {
        const { readConfig, writeConfig } = getModule();
        // Write a partial config (no 'ollamaModel' key)
        writeConfig({ geminiModel: 'my-model' });
        const config = readConfig();
        expect(config.ollamaModel).toBe('llama3:latest'); // default
    });
});

// ── writeConfig ────────────────────────────────────────────────────────────

describe('config_manager — writeConfig', () => {
    test('returns { success: true } on valid write', () => {
        const { writeConfig } = getModule();
        const result = writeConfig({ testKey: 'testValue' });
        expect(result).toEqual({ success: true });
    });

    test('written JSON is readable back correctly', () => {
        const { readConfig, writeConfig } = getModule();
        writeConfig({ geminiModel: 'custom-model-test' });
        const config = readConfig();
        expect(config.geminiModel).toBe('custom-model-test');
    });

    test('config file is actually created on disk', () => {
        const { writeConfig, CONFIG_PATH } = getModule();
        writeConfig({ geminiModel: 'test' });
        expect(fs.existsSync(CONFIG_PATH)).toBe(true);
    });
});

// ── getAvailableProviders ──────────────────────────────────────────────────

describe('config_manager — getAvailableProviders', () => {
    test('always includes ollama (local, no key needed)', () => {
        const { getAvailableProviders } = getModule();
        expect(getAvailableProviders({})).toContain('ollama');
    });

    test('includes gemini when apiKey is set', () => {
        const { getAvailableProviders } = getModule();
        expect(getAvailableProviders({ apiKey: 'test-key' })).toContain('gemini');
    });

    test('includes anthropic when anthropicApiKey is set', () => {
        const { getAvailableProviders } = getModule();
        expect(getAvailableProviders({ anthropicApiKey: 'ant-key' })).toContain('anthropic');
    });

    test('includes openai when openaiApiKey is set', () => {
        const { getAvailableProviders } = getModule();
        expect(getAvailableProviders({ openaiApiKey: 'oai-key' })).toContain('openai');
    });

    test('includes huggingface when hfApiKey is set', () => {
        const { getAvailableProviders } = getModule();
        expect(getAvailableProviders({ hfApiKey: 'hf-key' })).toContain('huggingface');
    });

    test('includes local_openai when localApiBaseUrl is set', () => {
        const { getAvailableProviders } = getModule();
        expect(
            getAvailableProviders({ localApiBaseUrl: 'http://localhost:1234/v1' })
        ).toContain('local_openai');
    });

    test('does NOT include gemini when no key', () => {
        const { getAvailableProviders } = getModule();
        const savedEnv = process.env.GEMINI_API_KEY;
        delete process.env.GEMINI_API_KEY;
        const providers = getAvailableProviders({ apiKey: '' });
        expect(providers).not.toContain('gemini');
        if (savedEnv) process.env.GEMINI_API_KEY = savedEnv;
    });

    test('returns array type', () => {
        const { getAvailableProviders } = getModule();
        expect(Array.isArray(getAvailableProviders({}))).toBe(true);
    });
});
