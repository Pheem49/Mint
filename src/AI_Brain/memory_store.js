/**
 * Mint Long-Term Memory Store
 * ---------------------------
 * Persists user preferences, session summaries, and usage patterns
 * across all Mint sessions using SQLite (same DB as knowledge_base).
 *
 * Auto-injects a "User Context" block into the system prompt so Mint
 * remembers who it's talking to even after restart.
 */

const fs = require('fs');
const path = require('path');
const os = require('os');
const crypto = require('crypto');
const { readConfig } = require('../System/config_manager');

// ── Electron-safe app path ──────────────────────────────────────────────────
let app;
try {
    const electron = require('electron');
    app = electron.app;
} catch (_) {
    app = null;
}

function getDbPath() {
    const fileName = 'mint-knowledge.sqlite'; // shared DB with knowledge_base
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
                console.log('[Memory] Migrated database from Electron userData');
            } catch (e) { console.error('[Memory] Migration from Electron failed:', e); }
        } else if (fs.existsSync(legacyDb)) {
            try {
                fs.copyFileSync(legacyDb, dbPath);
                console.log('[Memory] Migrated database from ~/.mint');
            } catch (e) { console.error('[Memory] Migration from ~/.mint failed:', e); }
        }
    }

    return dbPath;
}

// ── Lazy DatabaseSync init ─────────────────────────────────────────────────
let DatabaseSync = null;
function getDatabaseSync() {
    if (!DatabaseSync) ({ DatabaseSync } = require('node:sqlite'));
    return DatabaseSync;
}

let dbInstance = null;
function getDb() {
    if (dbInstance) return dbInstance;
    const Database = getDatabaseSync();
    dbInstance = new Database(getDbPath());
    
    // Enable WAL mode for better concurrency
    dbInstance.exec('PRAGMA journal_mode = WAL;');
    dbInstance.exec('PRAGMA synchronous = NORMAL;');

    dbInstance.exec(`
        -- User profile: arbitrary key-value pairs
        CREATE TABLE IF NOT EXISTS user_profile (
            key        TEXT PRIMARY KEY,
            value      TEXT,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Condensed summaries of past sessions
        CREATE TABLE IF NOT EXISTS session_memories (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            summary    TEXT NOT NULL,
            tags       TEXT DEFAULT '',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Frequently used topics / commands
        CREATE TABLE IF NOT EXISTS usage_patterns (
            pattern   TEXT PRIMARY KEY,
            count     INTEGER DEFAULT 1,
            last_used DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Response Cache: For repetitive exact queries
        CREATE TABLE IF NOT EXISTS response_cache (
            query_hash TEXT PRIMARY KEY,
            response   TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
    `);

    return dbInstance;
}

// ── Profile helpers ────────────────────────────────────────────────────────
function setProfile(key, value) {
    try {
        const db = getDb();
        db.prepare(`
            INSERT INTO user_profile (key, value, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
        `).run(key, String(value));
    } catch (err) {
        console.error('[Memory] setProfile error:', err.message);
    }
}

function getProfile(key, defaultValue = null) {
    try {
        const row = getDb().prepare('SELECT value FROM user_profile WHERE key = ?').get(key);
        return row ? row.value : defaultValue;
    } catch (_) {
        return defaultValue;
    }
}

function getAllProfile() {
    try {
        const rows = getDb().prepare('SELECT key, value FROM user_profile').all();
        return Object.fromEntries(rows.map(r => [r.key, r.value]));
    } catch (_) {
        return {};
    }
}

// ── Session memory helpers ─────────────────────────────────────────────────
const MAX_SESSION_MEMORIES = 20; // keep last N summaries

function addSessionMemory(summary, tags = []) {
    try {
        const db = getDb();
        db.prepare('INSERT INTO session_memories (summary, tags) VALUES (?, ?)').run(
            summary.slice(0, 800), // cap length
            tags.join(',')
        );
        // Prune oldest beyond limit
        db.exec(`
            DELETE FROM session_memories WHERE id NOT IN (
                SELECT id FROM session_memories ORDER BY id DESC LIMIT ${MAX_SESSION_MEMORIES}
            )
        `);
    } catch (err) {
        console.error('[Memory] addSessionMemory error:', err.message);
    }
}

function getRecentMemories(limit = 5) {
    try {
        return getDb()
            .prepare('SELECT summary, tags, created_at FROM session_memories ORDER BY id DESC LIMIT ?')
            .all(limit);
    } catch (_) {
        return [];
    }
}

// ── Usage pattern helpers ──────────────────────────────────────────────────
function recordPattern(pattern) {
    try {
        const db = getDb();
        db.prepare(`
            INSERT INTO usage_patterns (pattern, count, last_used)
            VALUES (?, 1, CURRENT_TIMESTAMP)
            ON CONFLICT(pattern) DO UPDATE
            SET count = count + 1, last_used = CURRENT_TIMESTAMP
        `).run(pattern.slice(0, 120));
    } catch (_) {}
}

function getTopPatterns(limit = 8) {
    try {
        return getDb()
            .prepare('SELECT pattern, count FROM usage_patterns ORDER BY count DESC, last_used DESC LIMIT ?')
            .all(limit);
    } catch (_) {
        return [];
    }
}

