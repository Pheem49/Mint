const DEFAULT_CONFIG = {
    theme: 'dark',
    accentColor: '#8b5cf6',
    systemTextColor: '#f8fafc',
    customBgStart: '#0f172a',
    customBgEnd: '#1e1b4b',
    customPanelBg: '#1e293b',
    apiKey: '',
    language: 'th-TH',
    proactiveInterval: 60,
    proactiveCooldown: 120
};

let currentConfig = { ...DEFAULT_CONFIG };

// Load settings from main process
async function loadSettings() {
    const config = await window.settingsApi.getSettings();
    currentConfig = { ...DEFAULT_CONFIG, ...config };
    applyConfig(currentConfig);
}

function applyConfig(config) {
    // Apply theme
    document.documentElement.setAttribute('data-theme', config.theme);
    
    if (config.theme === 'custom') {
        document.getElementById('custom-theme-controls').style.display = 'block';
        applyCustomThemeStyles(config);
    } else {
        document.getElementById('custom-theme-controls').style.display = 'none';
        // Reset dynamic style variables if not custom
        document.documentElement.style.removeProperty('--bg-gradient');
        document.documentElement.style.removeProperty('--panel-bg');
    }

    // Apply accent color
    document.documentElement.style.setProperty('--accent', config.accentColor);
    document.documentElement.style.setProperty('--accent-hover', lightenColor(config.accentColor, 20));

    // Apply API key
    document.getElementById('api-key-input').value = config.apiKey || '';

    // Apply Automation Browser
    if (config.automationBrowser) {
        document.getElementById('automation-browser-select').value = config.automationBrowser;
    }

    // Update active theme card
    document.querySelectorAll('.theme-card').forEach(card => {
        card.classList.toggle('active', card.dataset.theme === config.theme);
    });

    // Update active color dot
    document.querySelectorAll('.color-dot').forEach(dot => {
        dot.classList.toggle('active', dot.dataset.color === config.accentColor);
    });

    // Update color picker
    document.getElementById('custom-color').value = config.accentColor;

    document.getElementById('system-text-color').value = textColor;
    document.documentElement.style.setProperty('--text-main', textColor);

    // Update custom color pickers
    document.getElementById('custom-bg-start').value = config.customBgStart || '#0f172a';
    document.getElementById('custom-bg-end').value = config.customBgEnd || '#1e1b4b';
    document.getElementById('custom-panel-bg').value = config.customPanelBg || '#1e293b';
    updateCustomPreviewBox(config);

    // Apply proactive settings
    const interval = config.proactiveInterval || 60;
    const cooldown = config.proactiveCooldown || 120;
    document.getElementById('proactive-interval').value = interval;
    document.getElementById('proactive-cooldown').value = cooldown;
    updateIntervalDisplay(interval);
    updateCooldownDisplay(cooldown);
}

