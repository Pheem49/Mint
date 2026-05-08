/**
 * Tests: memory_store.js
 * Tests profile CRUD, usage pattern recording, and getUserContext output.
 * Uses an isolated in-memory DB path via MINT_TEST_DB env var.
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

// ── Isolate DB per test run ─────────────────────────────────────────────────
let tempDir;
let memStore;

beforeEach(() => {
    jest.resetModules();

    // Create isolated temp directory
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-mem-test-'));

    // Patch os.homedir so memory_store uses our temp .mint dir
    jest.spyOn(os, 'homedir').mockReturnValue(tempDir);

    // Require fresh instance (no cache)
    memStore = require('../src/AI_Brain/memory_store');
});

afterEach(() => {
    jest.restoreAllMocks();
    jest.resetModules();
    fs.rmSync(tempDir, { recursive: true, force: true });
});

// ── Profile tests ──────────────────────────────────────────────────────────

describe('memory_store — setProfile / getProfile', () => {
    test('stores and retrieves a string value', () => {
        memStore.setProfile('test_key', 'hello');
        expect(memStore.getProfile('test_key')).toBe('hello');
    });

    test('returns defaultValue when key does not exist', () => {
        expect(memStore.getProfile('nonexistent', 'default_val')).toBe('default_val');
    });

    test('overwrites existing key', () => {
        memStore.setProfile('key', 'first');
        memStore.setProfile('key', 'second');
        expect(memStore.getProfile('key')).toBe('second');
    });

    test('stores numeric value as string', () => {
        memStore.setProfile('count', 42);
        expect(memStore.getProfile('count')).toBe('42');
    });
});

// ── recordInteraction tests ────────────────────────────────────────────────

describe('memory_store — recordInteraction', () => {
    test('records preferred language as thai for thai text', () => {
        memStore.recordInteraction('ช่วยเขียนโค้ดให้หน่อยนะคะ', 'โค้ดค่ะ');
        expect(memStore.getProfile('preferred_language')).toBe('thai');
    });

    test('records preferred language as english for english text', () => {
        memStore.recordInteraction('please help me write a function', 'sure!');
        expect(memStore.getProfile('preferred_language')).toBe('english');
    });

    test('increments total_interactions counter', () => {
        memStore.recordInteraction('msg 1', 'reply 1');
        memStore.recordInteraction('msg 2', 'reply 2');
        expect(memStore.getProfile('total_interactions')).toBe('2');
    });

    test('records coding keywords as patterns', () => {
        memStore.recordInteraction('fix the Python script bug', 'sure!');
        const patterns = memStore.getTopPatterns(10).map(p => p.pattern);
        // 'python' and 'script' are keywords extracted (stop words removed)
        expect(patterns.some(p => ['python', 'script', 'fix'].includes(p))).toBe(true);
    });

    test('does not throw on empty message', () => {
        expect(() => memStore.recordInteraction('', '')).not.toThrow();
    });
});

// ── getTopPatterns tests ───────────────────────────────────────────────────

describe('memory_store — getTopPatterns', () => {
    test('returns patterns sorted by count descending', () => {
        memStore.recordInteraction('python python python', 'ok');
        memStore.recordInteraction('python javascript', 'ok');
        const patterns = memStore.getTopPatterns(5);
        const patternNames = patterns.map(p => p.pattern);
        expect(patternNames[0]).toBe('python'); // python used most
    });

    test('returns empty array when no interactions recorded', () => {
        expect(memStore.getTopPatterns(5)).toEqual([]);
    });
});

// ── getUserContext tests ───────────────────────────────────────────────────

describe('memory_store — getUserContext', () => {
    test('returns empty string when no data', () => {
        expect(memStore.getUserContext()).toBe('');
    });

    test('includes preferred_language in context output', () => {
        memStore.recordInteraction('สวัสดีครับ', 'สวัสดีค่ะ');
        const ctx = memStore.getUserContext();
        expect(ctx).toContain('thai');
    });

    test('includes total_interactions in context', () => {
        memStore.recordInteraction('hello', 'hi');
        const ctx = memStore.getUserContext();
        expect(ctx).toContain('1');
    });

    test('context starts with the user context header', () => {
        memStore.setProfile('preferred_language', 'english');
        const ctx = memStore.getUserContext();
        expect(ctx).toContain('[LONG-TERM USER CONTEXT');
    });

    test('context ends with END marker', () => {
        memStore.setProfile('preferred_language', 'thai');
        const ctx = memStore.getUserContext();
        expect(ctx).toContain('[END USER CONTEXT]');
    });
});

// ── saveSessionSummary / getRecentMemories tests ───────────────────────────

describe('memory_store — session summaries', () => {
    test('saves and retrieves a summary', () => {
        memStore.saveSessionSummary('User asked about Python scripting', ['python', 'coding']);
        const memories = memStore.getRecentMemories(5);
        expect(memories.length).toBe(1);
        expect(memories[0].summary).toBe('User asked about Python scripting');
    });

    test('ignores too-short summaries', () => {
        memStore.saveSessionSummary('ok', []);
        const memories = memStore.getRecentMemories(5);
        expect(memories.length).toBe(0);
    });

    test('returns most recent summaries first', () => {
        memStore.saveSessionSummary('First session about web development', []);
        memStore.saveSessionSummary('Second session about machine learning', []);
        const memories = memStore.getRecentMemories(5);
        expect(memories[0].summary).toContain('Second');
    });
});
