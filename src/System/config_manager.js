const fs = require('fs');
const path = require('path');
const os = require('os');

let app;
try {
    const electron = require('electron');
    app = electron.app;
} catch (e) {
    app = null;
}

const CONFIG_DIR = path.join(os.homedir(), '.config', 'mint');
const LEGACY_DIR = path.join(os.homedir(), '.mint');

if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
}

// Migration: If old .mint exists but new .config/mint is empty, move files
if (fs.existsSync(LEGACY_DIR) && fs.readdirSync(CONFIG_DIR).length === 0) {
    try {
        const files = fs.readdirSync(LEGACY_DIR);
        for (const file of files) {
            fs.copyFileSync(path.join(LEGACY_DIR, file), path.join(CONFIG_DIR, file));
        }
        console.log('[Config] Migrated settings from ~/.mint to ~/.config/mint');
    } catch (e) {
        console.error('[Config] Migration failed:', e);
    }
}

const CONFIG_PATH = path.join(CONFIG_DIR, 'mint-config.json');

const DEFAULT_CONFIG = {
    theme: 'dark',
    accentColor: '#8b5cf6',
    systemTextColor: '#f8fafc',
    customBgStart: '#0f172a',
    customBgEnd: '#1e1b4b',
    customPanelBg: '#1e293b',
    apiKey: '',
    geminiModel: 'gemini-2.5-flash',
    language: 'th-TH',
    automationBrowser: 'chromium',
    proactiveInterval: 60,   // seconds between screen captures
    proactiveCooldown: 120,   // seconds minimum between actual suggestions
    aiProvider: 'gemini',
    ollamaModel: 'llama3:latest',
    enableVoiceReply: true,
    enableCustomWorkflows: true,
    ttsProvider: 'google',
    ttsVolume: 1.0,
    ttsSpeed: 1.0,
    ttsPitch: 1.0,
    pluginCalendarEnabled: false,
    pluginGmailEnabled: false,
    pluginNotionEnabled: false,
    pluginDiscordEnabled: false,
    showDesktopWidget: true,
    mcpServers: {},
    telegramBotToken: '',
    enableTelegramBridge: false,
    discordBotToken: '',
    enableDiscordBridge: false,
    slackBotToken: '',
    slackAppToken: '',
    enableSlackBridge: false,
    lineChannelAccessToken: '',
    lineChannelSecret: '',
    enableLineBridge: false,
    lineWebhookPort: 3000,
    enableWhatsappBridge: false,
    googleSearchApiKey: '',
    googleSearchCx: '',
    googleCalendarClientId: '',
    googleCalendarClientSecret: '',
    googleCalendarRefreshToken: '',
    googleCalendarId: 'primary',
    gmailClientId: '',
    gmailClientSecret: '',
    gmailRefreshToken: '',
    gmailUserId: 'me',
    notionApiKey: '',
    notionDatabaseId: '',
    notionPageId: '',
    notionTitleProperty: 'Name',
    braveSearchApiKey: '',
    anthropicApiKey: '',

    openaiApiKey: '',
    hfApiKey: '',
    anthropicModel: 'claude-3-5-sonnet-latest',
    openaiModel: 'gpt-4o',
    hfModel: 'meta-llama/Meta-Llama-3-8B-Instruct',
    localApiBaseUrl: '',
    localModelName: 'local-model',
    ollamaHost: '',
    enableAgentCollaboration: false,
    enableAutoUpdate: true,
    autoUpdateCheckIntervalHours: 24,
    lastUpdateCheckAt: '',
    safetyEnabled: true,
    sandboxMode: 'prefer', // off | prefer | enforce
    sandboxCommand: process.platform === 'darwin' ? 'sandbox-exec' : process.platform === 'linux' ? 'bwrap' : '',
    allowedReadPaths: [
        os.homedir(),
        process.cwd(),
        path.join(os.homedir(), 'Desktop'),
        path.join(os.homedir(), 'Documents'),
        path.join(os.homedir(), 'Downloads'),
        path.join(os.homedir(), 'Pictures'),
        path.join(os.homedir(), 'Music'),
        path.join(os.homedir(), 'Videos')
    ],
    allowedWritePaths: [
        os.homedir(),
        process.cwd(),
        path.join(os.homedir(), 'Desktop'),
        path.join(os.homedir(), 'Documents'),
        path.join(os.homedir(), 'Downloads'),
        path.join(os.homedir(), 'Pictures'),
        path.join(os.homedir(), 'Music'),
        path.join(os.homedir(), 'Videos')
    ],
    blockedPaths: [
        path.join(os.homedir(), '.ssh'),
        path.join(os.homedir(), '.gnupg'),
        path.join(os.homedir(), '.config', 'mint', 'mint-config.json'),
        path.join(os.homedir(), '.mint', 'mint-config.json')
    ],
    blockedFileNames: ['.env', 'id_rsa', 'id_ed25519']
};



function readConfig() {
    try {
        if (!fs.existsSync(CONFIG_PATH)) {
            writeConfig(DEFAULT_CONFIG);
            return DEFAULT_CONFIG;
        }
        const raw = fs.readFileSync(CONFIG_PATH, 'utf-8');
        return { ...DEFAULT_CONFIG, ...JSON.parse(raw) };
    } catch (err) {
        console.error('readConfig error:', err);
        return DEFAULT_CONFIG;
    }
}

function writeConfig(config) {
    try {
        fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2), 'utf-8');
        return { success: true };
    } catch (err) {
        console.error('writeConfig error:', err);
        return { success: false, message: err.message };
    }
}

function getAvailableProviders(config) {
    const providers = [];
    const cfg = config || readConfig();
    
    // Check which providers have API keys or URLs configured
    const anthropicKey = cfg.anthropicApiKey || process.env.ANTHROPIC_API_KEY;
    if (!isPlaceholder(anthropicKey)) providers.push('anthropic');

    const openaiKey = cfg.openaiApiKey || process.env.OPENAI_API_KEY;
    if (!isPlaceholder(openaiKey)) providers.push('openai');

    const geminiKey = cfg.apiKey || process.env.GEMINI_API_KEY;
    if (!isPlaceholder(geminiKey)) providers.push('gemini');

    const hfKey = cfg.hfApiKey || process.env.HF_API_KEY;
    if (!isPlaceholder(hfKey)) providers.push('huggingface');

    if (cfg.localApiBaseUrl && cfg.localApiBaseUrl.trim() !== '') providers.push('local_openai');
    
    // Always push ollama at the end since it's local
    providers.push('ollama');

    return providers;
}

function isPlaceholder(val) {
    return !val || val.startsWith('your_') || val.includes('key_here') || val.trim() === '';
}

module.exports = { readConfig, writeConfig, getAvailableProviders, isPlaceholder, CONFIG_PATH };