function lightenColor(hex, amount) {
    const num = parseInt(hex.replace('#', ''), 16);
    const r = Math.min(255, (num >> 16) + amount);
    const g = Math.min(255, ((num >> 8) & 0x00FF) + amount);
    const b = Math.min(255, (num & 0x0000FF) + amount);
    return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`;
}

// --- Event Listeners ---

// Close button
document.getElementById('close-btn').addEventListener('click', () => {
    window.settingsApi.closeSettings();
});

// Toggle API key visibility
document.getElementById('toggle-key').addEventListener('click', () => {
    const input = document.getElementById('api-key-input');
    input.type = input.type === 'password' ? 'text' : 'password';
});

// AI Studio link
document.getElementById('ai-studio-link').addEventListener('click', () => {
    window.settingsApi.openExternal('https://aistudio.google.com/');
});

// Theme cards
document.querySelectorAll('.theme-card').forEach(card => {
    card.addEventListener('click', () => {
        currentConfig.theme = card.dataset.theme;
        applyConfig(currentConfig);
    });
});

// Color presets
document.querySelectorAll('.color-dot').forEach(dot => {
    dot.addEventListener('click', () => {
        currentConfig.accentColor = dot.dataset.color;
        applyConfig(currentConfig);
        document.getElementById('custom-color').value = dot.dataset.color;
    });
});

// Custom color picker
document.getElementById('custom-color').addEventListener('input', (e) => {
    currentConfig.accentColor = e.target.value;
    document.documentElement.style.setProperty('--accent', e.target.value);
    document.documentElement.style.setProperty('--accent-hover', lightenColor(e.target.value, 20));
    // Deselect presets
    document.querySelectorAll('.color-dot').forEach(dot => dot.classList.remove('active'));
});

// System text color picker
document.getElementById('system-text-color').addEventListener('input', (e) => {
    currentConfig.systemTextColor = e.target.value;
    document.documentElement.style.setProperty('--text-main', e.target.value);
});

// Custom Theme color pickers
function applyCustomThemeStyles(cfg) {
    const gradient = `linear-gradient(135deg, ${cfg.customBgStart} 0%, ${cfg.customBgEnd} 100%)`;
    document.documentElement.style.setProperty('--bg-gradient', gradient);
    
    // Convert hex to rgba for panel bg to keep transparency
    const panelRgb = hexToRgb(cfg.customPanelBg);
    document.documentElement.style.setProperty('--panel-bg', `rgba(${panelRgb.r}, ${panelRgb.g}, ${panelRgb.b}, 0.75)`);
    updateCustomPreviewBox(cfg);
}

function updateCustomPreviewBox(cfg) {
    const box = document.getElementById('custom-theme-preview-box');
    if (box) {
        box.style.background = `linear-gradient(135deg, ${cfg.customBgStart} 0%, ${cfg.customBgEnd} 100%)`;
    }
}

function hexToRgb(hex) {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16)
    } : { r: 30, g: 41, b: 59 };
}

document.getElementById('custom-bg-start').addEventListener('input', (e) => {
    currentConfig.customBgStart = e.target.value;
    if (currentConfig.theme === 'custom') applyCustomThemeStyles(currentConfig);
});
document.getElementById('custom-bg-end').addEventListener('input', (e) => {
    currentConfig.customBgEnd = e.target.value;
    if (currentConfig.theme === 'custom') applyCustomThemeStyles(currentConfig);
});
document.getElementById('custom-panel-bg').addEventListener('input', (e) => {
    currentConfig.customPanelBg = e.target.value;
    if (currentConfig.theme === 'custom') applyCustomThemeStyles(currentConfig);
});

// Proactive sliders
function formatSeconds(s) {
    if (s < 60) return `${s} sec`;
    const m = s / 60;
    return Number.isInteger(m) ? `${m} min` : `${m.toFixed(1)} min`;
}

function updateIntervalDisplay(val) {
    document.getElementById('proactive-interval-display').textContent = formatSeconds(Number(val));
}

function updateCooldownDisplay(val) {
    document.getElementById('proactive-cooldown-display').textContent = formatSeconds(Number(val));
}

document.getElementById('proactive-interval').addEventListener('input', (e) => {
    updateIntervalDisplay(e.target.value);
});

document.getElementById('proactive-cooldown').addEventListener('input', (e) => {
    updateCooldownDisplay(e.target.value);
});

// Save
document.getElementById('save-btn').addEventListener('click', async () => {
    currentConfig.apiKey = document.getElementById('api-key-input').value.trim();
    currentConfig.automationBrowser = document.getElementById('automation-browser-select').value;
    currentConfig.proactiveInterval = Number(document.getElementById('proactive-interval').value);
    currentConfig.proactiveCooldown = Number(document.getElementById('proactive-cooldown').value);
    currentConfig.systemTextColor = document.getElementById('system-text-color').value;
    
    currentConfig.customBgStart = document.getElementById('custom-bg-start').value;
    currentConfig.customBgEnd = document.getElementById('custom-bg-end').value;
    currentConfig.customPanelBg = document.getElementById('custom-panel-bg').value;

    await window.settingsApi.saveSettings(currentConfig);
    const btn = document.getElementById('save-btn');
    btn.textContent = '✅ Saved!';
    setTimeout(() => { btn.textContent = 'Save Settings'; }, 1500);
});

// Quit App
document.getElementById('quit-btn').addEventListener('click', () => {
    if (confirm('Are you sure you want to quit Mint?')) {
        window.settingsApi.quitApp();
    }
});

// Reset to default
document.getElementById('reset-btn').addEventListener('click', () => {
    currentConfig = { ...DEFAULT_CONFIG };
    applyConfig(currentConfig);
});

// Init
window.addEventListener('DOMContentLoaded', loadSettings);