// ── Simple keyword extractor (no external deps) ────────────────────────────
const STOP_WORDS = new Set([
    'ที่', 'ให้', 'และ', 'ของ', 'กับ', 'ใน', 'บน', 'เป็น', 'อยู่', 'มี', 'ได้', 'the', 'a', 'an',
    'is', 'are', 'was', 'were', 'it', 'in', 'on', 'at', 'for', 'to', 'of', 'with', 'and', 'or',
    'this', 'that', 'i', 'you', 'me', 'my', 'your', 'can', 'do', 'be', 'will', 'please', 'how',
    'what', 'which', 'when', 'where', 'why', 'help', 'want', 'need', 'make', 'create', 'get', 'run'
]);

function extractKeywords(text) {
    return text
        .toLowerCase()
        .replace(/[^\w\u0E00-\u0E7F\s]/g, ' ')
        .split(/\s+/)
        .filter(w => w.length > 2 && !STOP_WORDS.has(w))
        .slice(0, 6);
}

// ── Main public API ────────────────────────────────────────────────────────

/**
 * Called after every successful chat turn.
 * Extracts patterns & infers preferences — runs async, non-blocking.
 */
function recordInteraction(userMessage, aiResponseText) {
    try {
        if (!userMessage || !aiResponseText) return;

        // Extract keywords as usage patterns
        const keywords = extractKeywords(userMessage);
        keywords.forEach(kw => recordPattern(kw));

        // Detect preferred language
        const thaiRatio = (userMessage.match(/[\u0E00-\u0E7F]/g) || []).length / userMessage.length;
        if (thaiRatio > 0.3) setProfile('preferred_language', 'thai');
        else setProfile('preferred_language', 'english');

        // Detect coding intent (update project activity)
        const codingKeywords = ['code', 'fix', 'debug', 'function', 'class', 'import', 'script',
            'แก้', 'เขียน', 'โค้ด', 'สคริปต์', 'ฟังก์ชัน'];
        if (codingKeywords.some(k => userMessage.toLowerCase().includes(k))) {
            const cwd = process.cwd();
            if (cwd !== os.homedir()) {
                setProfile('last_active_project', path.basename(cwd));
                setProfile('last_active_project_path', cwd);
            }
        }

        // Update interaction counter
        const count = parseInt(getProfile('total_interactions', '0'), 10);
        setProfile('total_interactions', String(count + 1));
        setProfile('last_seen', new Date().toISOString());
    } catch (err) {
        console.error('[Memory] recordInteraction error:', err.message);
    }
}

/**
 * Saves a condensed summary of a completed conversation.
 * Call this when user clears history or after N turns.
 */
function saveSessionSummary(summary, tags = []) {
    if (!summary || summary.trim().length < 10) return;
    addSessionMemory(summary.trim(), tags);
}

/**
 * Returns a formatted context string to inject into the AI system prompt.
 * Lightweight — no async calls.
 */
function getUserContext() {
    try {
        const profile = getAllProfile();
        const patterns = getTopPatterns(6);
        const memories = getRecentMemories(3);

        const lines = ['\n\n[LONG-TERM USER CONTEXT — use this to personalize responses]'];

        // Profile info
        if (Object.keys(profile).length > 0) {
            if (profile.preferred_language)
                lines.push(`• Preferred language: ${profile.preferred_language}`);
            if (profile.last_active_project)
                lines.push(`• Last active project: ${profile.last_active_project} (${profile.last_active_project_path || ''})`);
            if (profile.total_interactions)
                lines.push(`• Total interactions with Mint: ${profile.total_interactions}`);
            if (profile.last_seen) {
                const d = new Date(profile.last_seen);
                lines.push(`• Last session: ${d.toLocaleDateString('th-TH')} ${d.toLocaleTimeString('th-TH', { hour: '2-digit', minute: '2-digit' })}`);
            }
        }

        // Usage patterns
        if (patterns.length > 0) {
            const topTopics = patterns.map(p => p.pattern).join(', ');
            lines.push(`• Frequent topics/tools: ${topTopics}`);
        }

        // Past session memories
        if (memories.length > 0) {
            lines.push('\nRecent session summaries:');
            memories.forEach((m, i) => lines.push(`  ${i + 1}. ${m.summary}`));
        }

        if (lines.length === 1) return ''; // nothing to add
        lines.push('[END USER CONTEXT]\n');
        return lines.join('\n');
    } catch (err) {
        console.error('[Memory] getUserContext error:', err.message);
        return '';
    }
}

// ── Response Cache helpers ────────────────────────────────────────────────
function getCachedResponse(query) {
    try {
        const hash = crypto.createHash('md5').update(query.trim().toLowerCase()).digest('hex');
        const row = getDb().prepare('SELECT response, created_at FROM response_cache WHERE query_hash = ?').get(hash);
        if (row) {
            // Optional: check TTL (e.g., 24 hours)
            const age = Date.now() - new Date(row.created_at).getTime();
            if (age < 24 * 60 * 60 * 1000) {
                return JSON.parse(row.response);
            }
        }
    } catch (_) {}
    return null;
}

function cacheResponse(query, responseObj) {
    try {
        const hash = crypto.createHash('md5').update(query.trim().toLowerCase()).digest('hex');
        getDb().prepare(`
            INSERT INTO response_cache (query_hash, response, created_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(query_hash) DO UPDATE SET response = excluded.response, created_at = CURRENT_TIMESTAMP
        `).run(hash, JSON.stringify(responseObj));
    } catch (_) {}
}

module.exports = {
    recordInteraction,
    saveSessionSummary,
    getUserContext,
    setProfile,
    getProfile,
    getTopPatterns,
    getRecentMemories,
    getCachedResponse,
    cacheResponse
};
