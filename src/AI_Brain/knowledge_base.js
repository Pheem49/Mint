const fs = require('fs');
const path = require('path');
const { GoogleGenAI } = require('@google/genai');
const { readConfig } = require('../System/config_manager');

// Handle electron dependency safely for benchmarks/tests
let app;
try {
    const electron = require('electron');
    app = electron.app;
} catch (e) {
    app = null;
}

let ai = null;
let activeApiKey = '';
const initialEnvKey = (process.env.GEMINI_API_KEY || '').trim();

function resolveApiKey() {
    let settingsKey = '';
    try {
        const cfg = readConfig();
        settingsKey = (cfg.apiKey || '').trim();
    } catch (e) {
        settingsKey = '';
    }

    const envKey = initialEnvKey;
    const selectedKey = settingsKey || envKey || '';

    if (selectedKey !== (process.env.GEMINI_API_KEY || '')) {
        process.env.GEMINI_API_KEY = selectedKey;
    }

    activeApiKey = selectedKey;
    return selectedKey;
}

function getAiClient() {
    const prevKey = activeApiKey;
    const nextKey = resolveApiKey();
    if (!ai || nextKey !== prevKey) {
        ai = new GoogleGenAI({});
    }
    return ai;
}

function getDbPath() {
    if (app && app.getPath) {
        return path.join(app.getPath('userData'), 'mint-knowledge.json');
    }
    // Fallback for Node.js environment (benchmarking)
    return path.join(process.cwd(), 'mint-knowledge.json');
}

function loadDb() {
    try {
        const p = getDbPath();
        if (fs.existsSync(p)) {
            return JSON.parse(fs.readFileSync(p, 'utf8'));
        }
    } catch (err) {
        console.error('[KnowledgeBase] Load Error:', err);
    }
    return { documents: [] };
}

function saveDb(db) {
    fs.writeFileSync(getDbPath(), JSON.stringify(db, null, 2));
}

async function generateEmbedding(text) {
    const client = getAiClient();
    const response = await client.models.embedContent({
        model: 'gemini-embedding-001',
        contents: text,
    });
    // The google/genai package returns an array of embeddings
    return response.embeddings[0].values;
}

function cosineSimilarity(vecA, vecB) {
    let dotProduct = 0.0;
    let normA = 0.0;
    let normB = 0.0;
    for (let i = 0; i < vecA.length; i++) {
        dotProduct += vecA[i] * vecB[i];
        normA += vecA[i] * vecA[i];
        normB += vecB[i] * vecB[i];
    }
    if (normA === 0 || normB === 0) return 0;
    return dotProduct / (Math.sqrt(normA) * Math.sqrt(normB));
}

function chunkText(text, maxChars = 1000, overlap = 200) {
    const chunks = [];
    let current = 0;
    const step = maxChars - overlap;
    while (current < text.length) {
        chunks.push(text.slice(current, current + maxChars));
        current += step;
    }
    return chunks;
}

/**
 * Reads a local file, chunks it, generates embeddings, and saves to knowledge base.
 */
async function indexFile(filePath) {
    try {
        if (!fs.existsSync(filePath)) return `ไม่พบไฟล์: ${filePath}`;
        
        const stats = fs.statSync(filePath);
        if (stats.size > 2 * 1024 * 1024) return `ขนาดไฟล์ใหญ่เกินไป (> 2MB): ${filePath}`;
        
        const content = fs.readFileSync(filePath, 'utf8');
        if (!content || content.trim().length === 0) return `ไฟล์ว่างเปล่า: ${filePath}`;
        
        const chunks = chunkText(content);
        const db = loadDb();
        
        for (let i = 0; i < chunks.length; i++) {
            const embedding = await generateEmbedding(chunks[i]);
            db.documents.push({
                id: `${filePath}#${i}-${Date.now()}`,
                source: path.basename(filePath),
                path: filePath,
                text: chunks[i],
                embedding
            });
        }
        
        saveDb(db);
        return `✅ เรียนรู้ข้อมูลจากไฟล์ ${path.basename(filePath)} เรียบร้อยแล้ว (แบ่งเป็น ${chunks.length} ส่วน)`;
    } catch (err) {
        console.error('[KnowledgeBase] Indexing error:', err);
        return `❌ เกิดข้อผิดพลาดในการเรียนรู้ไฟล์: ${err.message}`;
    }
}

/**
 * Searches the local knowledge base for relevant chunks.
 */
async function searchKnowledge(query, topK = 3) {
    const db = loadDb();
    if (!db.documents || db.documents.length === 0) return null;
    
    try {
        const queryVector = await generateEmbedding(query);
        const results = db.documents.map(doc => ({
            ...doc,
            score: cosineSimilarity(queryVector, doc.embedding)
        })).sort((a, b) => b.score - a.score);
        
        // Return top results above a threshold
        const top = results.slice(0, topK).filter(r => r.score > 0.65);
        if (top.length > 0) {
            console.log(`[KnowledgeBase] Found ${top.length} matches for query.`);
            return top;
        }
    } catch(err) {
        console.error("[KnowledgeBase] Search error:", err);
    }
    return null;
}

module.exports = { indexFile, searchKnowledge };
