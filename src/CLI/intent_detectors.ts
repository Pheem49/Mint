'use strict';

// ---------------------------------------------------------------------------
// Repository Summary
// ---------------------------------------------------------------------------

/**
 * Returns true when the user's plain-language input is asking for a repo summary.
 * @param {string} text
 * @returns {boolean}
 */
function isRepoSummaryRequest(text) {
    const input = (text || '').trim().toLowerCase();
    if (!input) return false;

    const hasRepoTarget = /\b(repo|repository|project|workspace|codebase)\b|รีโป|โปรเจค|โปรเจ็กต์|โปรเจกต์|โปรเจ็ค/.test(input);
    const asksSummary   = /\b(summarize|summary|overview)\b|สรุป|ภาพรวม/.test(input);
    const asksQuestion  = /\b(have|has|มีไหม|มีมั้ย|มีหรือเปล่า)\b/.test(input);

    return hasRepoTarget && asksSummary && !asksQuestion;
}

/**
 * Parses raw CLI args for the summarize tool.
 * @param {string} rawArgs
 * @returns {{ targetPath: string, json: boolean }}
 */
function parseRepoSummaryArgs(rawArgs) {
    const args     = (rawArgs || '').split(/\s+/).filter(Boolean);
    const json     = args.includes('--json');
    const pathArgs = args.filter(arg => arg !== '--json');
    return {
        targetPath: pathArgs.length > 0 ? pathArgs.join(' ') : process.cwd(),
        json
    };
}

// ---------------------------------------------------------------------------
// Symbol Index
// ---------------------------------------------------------------------------

/**
 * Returns true when the user's input is asking for a symbol index.
 * @param {string} text
 * @returns {boolean}
 */
function isSymbolIndexRequest(text) {
    const input = (text || '').trim().toLowerCase();
    if (!input) return false;

    const hasSymbolTarget      = /\b(symbol|symbols|ast|lsp)\b|ซิมโบล|สัญลักษณ์/.test(input);
    const asksIndex            = /\b(index|list|show|build|scan|overview)\b|ทำ|สร้าง|แสดง|ลิสต์|สแกน/.test(input);
    const referencesWorkspace  = /\b(repo|repository|project|workspace|codebase|source|code)\b|รีโป|โปรเจค|โปรเจ็กต์|โปรเจกต์|โค้ด/.test(input);
    const asksQuestion         = /\b(do i|have|has)\b|มีไหม|มีมั้ย|มีหรือเปล่า/.test(input);

    return hasSymbolTarget && (asksIndex || referencesWorkspace) && !asksQuestion;
}

/**
 * Parses raw CLI args for the symbol index tool.
 * @param {string} rawArgs
 * @returns {{ targetPath: string, json: boolean, limit: number }}
 */
function parseSymbolIndexArgs(rawArgs) {
    const args     = (rawArgs || '').split(/\s+/).filter(Boolean);
    const json     = args.includes('--json');
    let limit      = 80;
    const pathArgs = [];

    for (let i = 0; i < args.length; i++) {
        const arg = args[i];
        if (arg === '--json') continue;
        if (arg === '--limit') {
            const next = Number(args[i + 1]);
            if (Number.isFinite(next) && next >= 0) { limit = next; i++; }
            continue;
        }
        if (arg.startsWith('--limit=')) {
            const next = Number(arg.slice('--limit='.length));
            if (Number.isFinite(next) && next >= 0) limit = next;
            continue;
        }
        pathArgs.push(arg);
    }

    return {
        targetPath: pathArgs.length > 0 ? pathArgs.join(' ') : process.cwd(),
        json,
        limit
    };
}

// ---------------------------------------------------------------------------
// Semantic Code Search
// ---------------------------------------------------------------------------

/**
 * Returns true when the user's input is asking for a semantic code search.
 * @param {string} text
 * @returns {boolean}
 */
function isSemanticCodeSearchRequest(text) {
    const input = (text || '').trim().toLowerCase();
    if (!input) return false;

    const hasSemanticSearch   = /\bsemantic\b/.test(input) && /\b(search|find|look for)\b/.test(input);
    const referencesCode      = /\b(code|repo|repository|project|workspace|codebase|source)\b|โค้ด|รีโป|โปรเจค|โปรเจ็กต์|โปรเจกต์/.test(input);
    const thaiSemanticSearch  = /ค้นหา/.test(input) && /ความหมาย|semantic/.test(input) && /โค้ด|โปรเจค|รีโป/.test(input);
    const asksQuestion        = /\b(do i|have|has)\b|มีไหม|มีมั้ย|มีหรือเปล่า/.test(input);

    return (hasSemanticSearch && referencesCode || thaiSemanticSearch) && !asksQuestion;
}

/**
 * Parses raw CLI args for the semantic code search tool.
 * @param {string} rawArgs
 * @returns {{ mode: string, query: string, targetPath: string, json: boolean, topK: number }}
 */
function parseSemanticCodeArgs(rawArgs) {
    const args      = (rawArgs || '').split(/\s+/).filter(Boolean);
    const json      = args.includes('--json');
    let topK        = 5;
    const pathArgs  = [];
    const queryArgs = [];
    let mode        = 'search';

    for (let i = 0; i < args.length; i++) {
        const arg = args[i];
        if (arg === 'index' || arg === 'search') { mode = arg; continue; }
        if (arg === '--json') continue;
        if (arg === '--top-k' || arg === '--limit') {
            const next = Number(args[i + 1]);
            if (Number.isFinite(next) && next > 0) { topK = next; i++; }
            continue;
        }
        if (arg.startsWith('--top-k=') || arg.startsWith('--limit=')) {
            const next = Number(arg.slice(arg.indexOf('=') + 1));
            if (Number.isFinite(next) && next > 0) topK = next;
            continue;
        }
        if (arg === '--path') {
            if (args[i + 1]) { pathArgs.push(args[i + 1]); i++; }
            continue;
        }
        queryArgs.push(arg);
    }

    return {
        mode,
        query:      queryArgs.join(' ').trim(),
        targetPath: pathArgs.length > 0 ? pathArgs.join(' ') : process.cwd(),
        json,
        topK
    };
}

/**
 * Strips intent phrases from text to extract the raw search query.
 * @param {string} text
 * @returns {string}
 */
function extractSemanticCodeQuery(text) {
    return String(text || '')
        .replace(/semantic\s+code\s+search/ig, '')
        .replace(/semantic\s+search/ig, '')
        .replace(/search\s+code/ig, '')
        .replace(/ค้นหาโค้ดแบบความหมาย/g, '')
        .replace(/ค้นหาแบบ semantic/g, '')
        .replace(/ใน repo นี้|ในโปรเจคนี้|ในรีโปนี้/g, '')
        .trim();
}

module.exports = {
    isRepoSummaryRequest,
    parseRepoSummaryArgs,
    isSymbolIndexRequest,
    parseSymbolIndexArgs,
    isSemanticCodeSearchRequest,
    parseSemanticCodeArgs,
    extractSemanticCodeQuery
};
