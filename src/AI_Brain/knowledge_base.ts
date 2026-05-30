import * as fs from 'fs'
import * as path from 'path'
import * as os from 'os'
import * as crypto from 'crypto'
import { GoogleGenAI  } from '@google/genai'
const pdf = require('pdf-parse')
import mammoth from 'mammoth'
import readXlsxFile from 'read-excel-file/node'
import { readConfig  } from '../System/config_manager'

// Handle electron dependency safely
let app;
try {
    const electron = require('electron')
    app = electron.app;
} catch (e) {
    app = null;
}

let ai = null;
let activeApiKey = '';
let DatabaseSync = null;

function resolveApiKey() {
    let settingsKey = '';
    try {
        const cfg = readConfig();
        settingsKey = (cfg.apiKey || '').trim();
    } catch (e) {
        settingsKey = '';
    }
    const selectedKey = settingsKey || process.env.GEMINI_API_KEY || '';
    activeApiKey = selectedKey;
    return selectedKey;
}

function getAiClient() {
    const key = resolveApiKey();
    if (!ai || activeApiKey !== key) {
        ai = new GoogleGenAI({ apiKey: key });
    }
    return ai;
}

function getDbPath() {
    const fileName = 'mint-knowledge.sqlite';
    const configDir = path.join(os.homedir(), '.config', 'mint');
    const dbPath = path.join(configDir, fileName);

    if (!fs.existsSync(configDir)) {
        fs.mkdirSync(configDir, { recursive: true });
    }

    // Migration Logic
    if (!fs.existsSync(dbPath)) {
        const electronDb = app && app.getPath ? path.join(app.getPath('userData'), fileName) : null;
        const legacyDb = path.join(os.homedir(), '.mint', fileName);

        if (electronDb && fs.existsSync(electronDb)) {
            try {
                fs.copyFileSync(electronDb, dbPath);
                console.log('[RAG] Migrated database from Electron userData');
            } catch (e) { console.error('[RAG] Migration from Electron failed:', e); }
        } else if (fs.existsSync(legacyDb)) {
            try {
                fs.copyFileSync(legacyDb, dbPath);
                console.log('[RAG] Migrated database from ~/.mint');
            } catch (e) { console.error('[RAG] Migration from ~/.mint failed:', e); }
        }
    }

    return dbPath;
}

function getDatabaseSync() {
    if (!DatabaseSync) {
        ({ DatabaseSync } = require('node:sqlite'));
    }
    return DatabaseSync;
}

// Initialize Database
let dbInstance = null;
function getDb() {
    if (dbInstance) return dbInstance;
    const dbPath = getDbPath();
    const Database = getDatabaseSync();
    dbInstance = new Database(dbPath);

    // Enable WAL mode for better concurrency
    dbInstance.exec('PRAGMA journal_mode = WAL;');
    dbInstance.exec('PRAGMA synchronous = NORMAL;');

    // Create Tables
    dbInstance.exec(`
        -- Shared knowledge tables
        CREATE TABLE IF NOT EXISTS sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT UNIQUE,
            name TEXT,
            hash TEXT,
            last_indexed DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id INTEGER,
            text TEXT,
            embedding BLOB,
            FOREIGN KEY(source_id) REFERENCES sources(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source_id);

        -- Shared memory tables (ensuring consistency)
        CREATE TABLE IF NOT EXISTS user_profile (
            key        TEXT PRIMARY KEY,
            value      TEXT,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS session_memories (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            summary    TEXT NOT NULL,
            tags       TEXT DEFAULT '',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS usage_patterns (
            pattern   TEXT PRIMARY KEY,
            count     INTEGER DEFAULT 1,
            last_used DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS response_cache (
            query_hash TEXT PRIMARY KEY,
            response   TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
    `);
    return dbInstance;
}

async function generateEmbedding(text) {
    const client = getAiClient();
    const response = await client.models.embedContent({
        model: 'gemini-embedding-001',
        contents: text,
    });
    return response.embeddings[0].values;
}


function cosineSimilarity(vecA, vecB) {
    let dotProduct = 0, normA = 0, normB = 0;
    for (let i = 0; i < vecA.length; i++) {
        dotProduct += vecA[i] * vecB[i];
        normA += vecA[i] * vecA[i];
        normB += vecB[i] * vecB[i];
    }
    return dotProduct / (Math.sqrt(normA) * Math.sqrt(normB));
}

function getFileHash(filePath) {
    const content = fs.readFileSync(filePath);
    return crypto.createHash('md5').update(content).digest('hex');
}

function chunkText(text, maxChars = 1000, overlap = 200) {
    const chunks = [];
    let current = 0;
    while (current < text.length) {
        chunks.push(text.slice(current, current + maxChars));
        current += (maxChars - overlap);
        if (current >= text.length) break;
    }
    return chunks;
}

