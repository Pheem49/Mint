const fs = require('fs');
const os = require('os');
const path = require('path');

const {
    createCodeChunks,
    indexSemanticCode,
    searchSemanticCode,
    formatSemanticCodeIndex,
    formatSemanticCodeSearch,
    _helpers
} = require('../src/CLI/semantic_code_search');

function fakeEmbed(text) {
    const lower = String(text || '').toLowerCase();
    return [
        lower.includes('approval') || lower.includes('approve') ? 1 : 0,
        lower.includes('calendar') ? 1 : 0,
        lower.includes('search') ? 1 : 0
    ];
}

describe('semantic_code_search', () => {
    let tempDir;
    let storePath;

    beforeEach(() => {
        tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-semantic-code-'));
        storePath = path.join(tempDir, 'semantic-index.json');
        fs.mkdirSync(path.join(tempDir, 'src'));
        fs.writeFileSync(path.join(tempDir, 'src', 'approval.js'), [
            'function requestApproval(action) {',
            '  return action.approved === true;',
            '}'
        ].join('\n'));
        fs.writeFileSync(path.join(tempDir, 'src', 'calendar.js'), [
            'function createCalendarEvent(summary) {',
            '  return { summary };',
            '}'
        ].join('\n'));
    });

    afterEach(() => {
        fs.rmSync(tempDir, { recursive: true, force: true });
    });

    test('creates code chunks with file metadata', () => {
        const prepared = createCodeChunks(tempDir, { maxChars: 120 });

        expect(prepared.root).toBe(tempDir);
        expect(prepared.chunks).toEqual(expect.arrayContaining([
            expect.objectContaining({
                file: path.join('src', 'approval.js'),
                text: expect.stringContaining('File: src/approval.js')
            })
        ]));
    });

    test('indexes and searches code semantically with injected embeddings', async () => {
        const index = await indexSemanticCode(tempDir, {
            storePath,
            embedText: fakeEmbed,
            maxChars: 200
        });
        const results = await searchSemanticCode('approval flow', tempDir, {
            storePath,
            embedText: fakeEmbed,
            topK: 1
        });

        expect(index.chunkCount).toBeGreaterThan(0);
        expect(results.results[0]).toEqual(expect.objectContaining({
            file: path.join('src', 'approval.js')
        }));
        expect(results.results[0].score).toBeGreaterThan(0.9);
    });

    test('formats index and search output', async () => {
        const index = await indexSemanticCode(tempDir, {
            storePath,
            embedText: fakeEmbed,
            maxChars: 200
        });
        const results = await searchSemanticCode('calendar', tempDir, {
            storePath,
            embedText: fakeEmbed,
            topK: 1
        });

        expect(formatSemanticCodeIndex(index)).toContain('# Semantic Code Index');
        expect(formatSemanticCodeSearch(results)).toContain('src/calendar.js');
    });

    test('computes cosine similarity', () => {
        expect(_helpers.cosineSimilarity([1, 0], [1, 0])).toBe(1);
        expect(_helpers.cosineSimilarity([1, 0], [0, 1])).toBe(0);
    });
});
