/**
 * Mint Long-Term Memory Store
 * ---------------------------
 * Persists user preferences, session summaries, and usage patterns
 * across all Mint sessions using SQLite (same DB as knowledge_base).
 *
 * Auto-injects a "User Context" block into the system prompt so Mint
 * remembers who it's talking to even after restart.
 */

import * as fs from 'fs'
import * as path from 'path'
import * as os from 'os'
import * as crypto from 'crypto'
import { readConfig  } from '../System/config_manager'

// ── Electron-safe app path ──────────────────────────────────────────────────
let app;
try {
    const electron = require('electron')
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

        -- Raw episodic memories of user/assistant turns.
        CREATE TABLE IF NOT EXISTS interaction_memories (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            user_text   TEXT NOT NULL,
            ai_text     TEXT NOT NULL,
            keywords    TEXT DEFAULT '',
            created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Response Cache: For repetitive exact queries
        CREATE TABLE IF NOT EXISTS response_cache (
            query_hash TEXT PRIMARY KEY,
            response   TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Learned skill/instruction documents imported from local files.
        CREATE TABLE IF NOT EXISTS learned_skills (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT NOT NULL,
            source_path TEXT NOT NULL UNIQUE,
            content    TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
    `);

    return dbInstance;
}

function ensureLearnedSkillsTable() {
    getDb().exec(`
        CREATE TABLE IF NOT EXISTS learned_skills (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            source_path TEXT NOT NULL UNIQUE,
            content     TEXT NOT NULL,
            created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
        );
    `);
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

function deleteProfile(key) {
    try {
        getDb().prepare('DELETE FROM user_profile WHERE key = ?').run(key);
    } catch (err) {
        console.error('[Memory] deleteProfile error:', err.message);
    }
}

function clearConversationScopedProfile() {
    deleteProfile('preferred_language');
    clearResponseCache();
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

const MAX_INTERACTION_MEMORIES = 1000;

function stripRelevantMemoryBlock(text) {
    return String(text || '')
        .replace(/\n?\[Relevant long-term memory for this user message\][\s\S]*?\[End relevant memory\]\n?/g, '\n')
        .replace(/^\s*\[Relevant long-term memory for this user message\][\s\S]*?\[End relevant memory\]\s*/g, '')
        .replace(/\n?\[LOCAL KNOWLEDGE BASE - USE THIS CONTEXT TO ANSWER\][\s\S]*/g, '')
        .trim();
}

function addInteractionMemory(userMessage, aiResponseText, keywords = []) {
    try {
        const db = getDb();
        db.prepare(`
            INSERT INTO interaction_memories (user_text, ai_text, keywords)
            VALUES (?, ?, ?)
        `).run(
            String(userMessage || '').slice(0, 1200),
            String(aiResponseText || '').slice(0, 1200),
            keywords.join(',')
        );
        db.exec(`
            DELETE FROM interaction_memories WHERE id NOT IN (
                SELECT id FROM interaction_memories ORDER BY id DESC LIMIT ${MAX_INTERACTION_MEMORIES}
            )
        `);
    } catch (err) {
        console.error('[Memory] addInteractionMemory error:', err.message);
    }
}

function getRecentInteractions(limit = 5) {
    try {
        return getDb()
            .prepare('SELECT id, user_text, ai_text, keywords, created_at FROM interaction_memories ORDER BY id DESC LIMIT ?')
            .all(limit);
    } catch (_) {
        return [];
    }
}

function deleteInteractionMemory(id) {
    try {
        const result = getDb().prepare('DELETE FROM interaction_memories WHERE id = ?').run(id);
        return result.changes > 0;
    } catch (err) {
        console.error('[Memory] deleteInteractionMemory error:', err.message);
        return false;
    }
}

function searchInteractions(query, limit = 8) {
    try {
        const keywords = extractKeywords(query);
        const terms = keywords.length > 0 ? keywords : [String(query || '').trim()].filter(Boolean);
        if (terms.length === 0) return [];

        const rows = [];
        const seen = new Set();
        const stmt = getDb().prepare(`
            SELECT id, user_text, ai_text, keywords, created_at
            FROM interaction_memories
            WHERE user_text LIKE ? OR ai_text LIKE ? OR keywords LIKE ?
            ORDER BY id DESC
            LIMIT ?
        `);

        for (const term of terms.slice(0, 5)) {
            const like = `%${term}%`;
            for (const row of stmt.all(like, like, like, limit)) {
                if (!seen.has(row.id)) {
                    seen.add(row.id);
                    rows.push(row);
                    if (rows.length >= limit) return rows;
                }
            }
        }
        return rows;
    } catch (_) {
        return [];
    }
}

function clearInteractionMemories() {
    try {
        getDb().prepare('DELETE FROM interaction_memories').run();
    } catch (err) {
        console.error('[Memory] clearInteractionMemories error:', err.message);
    }
}

function exportMemorySnapshot() {
    try {
        return {
            profile: getAllProfile(),
            session_memories: getRecentMemories(MAX_SESSION_MEMORIES),
            usage_patterns: getTopPatterns(50),
            interaction_memories: getRecentInteractions(MAX_INTERACTION_MEMORIES)
        };
    } catch (err) {
        console.error('[Memory] exportMemorySnapshot error:', err.message);
        return {
            profile: {},
            session_memories: [],
            usage_patterns: [],
            interaction_memories: []
        };
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

function cleanProfileValue(value) {
    return String(value || '')
        .replace(/[.,!?;:()[\]{}"'`“”‘’]+$/g, '')
        .replace(/(นะ|น่ะ|ครับ|ค่ะ|คะ|จ้า|จ๊ะ|ฮะ|ค้าบ|ค่า)+$/u, '')
        .trim();
}

function extractUserName(text) {
    const input = String(text || '').trim();
    const patterns = [
        /(?:ผม|ฉัน|ชั้น|หนู|เรา|ข้า|ดิฉัน)?\s*ชื่อ(?:เล่น)?\s*(?:คือ|ว่า|เป็น)?\s*([A-Za-z\u0E00-\u0E7F][A-Za-z\u0E00-\u0E7F\s]{0,40})/iu,
        /(?:เรียก(?:ผม|ฉัน|ชั้น|หนู|เรา)?ว่า)\s*([A-Za-z\u0E00-\u0E7F][A-Za-z\u0E00-\u0E7F\s]{0,40})/iu,
        /\bmy name is\s+([A-Za-z][A-Za-z\s'-]{0,40})/iu,
        /\bcall me\s+([A-Za-z][A-Za-z\s'-]{0,40})/iu,
        /\bi am\s+([A-Za-z][A-Za-z\s'-]{0,40})/iu,
        /\bi'm\s+([A-Za-z][A-Za-z\s'-]{0,40})/iu
    ];

    for (const pattern of patterns) {
        const match = input.match(pattern);
        if (match && match[1]) {
            const name = cleanProfileValue(match[1])
                .split(/\s+(?:and|แล้ว|นะ|ครับ|ค่ะ|คะ)\s+/i)[0]
                .trim();
            if (name && name.length <= 40) return name;
        }
    }

    return '';
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
        addInteractionMemory(userMessage, aiResponseText, keywords);

        // Detect preferred language
        const thaiRatio = (userMessage.match(/[\u0E00-\u0E7F]/g) || []).length / userMessage.length;
        if (thaiRatio > 0.3) setProfile('preferred_language', 'thai');
        else setProfile('preferred_language', 'english');

        const userName = extractUserName(userMessage);
        if (userName) {
            setProfile('user_name', userName);
        }

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

function addLearnedSkill(name, sourcePath, content) {
    const cleanName = String(name || '').trim() || path.basename(sourcePath || 'skill.md');
    const cleanPath = path.resolve(String(sourcePath || ''));
    const cleanContent = String(content || '').trim();
    if (!cleanContent) {
        throw new Error('Skill file is empty.');
    }

    const storedContent = cleanContent.slice(0, 12000);
    ensureLearnedSkillsTable();
    const db = getDb();
    db.prepare(`
        INSERT INTO learned_skills (name, source_path, content, created_at, updated_at)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT(source_path) DO UPDATE SET
            name = excluded.name,
            content = excluded.content,
            updated_at = CURRENT_TIMESTAMP
    `).run(cleanName, cleanPath, storedContent);

    return {
        name: cleanName,
        source_path: cleanPath,
        content_length: cleanContent.length,
        stored_length: storedContent.length
    };
}

function getLearnedSkills(limit = 10) {
    try {
        ensureLearnedSkillsTable();
        return getDb().prepare(`
            SELECT id, name, source_path, content, created_at, updated_at
            FROM learned_skills
            ORDER BY updated_at DESC, id DESC
            LIMIT ?
        `).all(limit);
    } catch (err) {
        console.error('[Memory] getLearnedSkills error:', err.message);
        return [];
    }
}

function deleteLearnedSkill(identifier) {
    try {
        ensureLearnedSkillsTable();
        const input = String(identifier || '').trim();
        if (!input) return 0;

        const db = getDb();
        if (/^\d+$/.test(input)) {
            return db.prepare('DELETE FROM learned_skills WHERE id = ?').run(Number(input)).changes;
        }

        const resolved = path.resolve(input);
        return db.prepare('DELETE FROM learned_skills WHERE source_path = ? OR name = ?').run(resolved, input).changes;
    } catch (err) {
        console.error('[Memory] deleteLearnedSkill error:', err.message);
        return 0;
    }
}

/**
 * Returns a formatted context string to inject into the AI system prompt.
 * Lightweight — no async calls.
 */
function getUserContext(query = '') {
    try {
        const profile = getAllProfile();
        const patterns = getTopPatterns(6);
        const memories = getRecentMemories(3);
        const interactions = getRecentInteractions(6);
        const relevantInteractions = query ? searchInteractions(query, 5) : [];

        const lines = ['\n\n[LONG-TERM USER CONTEXT — use this to personalize responses]'];

        // Profile info
        if (Object.keys(profile).length > 0) {
            if (profile.user_name)
                lines.push(`• User name: ${profile.user_name}`);
            if (profile.preferred_language)
                lines.push(`• Previously inferred language: ${profile.preferred_language} (do not override the current user message language)`);
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

        if (interactions.length > 0) {
            lines.push('\nRecent remembered interactions:');
            interactions.forEach((m, i) => {
                lines.push(`  ${i + 1}. User: ${m.user_text}`);
                lines.push(`     Mint: ${m.ai_text}`);
            });
        }

        if (relevantInteractions.length > 0) {
            lines.push('\nRelevant remembered interactions for the current request:');
            relevantInteractions.forEach((m, i) => {
                lines.push(`  ${i + 1}. User: ${m.user_text}`);
                lines.push(`     Mint: ${m.ai_text}`);
            });
        }

        const learnedSkills = getLearnedSkills(8);
        if (learnedSkills.length > 0) {
            lines.push('\nLearned skill/instruction files:');
            learnedSkills.forEach((skill, i) => {
                lines.push(`\n  ${i + 1}. ${skill.name}`);
                lines.push(`     Source: ${skill.source_path}`);
                lines.push('     Content:');
                lines.push(skill.content
                    .split('\n')
                    .map(line => `       ${line}`)
                    .join('\n'));
            });
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
                const parsed = JSON.parse(row.response);
                if (parsed && typeof parsed.response === 'string') {
                    parsed.response = stripRelevantMemoryBlock(parsed.response);
                }
                return parsed;
            }
        }
    } catch (_) {}
    return null;
}

function cacheResponse(query, responseObj) {
    try {
        const hash = crypto.createHash('md5').update(query.trim().toLowerCase()).digest('hex');
        const sanitized = (responseObj && typeof responseObj === 'object')
            ? {
                ...responseObj,
                response: typeof responseObj.response === 'string'
                    ? stripRelevantMemoryBlock(responseObj.response)
                    : responseObj.response
            }
            : responseObj;
        getDb().prepare(`
            INSERT INTO response_cache (query_hash, response, created_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(query_hash) DO UPDATE SET response = excluded.response, created_at = CURRENT_TIMESTAMP
        `).run(hash, JSON.stringify(sanitized));
    } catch (_) {}
}

function clearResponseCache() {
    try {
        getDb().prepare('DELETE FROM response_cache').run();
    } catch (err) {
        console.error('[Memory] clearResponseCache error:', err.message);
    }
}

export { recordInteraction,
    saveSessionSummary,
    getUserContext,
    setProfile,
    deleteProfile,
    clearConversationScopedProfile,
    getProfile,
    getAllProfile,
    addLearnedSkill,
    getLearnedSkills,
    deleteLearnedSkill,
    getTopPatterns,
    getRecentInteractions,
    searchInteractions,
    deleteInteractionMemory,
    clearInteractionMemories,
    exportMemorySnapshot,
    getRecentMemories,
    getCachedResponse,
    cacheResponse,
    clearResponseCache
 }
