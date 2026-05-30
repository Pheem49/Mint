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
    memStore = require('../dist/src/AI_Brain/memory_store');
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

    test('records thai user name when explicitly introduced', () => {
        memStore.recordInteraction('ผมชื่อภีมนะ', 'ยินดีที่ได้รู้จักค่ะ');
        expect(memStore.getProfile('user_name')).toBe('ภีม');
    });

    test('records english user name when explicitly introduced', () => {
        memStore.recordInteraction('my name is Pheem', 'nice to meet you');
        expect(memStore.getProfile('user_name')).toBe('Pheem');
    });

    test('increments total_interactions counter', () => {
        memStore.recordInteraction('msg 1', 'reply 1');
        memStore.recordInteraction('msg 2', 'reply 2');
        expect(memStore.getProfile('total_interactions')).toBe('2');
    });

    test('stores every interaction as episodic memory', () => {
        memStore.recordInteraction('ผมชอบใช้ Pop!_OS', 'จำไว้แล้วค่ะ');
        const interactions = memStore.getRecentInteractions(5);
        expect(interactions.length).toBe(1);
        expect(interactions[0].user_text).toContain('Pop!_OS');
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

// ── Interaction memory tests ───────────────────────────────────────────────

describe('memory_store — interaction memories', () => {
    test('searches stored interactions by keyword', () => {
        memStore.recordInteraction('ผมกำลังทำโปรเจกต์ Mint agent', 'รับทราบค่ะ');
        memStore.recordInteraction('วันนี้อากาศร้อน', 'ใช่ค่ะ');

        const results = memStore.searchInteractions('Mint agent', 5);
        expect(results.length).toBeGreaterThan(0);
        expect(results[0].user_text).toContain('Mint agent');
    });

    test('returns recent interactions newest first', () => {
        memStore.recordInteraction('first memory', 'one');
        memStore.recordInteraction('second memory', 'two');

        const interactions = memStore.getRecentInteractions(2);
        expect(interactions[0].user_text).toBe('second memory');
        expect(interactions[1].user_text).toBe('first memory');
    });

    test('clears interaction memories', () => {
        memStore.recordInteraction('remember this', 'ok');
        memStore.clearInteractionMemories();
        expect(memStore.getRecentInteractions(5)).toEqual([]);
    });

    test('deletes one interaction memory by id', () => {
        memStore.recordInteraction('delete this one', 'ok');
        const [memory] = memStore.getRecentInteractions(1);
        expect(memStore.deleteInteractionMemory(memory.id)).toBe(true);
        expect(memStore.getRecentInteractions(5)).toEqual([]);
    });

    test('exports memory snapshot', () => {
        memStore.recordInteraction('export this memory', 'ok');
        const snapshot = memStore.exportMemorySnapshot();
        expect(snapshot.profile.total_interactions).toBe('1');
        expect(snapshot.interaction_memories[0].user_text).toBe('export this memory');
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

    test('includes user name in context output', () => {
        memStore.recordInteraction('ผมชื่อภีมนะ', 'จำชื่อไว้แล้วค่ะ');
        const ctx = memStore.getUserContext();
        expect(ctx).toContain('User name: ภีม');
    });

    test('includes total_interactions in context', () => {
        memStore.recordInteraction('hello', 'hi');
        const ctx = memStore.getUserContext();
        expect(ctx).toContain('1');
    });

    test('includes recent remembered interactions in context', () => {
        memStore.recordInteraction('ผมชอบธีมสีเขียว', 'จำไว้แล้วค่ะ');
        const ctx = memStore.getUserContext();
        expect(ctx).toContain('Recent remembered interactions');
        expect(ctx).toContain('ผมชอบธีมสีเขียว');
    });

    test('includes relevant remembered interactions for a query', () => {
        memStore.recordInteraction('ผมใช้ editor ชื่อ Cursor', 'จำไว้แล้วค่ะ');
        memStore.recordInteraction('ผมชอบกาแฟเย็น', 'จำไว้แล้วค่ะ');

        const ctx = memStore.getUserContext('ผมใช้ editor อะไร');
        expect(ctx).toContain('Relevant remembered interactions');
        expect(ctx).toContain('Cursor');
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

// ── Response Cache tests ──────────────────────────────────────────────────

describe('memory_store — response cache', () => {
    test('stores and retrieves a response', () => {
        const query = 'What is 2+2?';
        const response = { response: '4', action: { type: 'none' } };
        memStore.cacheResponse(query, response);
        
        const cached = memStore.getCachedResponse(query);
        expect(cached.response).toBe('4');
    });

    test('returns null for missing query', () => {
        expect(memStore.getCachedResponse('where is my cat?')).toBe(null);
    });

    test('is case-insensitive and ignores whitespace', () => {
        const query = '  Hello World  ';
        const response = { response: 'Hi!', action: { type: 'none' } };
        memStore.cacheResponse(query, response);
        
        const cached = memStore.getCachedResponse('hello world');
        expect(cached.response).toBe('Hi!');
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