async function indexFile(filePath) {
    try {
        if (!fs.existsSync(filePath)) return `ไม่พบไฟล์: ${filePath}`;
        const stats = fs.statSync(filePath);
        if (stats.isDirectory()) return await indexFolder(filePath);
        if (stats.size > 10 * 1024 * 1024) return `ไฟล์ใหญ่เกินไป (> 10MB): ${filePath}`;

        const hash = getFileHash(filePath);
        const db = getDb();

        // Check if already indexed and unchanged
        const checkStmt = db.prepare("SELECT id, hash FROM sources WHERE path = ?");
        const existing = checkStmt.get(filePath);

        if (existing && existing.hash === hash) {
            return `⏩ ${path.basename(filePath)} ไม่มีการเปลี่ยนแปลง (ข้ามการอ่าน)`;
        }

        console.log(`[RAG] Indexing ${filePath}...`);
        let content = '';
        const ext = path.extname(filePath).toLowerCase();

        // Extraction logic
        if (ext === '.pdf') {
            const data = await pdf(fs.readFileSync(filePath));
            content = data.text;
        } else if (ext === '.docx') {
            const res = await mammoth.extractRawText({ path: filePath });
            content = res.value;
        } else if (ext === '.xlsx') {
            const sheets = await readXlsxFile(filePath);
            content = sheets
                .map(({ sheet, data }) => [
                    `Sheet: ${sheet}`,
                    ...data.map(row => row.map(value => value == null ? '' : String(value)).join(','))
                ].join('\n'))
                .join('\n');
        } else {
            content = fs.readFileSync(filePath, 'utf8');
        }

        if (!content.trim()) return `⚠️ ไฟล์ไม่มีข้อความ: ${filePath}`;

        // Begin transaction
        db.exec("BEGIN TRANSACTION");
        try {
            if (existing) {
                db.prepare("DELETE FROM chunks WHERE source_id = ?").run(existing.id);
                db.prepare("UPDATE sources SET hash = ?, last_indexed = CURRENT_TIMESTAMP WHERE id = ?").run(hash, existing.id);
            } else {
                db.prepare("INSERT INTO sources (path, name, hash) VALUES (?, ?, ?)").run(filePath, path.basename(filePath), hash);
            }
            
            const sourceId = existing ? existing.id : db.prepare("SELECT last_insert_rowid() as id").get().id;
            const chunks = chunkText(content);
            
            const insertChunk = db.prepare("INSERT INTO chunks (source_id, text, embedding) VALUES (?, ?, ?)");
            for (const chunk of chunks) {
                const embedding = await generateEmbedding(chunk);
                const embeddingBlob = Buffer.from(new Float32Array(embedding).buffer);
                insertChunk.run(sourceId, chunk, embeddingBlob);
            }
            db.exec("COMMIT");
            return `✅ Successfully indexed ${path.basename(filePath)} (${chunks.length} chunks)`;
        } catch (e) {
            db.exec("ROLLBACK");
            throw e;
        }
    } catch (err) {
        console.error('[RAG] Error:', err);
        return `❌ Failed to index: ${err.message}`;
    }
}

/**
 * Recursively gets all files in a directory asynchronously
 */
async function getAllFiles(dirPath, arrayOfFiles = []) {
    const files = await fs.promises.readdir(dirPath, { withFileTypes: true });
    
    for (const file of files) {
        const fullPath = path.join(dirPath, file.name);
        if (file.isDirectory()) {
            await getAllFiles(fullPath, arrayOfFiles);
        } else {
            arrayOfFiles.push(fullPath);
        }
    }
    return arrayOfFiles;
}

async function indexFolder(folderPath) {
    console.log(`[RAG] Indexing folder: ${folderPath}`);
    const files = await getAllFiles(folderPath);
    console.log(`[RAG] Found ${files.length} files to check.`);
    
    // Process in small batches to avoid blocking
    const BATCH_SIZE = 5;
    let indexedCount = 0;
    let skippedCount = 0;

    for (let i = 0; i < files.length; i += BATCH_SIZE) {
        const batch = files.slice(i, i + BATCH_SIZE);
        await Promise.all(batch.map(async (file) => {
            const res = await indexFile(file);
            if (res && res.startsWith('✅')) indexedCount++;
            else skippedCount++;
        }));
    }
    
    console.log(`[RAG] Indexing complete. ${indexedCount} new/updated, ${skippedCount} skipped.`);
    return `📂 Folder indexing complete: ${indexedCount} learned, ${skippedCount} skipped.`;
}

async function searchKnowledge(query, topK = 3) {
    const startTime = Date.now();
    const db = getDb();
    const MAX_CHUNKS_TO_SEARCH = 2000; // Limit search to keep it fast
    
    const countRes = db.prepare("SELECT COUNT(*) as count FROM chunks").get();
    if (!countRes || countRes.count === 0) return null;

    try {
        const queryVector = await generateEmbedding(query);
        const queryTyped = new Float32Array(queryVector);
        const results = [];

        // Search most recent or top chunks first, but limit the total scan
        const stmt = db.prepare("SELECT text, embedding, source_id FROM chunks LIMIT ?");
        let processed = 0;

        for (const c of stmt.iterate(MAX_CHUNKS_TO_SEARCH)) {
            if (!c.embedding) continue;
            processed++;
            
            const chunkVector = new Float32Array(c.embedding.buffer, c.embedding.byteOffset, c.embedding.byteLength / 4);
            
            let dotProduct = 0, normA = 0, normB = 0;
            for (let i = 0; i < queryTyped.length; i++) {
                const a = queryTyped[i];
                const b = chunkVector[i];
                dotProduct += a * b;
                normA += a * a;
                normB += b * b;
            }
            const score = dotProduct / (Math.sqrt(normA) * Math.sqrt(normB));

            if (score > 0.65) {
                results.push({ text: c.text, score, source_id: c.source_id });
            }
        }

        if (results.length > 0) {
            results.sort((a, b) => b.score - a.score);
            const top = results.slice(0, topK);
            
            const sourceIds = [...new Set(top.map(t => t.source_id))];
            const sources = db.prepare(`SELECT id, name FROM sources WHERE id IN (${sourceIds.join(',')})`).all();
            const sourceMap = Object.fromEntries(sources.map(s => [s.id, s.name]));
            
            console.log(`[RAG] Search took ${Date.now() - startTime}ms for ${processed} chunks.`);
            return top.map(t => ({
                text: t.text,
                source: sourceMap[t.source_id],
                score: t.score
            }));
        }
    } catch (e) {
        console.error("[RAG] Search Error:", e);
    }
    return null;
}


export { indexFile, indexFolder, searchKnowledge  }
