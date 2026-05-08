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
    proactiveInterval: 60,
    proactiveCooldown: 120,
    glassBlur: 'blur(16px)',
    fontFamily: "'Outfit', sans-serif",
    aiProvider: 'gemini',
    ollamaModel: 'llama3:latest',
    enableVoiceReply: true,
    enableCustomWorkflows: true,
    enableAgentCollaboration: true,
    ttsProvider: 'google',
    ttsVolume: 1.0,
    ttsSpeed: 1.0,
    ttsPitch: 1.0,
    pluginSpotifyEnabled: true,
    pluginCalendarEnabled: false,
    pluginDiscordEnabled: false,
    showDesktopWidget: true,
    mcpServers: {}
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

    const anthropicInput = document.getElementById('anthropic-api-key-input');
    if (anthropicInput) anthropicInput.value = config.anthropicApiKey || '';

    const openaiInput = document.getElementById('openai-api-key-input');
    if (openaiInput) openaiInput.value = config.openaiApiKey || '';

    const hfInput = document.getElementById('hf-api-key');
    if (hfInput) hfInput.value = config.hfApiKey || '';

    const ollamaHostInput = document.getElementById('ollama-host-input');
    if (ollamaHostInput) ollamaHostInput.value = config.ollamaHost || 'http://localhost:11434';

    // Apply Gemini model
    applyModelSelection(config.geminiModel);
    
    // Apply AI Provider
    const providerSelect = document.getElementById('ai-provider-select');
    if (providerSelect) {
        providerSelect.value = config.aiProvider || 'gemini';
        toggleProviderOptions(providerSelect.value);
    }
    
    const ollamaInput = document.getElementById('ollama-model-input');
    if (ollamaInput) {
        ollamaInput.value = config.ollamaModel || 'llama3:latest';
    }

    applyModelSelectionGeneric('openai-model-select', 'openai-model-custom-row', 'openai-model-custom', config.openaiModel || 'gpt-4o');
    applyModelSelectionGeneric('anthropic-model-select', 'anthropic-model-custom-row', 'anthropic-model-custom', config.anthropicModel || 'claude-3-5-sonnet-latest');

    const hfModelInput = document.getElementById('hf-model-name');
    if (hfModelInput) {
        hfModelInput.value = config.hfModel || 'meta-llama/Meta-Llama-3-8B-Instruct';
    }

    const localApiBaseUrlInput = document.getElementById('local-api-base-url');
    if (localApiBaseUrlInput) {
        localApiBaseUrlInput.value = config.localApiBaseUrl || 'http://localhost:1234/v1';
    }

    const localModelNameInput = document.getElementById('local-model-name');
    if (localModelNameInput) {
        localModelNameInput.value = config.localModelName || 'local-model';
    }

    const voiceReplyToggle = document.getElementById('enable-voice-reply');
    if (voiceReplyToggle) {
        voiceReplyToggle.checked = config.enableVoiceReply !== false;
    }

    const ttsProviderSelect = document.getElementById('tts-provider-select');
    if (ttsProviderSelect) ttsProviderSelect.value = config.ttsProvider || 'google';

    const ttsVolume = document.getElementById('tts-volume');
    if (ttsVolume) {
        ttsVolume.value = config.ttsVolume !== undefined ? config.ttsVolume : 1.0;
        document.getElementById('tts-volume-val').textContent = `${Math.round(ttsVolume.value * 100)}%`;
    }

    const ttsSpeed = document.getElementById('tts-speed');
    if (ttsSpeed) {
        ttsSpeed.value = config.ttsSpeed !== undefined ? config.ttsSpeed : 1.0;
        document.getElementById('tts-speed-val').textContent = `${parseFloat(ttsSpeed.value).toFixed(1)}x`;
    }

    const ttsPitch = document.getElementById('tts-pitch');
    if (ttsPitch) {
        ttsPitch.value = config.ttsPitch !== undefined ? config.ttsPitch : 1.0;
        document.getElementById('tts-pitch-val').textContent = parseFloat(ttsPitch.value).toFixed(1);
    }

    const enableWorkflowsToggle = document.getElementById('enable-custom-workflows');
    if (enableWorkflowsToggle) {
        enableWorkflowsToggle.checked = config.enableCustomWorkflows !== false;
    }

    const enableAgentCollaborationToggle = document.getElementById('enable-agent-collaboration');
    if (enableAgentCollaborationToggle) {
        enableAgentCollaborationToggle.checked = config.enableAgentCollaboration !== false;
    }

    // Plugins logic
    updatePluginButton('spotify', config.pluginSpotifyEnabled);
    updatePluginButton('calendar', config.pluginCalendarEnabled);
    updatePluginButton('discord', config.pluginDiscordEnabled);

    // Apply Automation Browser
    if (config.automationBrowser) {
        document.getElementById('automation-browser-select').value = config.automationBrowser;
    }

    const showWidgetToggle = document.getElementById('show-desktop-widget');
    if (showWidgetToggle) {
        showWidgetToggle.checked = config.showDesktopWidget !== false;
    }

    // Apply UI Customizations
    document.getElementById('glass-blur-select').value = config.glassBlur || 'blur(16px)';
    document.documentElement.style.setProperty('--glass-blur', config.glassBlur || 'blur(16px)');

    document.getElementById('font-family-select').value = config.fontFamily || "'Outfit', sans-serif";
    document.body.style.fontFamily = config.fontFamily || "'Outfit', sans-serif";

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

    // MCP Servers
    renderMcpServers();
}

