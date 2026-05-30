import * as fs from 'fs'
import * as path from 'path'
import * as os from 'os'

// Handle electron dependency safely
let app;
try {
    const electron = require('electron')
    app = electron.app;
} catch (e) {
    app = null;
}

const CONFIG_DIR = path.join(os.homedir(), '.config', 'mint');
if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
}

const MEMORY_FILE = path.join(CONFIG_DIR, 'behavior_memory.json');

// Migration Logic: Move from Electron userData to ~/.config/mint
if (!fs.existsSync(MEMORY_FILE) && app && app.getPath) {
    const electronPath = path.join(app.getPath('userData'), 'behavior_memory.json');
    if (fs.existsSync(electronPath)) {
        try {
            fs.copyFileSync(electronPath, MEMORY_FILE);
            console.log('[BehaviorMemory] Migrated memory from Electron userData');
        } catch (e) { console.error('[BehaviorMemory] Migration failed:', e); }
    }
}
const MAX_CONTEXT_HISTORY = 20; // Keep last 20 context snapshots

/**
 * Load memory from disk (or return default empty structure)
 */
function loadMemory() {
    try {
        if (fs.existsSync(MEMORY_FILE)) {
            const raw = fs.readFileSync(MEMORY_FILE, 'utf8');
            return JSON.parse(raw);
        }
    } catch (err) {
        console.error('[BehaviorMemory] Failed to read memory file:', err);
    }

    // Default empty memory structure
    return {
        appFrequency: {},       // { "YouTube": 5, "Google Chrome": 12 }
        contextHistory: [],     // Last N context strings
        lastUpdated: null
    };
}

/**
 * Save memory to disk
 */
function saveMemory(memory) {
    try {
        fs.writeFileSync(MEMORY_FILE, JSON.stringify(memory, null, 2), 'utf8');
    } catch (err) {
        console.error('[BehaviorMemory] Failed to save memory:', err);
    }
}

/**
 * Record a new context observation (called after each proactive scan)
 * @param {string} contextDescription - Short description of what user is doing
 */
function recordBehavior(contextDescription) {
    if (!contextDescription || typeof contextDescription !== 'string') return;

    const memory = loadMemory();

    // Append to context history (capped at MAX)
    memory.contextHistory.unshift({
        context: contextDescription,
        time: new Date().toISOString(),
        hour: new Date().getHours()
    });
    if (memory.contextHistory.length > MAX_CONTEXT_HISTORY) {
        memory.contextHistory = memory.contextHistory.slice(0, MAX_CONTEXT_HISTORY);
    }

    // Extract app mentions and bump frequency
    const appKeywords = [
        'YouTube', 'Chrome', 'Firefox', 'VS Code', 'Spotify', 'Terminal',
        'Google', 'Discord', 'Slack', 'Gmail', 'GitHub', 'Figma', 'Notion'
    ];
    for (const app of appKeywords) {
        if (contextDescription.toLowerCase().includes(app.toLowerCase())) {
            memory.appFrequency[app] = (memory.appFrequency[app] || 0) + 1;
        }
    }

    memory.lastUpdated = new Date().toISOString();
    saveMemory(memory);
}

/**
 * Get a summary of behavior patterns for the Gemini prompt
 * @returns {string} A human-readable behavior summary
 */
function getBehaviorSummary() {
    const memory = loadMemory();

    const parts = [];

    // Top apps by frequency
    const topApps = Object.entries(memory.appFrequency)
        .sort((a: any, b: any) => b[1] - a[1])
        .slice(0, 5)
        .map(([app, count]) => `${app} (${count}x)`);

    if (topApps.length > 0) {
        parts.push(`Apps user frequently uses: ${topApps.join(', ')}`);
    }

    // Recent contexts (last 3)
    const recentCtx = memory.contextHistory.slice(0, 3).map(c => c.context);
    if (recentCtx.length > 0) {
        parts.push(`Recent activities: ${recentCtx.join(' | ')}`);
    }

    // Time of day
    const hour = new Date().getHours();
    let timeOfDay = 'morning';
    if (hour >= 12 && hour < 17) timeOfDay = 'afternoon';
    else if (hour >= 17 && hour < 21) timeOfDay = 'evening';
    else if (hour >= 21 || hour < 5) timeOfDay = 'night';
    parts.push(`Current time of day: ${timeOfDay}`);

    return parts.join('. ');
}

export { recordBehavior, getBehaviorSummary  }
