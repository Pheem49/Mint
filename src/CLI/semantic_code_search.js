const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const { GoogleGenAI } = require('@google/genai');
const { readConfig } = require('../System/config_manager');
const { _helpers: symbolHelpers } = require('./symbol_indexer');

const DEFAULT_EMBEDDING_MODEL = 'gemini-embedding-001';
const DEFAULT_MAX_CHARS = 1800;
const DEFAULT_OVERLAP_LINES = 8;
const DEFAULT_MAX_FILE_BYTES = 512 * 1024;

function getStoreDir(options = {}) {
    const dir = path.join(os.homedir(), '.config', 'mint', 'semantic-code');
    if (options.create) {
        fs.mkdirSync(dir, { recursive: true });
    }
    return dir;
}

function getWorkspaceStorePath(root, options = {}) {
    const hash = crypto.createHash('sha1').update(path.resolve(root)).digest('hex').slice(0, 16);
    return path.join(getStoreDir(options), `${hash}.json`);
}

function resolveApiKey() {
    let settingsKey = '';
    try {
        settingsKey = (readConfig().apiKey || '').trim();
    } catch (_) {
        settingsKey = '';
    }
    return settingsKey || process.env.GEMINI_API_KEY || '';
}

async function defaultEmbedText(text, model = DEFAULT_EMBEDDING_MODEL) {
    const apiKey = resolveApiKey();
    if (!apiKey) {
        throw new Error('Gemini API key is required for semantic code embeddings.');
    }

    const client = new GoogleGenAI({ apiKey });
    const response = await client.models.embedContent({
        model,
        contents: text
    });
    return response.embeddings[0].values;
}

function hashFile(filePath) {
    return crypto.createHash('sha1').update(fs.readFileSync(filePath)).digest('hex');
}