function lightenColor(hex, amount) {
    const num = parseInt(hex.replace('#', ''), 16);
    const r = Math.min(255, (num >> 16) + amount);
    const g = Math.min(255, ((num >> 8) & 0x00FF) + amount);
    const b = Math.min(255, (num & 0x0000FF) + amount);
    return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`;
}

function applyModelSelection(model) {
    applyModelSelectionGeneric('gemini-model-select', 'gemini-model-custom-row', 'gemini-model-custom', model);
}

function applyModelSelectionGeneric(selectId, customRowId, customInputId, model) {
    const select = document.getElementById(selectId);
    const customRow = document.getElementById(customRowId);
    const customInput = document.getElementById(customInputId);
    if (!select || !customRow || !customInput) return;

    const normalized = (model || '').trim();
    const optionValues = Array.from(select.options).map(opt => opt.value);

    if (normalized && optionValues.includes(normalized)) {
        select.value = normalized;
        customRow.style.display = 'none';
        customInput.value = '';
    } else {
        select.value = 'custom';
        customRow.style.display = 'block';
        customInput.value = normalized;
    }
}

function getSelectedModel() {
    return getSelectedModelGeneric('gemini-model-select', 'gemini-model-custom', DEFAULT_CONFIG.geminiModel);
}

function getSelectedModelGeneric(selectId, customInputId, defaultModel) {
    const select = document.getElementById(selectId);
    const customInput = document.getElementById(customInputId);
    if (!select || !customInput) return defaultModel;
    if (select.value === 'custom') {
        const custom = (customInput.value || '').trim();
        return custom || defaultModel;
    }
    return select.value;
}

// --- Event Listeners ---

// Close button
// Close button
const closeBtn = document.getElementById('close-btn');
if (closeBtn) {
    closeBtn.addEventListener('click', () => {
        window.settingsApi.closeSettings();
    });
}

// Toggle API key visibility
const toggleKey = document.getElementById('toggle-key');
if (toggleKey) {
    toggleKey.addEventListener('click', () => {
        const input = document.getElementById('api-key-input');
        input.type = input.type === 'password' ? 'text' : 'password';
    });
}

async function saveApiKeyOnly() {
    const input = document.getElementById('api-key-input');
    const status = document.getElementById('api-save-status');
    const btn = document.getElementById('save-api-key');
    const apiKey = input ? input.value.trim() : '';

    try {
        const baseConfig = await window.settingsApi.getSettings();
        const nextConfig = { ...baseConfig, apiKey };
        await window.settingsApi.saveSettings(nextConfig);
        currentConfig.apiKey = apiKey;

        if (btn) btn.textContent = 'Saved!';
        if (status) status.textContent = 'API key saved';
        setTimeout(() => {
            if (btn) btn.textContent = 'Save API Key';
            if (status) status.textContent = '';
        }, 1500);
    } catch (err) {
        console.error('Failed to save API key:', err);
        if (status) status.textContent = 'Save failed';
        setTimeout(() => { if (status) status.textContent = ''; }, 1500);
    }
}

const saveApiKey = document.getElementById('save-api-key');
if (saveApiKey) saveApiKey.addEventListener('click', saveApiKeyOnly);

const apiKeyInput = document.getElementById('api-key-input');
if (apiKeyInput) {
    apiKeyInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            saveApiKeyOnly();
        }
    });
}

// Gemini model select
document.getElementById('gemini-model-select').addEventListener('change', (e) => {
    const customRow = document.getElementById('gemini-model-custom-row');
    if (e.target.value === 'custom') {
        customRow.style.display = 'block';
        currentConfig.geminiModel = (document.getElementById('gemini-model-custom').value || '').trim();
    } else {
        customRow.style.display = 'none';
        currentConfig.geminiModel = e.target.value;
    }
});

document.getElementById('gemini-model-custom').addEventListener('input', (e) => {
    currentConfig.geminiModel = e.target.value.trim();
});

// OpenAI model select
document.getElementById('openai-model-select').addEventListener('change', (e) => {
    const customRow = document.getElementById('openai-model-custom-row');
    if (e.target.value === 'custom') {
        customRow.style.display = 'block';
        currentConfig.openaiModel = (document.getElementById('openai-model-custom').value || '').trim();
    } else {
        customRow.style.display = 'none';
        currentConfig.openaiModel = e.target.value;
    }
});

document.getElementById('openai-model-custom').addEventListener('input', (e) => {
    currentConfig.openaiModel = e.target.value.trim();
});

// Anthropic model select
document.getElementById('anthropic-model-select').addEventListener('change', (e) => {
    const customRow = document.getElementById('anthropic-model-custom-row');
    if (e.target.value === 'custom') {
        customRow.style.display = 'block';
        currentConfig.anthropicModel = (document.getElementById('anthropic-model-custom').value || '').trim();
    } else {
        customRow.style.display = 'none';
        currentConfig.anthropicModel = e.target.value;
    }
});

document.getElementById('anthropic-model-custom').addEventListener('input', (e) => {
    currentConfig.anthropicModel = e.target.value.trim();
});

// AI Provider toggle (No-op since all sections stay visible)
function toggleProviderOptions(provider) {
    // No-op
}

document.getElementById('ai-provider-select').addEventListener('change', (e) => {
    currentConfig.aiProvider = e.target.value;
});

document.getElementById('ollama-model-input').addEventListener('input', (e) => {
    currentConfig.ollamaModel = e.target.value.trim();
});

// AI Studio link
const aiStudioLink = document.getElementById('ai-studio-link');
if (aiStudioLink) {
    aiStudioLink.addEventListener('click', () => {
        window.settingsApi.openExternal('https://aistudio.google.com/');
    });
}

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

// TTS slider UI updates
if (document.getElementById('tts-volume')) {
    document.getElementById('tts-volume').addEventListener('input', (e) => {
        document.getElementById('tts-volume-val').textContent = `${Math.round(e.target.value * 100)}%`;
    });
}
if (document.getElementById('tts-speed')) {
    document.getElementById('tts-speed').addEventListener('input', (e) => {
        document.getElementById('tts-speed-val').textContent = `${parseFloat(e.target.value).toFixed(1)}x`;
    });
}
if (document.getElementById('tts-pitch')) {
    document.getElementById('tts-pitch').addEventListener('input', (e) => {
        document.getElementById('tts-pitch-val').textContent = parseFloat(e.target.value).toFixed(1);
    });
}

// Save
document.getElementById('save-btn').addEventListener('click', async () => {
    currentConfig.apiKey = document.getElementById('api-key-input').value.trim();
    
    const anthropicInput = document.getElementById('anthropic-api-key-input');
    if (anthropicInput) currentConfig.anthropicApiKey = anthropicInput.value.trim();

    const openaiInput = document.getElementById('openai-api-key-input');
    if (openaiInput) currentConfig.openaiApiKey = openaiInput.value.trim();

    const hfInput = document.getElementById('hf-api-key');
    if (hfInput) currentConfig.hfApiKey = hfInput.value.trim();

    const ollamaHostInput = document.getElementById('ollama-host-input');
    if (ollamaHostInput) currentConfig.ollamaHost = ollamaHostInput.value.trim();

    currentConfig.geminiModel = getSelectedModel();
    currentConfig.aiProvider = document.getElementById('ai-provider-select').value;
    currentConfig.ollamaModel = document.getElementById('ollama-model-input').value.trim();

    currentConfig.openaiModel = getSelectedModelGeneric('openai-model-select', 'openai-model-custom', DEFAULT_CONFIG.openaiModel || 'gpt-4o');
    currentConfig.anthropicModel = getSelectedModelGeneric('anthropic-model-select', 'anthropic-model-custom', DEFAULT_CONFIG.anthropicModel || 'claude-3-5-sonnet-latest');

    const hfModelInput = document.getElementById('hf-model-name');
    if (hfModelInput) currentConfig.hfModel = hfModelInput.value.trim();

    const localApiBaseUrlInput = document.getElementById('local-api-base-url');
    if (localApiBaseUrlInput) currentConfig.localApiBaseUrl = localApiBaseUrlInput.value.trim();

    const localModelNameInput = document.getElementById('local-model-name');
    if (localModelNameInput) currentConfig.localModelName = localModelNameInput.value.trim();
    
    const voiceReplyToggle = document.getElementById('enable-voice-reply');
    if (voiceReplyToggle) {
        currentConfig.enableVoiceReply = voiceReplyToggle.checked;
    }

    const ttsProviderSelect = document.getElementById('tts-provider-select');
    if (ttsProviderSelect) currentConfig.ttsProvider = ttsProviderSelect.value;
    
    if (document.getElementById('tts-volume')) currentConfig.ttsVolume = parseFloat(document.getElementById('tts-volume').value);
    if (document.getElementById('tts-speed')) currentConfig.ttsSpeed = parseFloat(document.getElementById('tts-speed').value);
    if (document.getElementById('tts-pitch')) currentConfig.ttsPitch = parseFloat(document.getElementById('tts-pitch').value);

    const enableWorkflowsToggle = document.getElementById('enable-custom-workflows');
    if (enableWorkflowsToggle) {
        currentConfig.enableCustomWorkflows = enableWorkflowsToggle.checked;
    }

    const enableAgentCollaborationToggle = document.getElementById('enable-agent-collaboration');
    if (enableAgentCollaborationToggle) {
        currentConfig.enableAgentCollaboration = enableAgentCollaborationToggle.checked;
    }

    const showWidgetToggle = document.getElementById('show-desktop-widget');
    if (showWidgetToggle) {
        currentConfig.showDesktopWidget = showWidgetToggle.checked;
    }

    currentConfig.automationBrowser = document.getElementById('automation-browser-select').value;
    currentConfig.proactiveInterval = Number(document.getElementById('proactive-interval').value);
    currentConfig.proactiveCooldown = Number(document.getElementById('proactive-cooldown').value);
    currentConfig.systemTextColor = document.getElementById('system-text-color').value;
    currentConfig.glassBlur = document.getElementById('glass-blur-select').value;
    currentConfig.fontFamily = document.getElementById('font-family-select').value;
    
    currentConfig.customBgStart = document.getElementById('custom-bg-start').value;
    currentConfig.customBgEnd = document.getElementById('custom-bg-end').value;
    currentConfig.customPanelBg = document.getElementById('custom-panel-bg').value;

    // Ensure mcpServers is part of the saved config
    if (!currentConfig.mcpServers) currentConfig.mcpServers = {};

    console.log('[Settings] Saving config with MCP servers:', Object.keys(currentConfig.mcpServers).length);
    await window.settingsApi.saveSettings(currentConfig);
    const btn = document.getElementById('save-btn');
    btn.textContent = '✅ Saved!';
    setTimeout(() => { btn.textContent = 'Save Settings'; }, 1500);
});

// --- MCP Management Functions ---
function renderMcpServers() {
    console.log('[Settings] Rendering MCP Servers UI...');
    const list = document.getElementById('mcp-server-list');
    if (!list) {
        console.warn('[Settings] MCP list element not found in DOM.');
        return;
    }
    list.innerHTML = '';

    const servers = currentConfig.mcpServers || {};
    const entries = Object.entries(servers);
    console.log(`[Settings] Found ${entries.length} servers in currentConfig.`);

    if (entries.length === 0) {
        list.innerHTML = '<p class="hint" style="text-align: center; padding: 10px;">No MCP servers connected.</p>';
        return;
    }

    for (const [name, cfg] of entries) {
        const item = document.createElement('div');
        item.style.cssText = 'display: flex; justify-content: space-between; align-items: center; padding: 12px; background: rgba(0,0,0,0.2); border-radius: 10px; border: 1px solid var(--border);';
        
        item.innerHTML = `
            <div style="display: flex; flex-direction: column; gap: 4px;">
                <div style="font-weight: 600; color: var(--accent); display: flex; align-items: center; gap: 6px;">
                    <span>🌐 ${name}</span>
                </div>
                <div style="font-size: 0.75rem; opacity: 0.7; font-family: monospace;">${cfg.command} ${cfg.args.join(' ')}</div>
            </div>
            <button class="btn-danger" style="padding: 6px 12px; font-size: 0.8rem;" onclick="removeMcpServer('${name}')">Remove</button>
        `;
        list.appendChild(item);
    }
}

window.removeMcpServer = function(name) {
    if (confirm(`Remove MCP server "${name}"?`)) {
        delete currentConfig.mcpServers[name];
        renderMcpServers();
    }
};

document.getElementById('btn-add-mcp').addEventListener('click', () => {
    const nameInput = document.getElementById('mcp-new-name');
    const cmdInput = document.getElementById('mcp-new-command');
    const argsInput = document.getElementById('mcp-new-args');
    const envInput = document.getElementById('mcp-new-env');

    const name = nameInput.value.trim();
    const command = cmdInput.value.trim();
    const argsStr = argsInput.value.trim();
    const envStr = envInput.value.trim();

    if (!name || !command) {
        alert('Name and Command are required!');
        return;
    }

    // Basic args split (by space, but respecting some quotes if possible - simple for now)
    const args = argsStr ? argsStr.split(/\s+/) : [];
    
    let env = {};
    if (envStr) {
        try {
            env = JSON.parse(envStr);
        } catch (e) {
            alert('Invalid JSON in Environment Variables field!');
            return;
        }
    }

    if (!currentConfig.mcpServers) currentConfig.mcpServers = {};
    currentConfig.mcpServers[name] = { command, args, env };

    // Clear inputs
    nameInput.value = '';
    cmdInput.value = '';
    argsInput.value = '';
    envInput.value = '';

    renderMcpServers();
});

// Custom Workflows functionality
const openWorkflowsBtn = document.getElementById('open-workflows-btn');
const reloadWorkflowsBtn = document.getElementById('reload-workflows-btn');
if (openWorkflowsBtn) {
    openWorkflowsBtn.addEventListener('click', () => {
        window.settingsApi.openCustomWorkflows();
    });
}
if (reloadWorkflowsBtn) {
    reloadWorkflowsBtn.addEventListener('click', async () => {
        await window.settingsApi.reloadCustomWorkflows();
        const originalText = reloadWorkflowsBtn.textContent;
        reloadWorkflowsBtn.textContent = '✅ Reloaded!';
        setTimeout(() => { reloadWorkflowsBtn.textContent = originalText; }, 1500);
    });
}

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

function updatePluginButton(pluginName, isEnabled) {
    const btn = document.getElementById(`btn-plugin-${pluginName}`);
    if (!btn) return;
    
    if (isEnabled) {
        btn.textContent = 'Disconnect';
        btn.classList.remove('btn-connect');
        btn.classList.add('btn-disconnect');
    } else {
        btn.textContent = 'Connect';
        btn.classList.add('btn-connect');
        btn.classList.remove('btn-disconnect');
    }
}

// Bind plugin buttons
['spotify', 'calendar', 'discord'].forEach(plugin => {
    const btn = document.getElementById(`btn-plugin-${plugin}`);
    if (btn) {
        btn.addEventListener('click', () => {
            const key = `plugin${plugin.charAt(0).toUpperCase() + plugin.slice(1)}Enabled`;
            currentConfig[key] = !currentConfig[key];
            updatePluginButton(plugin, currentConfig[key]);
        });
    }
});

// Init
// Sidebar Tab Navigation
document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.addEventListener('click', () => {
        const target = btn.dataset.target;
        // Deactivate all
        document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
        document.querySelectorAll('.tab-pane').forEach(p => p.classList.remove('active'));
        // Activate selected
        btn.classList.add('active');
        const pane = document.getElementById(target);
        if (pane) pane.classList.add('active');

        // Re-render MCP list if switching to plugins tab
        if (target === 'sect-plugins') {
            renderMcpServers();
        }
    });
});

window.addEventListener('DOMContentLoaded', loadSettings);