function cosineSimilarity(vecA, vecB) {
    let dot = 0;
    let normA = 0;
    let normB = 0;
    const length = Math.min(vecA.length, vecB.length);

    for (let index = 0; index < length; index++) {
        const a = vecA[index];
        const b = vecB[index];
        dot += a * b;
        normA += a * a;
        normB += b * b;
    }

    if (normA === 0 || normB === 0) return 0;
    return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

function languageForFile(file) {
    const ext = path.extname(file).toLowerCase();
    if (ext === '.py') return 'Python';
    if (ext === '.rs') return 'Rust';
    if (ext === '.ts' || ext === '.tsx') return 'TypeScript';
    return 'JavaScript';
}

function chunkLines(lines, maxChars = DEFAULT_MAX_CHARS, overlapLines = DEFAULT_OVERLAP_LINES) {
    const chunks = [];
    let start = 0;

    while (start < lines.length) {
        let end = start;
        let charCount = 0;

        while (end < lines.length && (charCount + lines[end].length + 1 <= maxChars || end === start)) {
            charCount += lines[end].length + 1;
            end++;
        }

        chunks.push({
            startLine: start + 1,
            endLine: end,
            content: lines.slice(start, end).join('\n')
        });

        if (end >= lines.length) break;
        start = Math.max(end - overlapLines, start + 1);
    }

    return chunks;
}

function createCodeChunks(targetPath = process.cwd(), options = {}) {
    const root = path.resolve(targetPath);
    const stat = fs.statSync(root);
    if (!stat.isDirectory()) {
        throw new Error(`Semantic code search path is not a directory: ${root}`);
    }

    const files = symbolHelpers.walkSourceFiles(root, options);
    const maxFileBytes = options.maxFileBytes || DEFAULT_MAX_FILE_BYTES;
    const chunks = [];
    const fileHashes = {};

    for (const file of files) {
        const fullPath = path.join(root, file);
        let fileStat;
        try {
            fileStat = fs.statSync(fullPath);
        } catch (_) {
            continue;
        }
        if (!fileStat.isFile() || fileStat.size > maxFileBytes) continue;

        let content = '';
        try {
            content = fs.readFileSync(fullPath, 'utf8');
        } catch (_) {
            continue;
        }
        if (!content.trim()) continue;

        const language = languageForFile(file);
        const symbols = symbolHelpers.indexFileSymbols(root, file).map(symbol => symbol.name);
        fileHashes[file] = hashFile(fullPath);

        for (const chunk of chunkLines(content.split('\n'), options.maxChars || DEFAULT_MAX_CHARS, options.overlapLines || DEFAULT_OVERLAP_LINES)) {
            const header = [
                `File: ${file}`,
                `Language: ${language}`,
                symbols.length > 0 ? `Symbols: ${symbols.slice(0, 20).join(', ')}` : ''
            ].filter(Boolean).join('\n');

            chunks.push({
                id: `${file}:${chunk.startLine}-${chunk.endLine}`,
                file,
                language,
                startLine: chunk.startLine,
                endLine: chunk.endLine,
                symbols,
                text: `${header}\n\n${chunk.content}`
            });
        }
    }

    return { root, files, fileHashes, chunks };
}

function writeJson(filePath, data) {
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    const tmpPath = `${filePath}.tmp`;
    fs.writeFileSync(tmpPath, JSON.stringify(data, null, 2), 'utf8');
    fs.renameSync(tmpPath, filePath);
}

async function indexSemanticCode(targetPath = process.cwd(), options = {}) {
    const root = path.resolve(targetPath);
    const embedText = options.embedText || defaultEmbedText;
    const model = options.model || DEFAULT_EMBEDDING_MODEL;
    const storePath = options.storePath || getWorkspaceStorePath(root, { create: true });
    const prepared = createCodeChunks(root, options);
    const indexedChunks = [];

    for (let index = 0; index < prepared.chunks.length; index++) {
        const chunk = prepared.chunks[index];
        if (typeof options.onProgress === 'function') {
            options.onProgress({ current: index + 1, total: prepared.chunks.length, file: chunk.file });
        }
        const embedding = await embedText(chunk.text, model);
        indexedChunks.push({ ...chunk, embedding });
    }

    const payload = {
        version: 1,
        root: prepared.root,
        model,
        indexedAt: new Date().toISOString(),
        fileCount: prepared.files.length,
        chunkCount: indexedChunks.length,
        fileHashes: prepared.fileHashes,
        chunks: indexedChunks
    };
    writeJson(storePath, payload);

    return { ...payload, storePath };
}

function loadSemanticCodeIndex(targetPath = process.cwd(), options = {}) {
    const root = path.resolve(targetPath);
    const storePath = options.storePath || getWorkspaceStorePath(root);
    if (!fs.existsSync(storePath)) {
        throw new Error(`Semantic code index not found. Run "mint semantic-code index ${root}" first.`);
    }

    const payload = JSON.parse(fs.readFileSync(storePath, 'utf8'));
    if (path.resolve(payload.root) !== root) {
        throw new Error(`Semantic code index belongs to a different workspace: ${payload.root}`);
    }
    return { ...payload, storePath };
}

async function searchSemanticCode(query, targetPath = process.cwd(), options = {}) {
    const trimmedQuery = String(query || '').trim();
    if (!trimmedQuery) {
        throw new Error('Semantic code search query is required.');
    }

    const index = loadSemanticCodeIndex(targetPath, options);
    const embedText = options.embedText || defaultEmbedText;
    const topK = options.topK || 5;
    const queryEmbedding = await embedText(trimmedQuery, index.model || DEFAULT_EMBEDDING_MODEL);

    const results = index.chunks
        .filter(chunk => Array.isArray(chunk.embedding))
        .map(chunk => ({
            file: chunk.file,
            language: chunk.language,
            startLine: chunk.startLine,
            endLine: chunk.endLine,
            symbols: chunk.symbols || [],
            score: cosineSimilarity(queryEmbedding, chunk.embedding),
            text: chunk.text
        }))
        .sort((a, b) => b.score - a.score)
        .slice(0, topK);

    return {
        query: trimmedQuery,
        root: index.root,
        indexedAt: index.indexedAt,
        model: index.model,
        resultCount: results.length,
        results
    };
}

function formatSemanticCodeIndex(index) {
    return [
        '# Semantic Code Index',
        '',
        `Root: ${index.root}`,
        `Source files scanned: ${index.fileCount}`,
        `Chunks embedded: ${index.chunkCount}`,
        `Embedding model: ${index.model}`,
        `Index file: ${index.storePath}`
    ].join('\n');
}

function firstCodeLine(text) {
    return String(text || '')
        .split('\n')
        .map(line => line.trim())
        .find(line => line && !line.startsWith('File:') && !line.startsWith('Language:') && !line.startsWith('Symbols:')) || '';
}

function formatSemanticCodeSearch(results) {
    const lines = [
        '# Semantic Code Search',
        '',
        `Query: ${results.query}`,
        `Root: ${results.root}`,
        `Indexed at: ${results.indexedAt}`,
        ''
    ];

    if (!results.results.length) {
        lines.push('No matches found.');
        return lines.join('\n');
    }

    results.results.forEach((result, index) => {
        lines.push(`${index + 1}. ${result.file}:${result.startLine}-${result.endLine} (${result.score.toFixed(3)})`);
        if (result.symbols.length > 0) {
            lines.push(`   Symbols: ${result.symbols.slice(0, 8).join(', ')}`);
        }
        const preview = firstCodeLine(result.text);
        if (preview) {
            lines.push(`   ${preview}`);
        }
    });

    return lines.join('\n');
}

module.exports = {
    createCodeChunks,
    indexSemanticCode,
    loadSemanticCodeIndex,
    searchSemanticCode,
    formatSemanticCodeIndex,
    formatSemanticCodeSearch,
    _helpers: {
        chunkLines,
        cosineSimilarity,
        getWorkspaceStorePath,
        defaultEmbedText
    }
};
