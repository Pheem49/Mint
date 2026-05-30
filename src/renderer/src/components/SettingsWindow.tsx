import React, { useEffect, useState } from 'react'
import '../styles/settings.css'

export default function SettingsWindow() {
    const [activeTab, setActiveTab] = useState('sect-general')
    const [config, setConfig] = useState<any>(null)
    const [showCustomGemini, setShowCustomGemini] = useState(false)
    const [showCustomOpenAI, setShowCustomOpenAI] = useState(false)
    const [showCustomAnthropic, setShowCustomAnthropic] = useState(false)

    // MCP Server states
    const [mcpServers, setMcpServers] = useState<any>({})
    const [newMcpName, setNewMcpName] = useState('')
    const [newMcpCommand, setNewMcpCommand] = useState('')
    const [newMcpArgs, setNewMcpArgs] = useState('')
    const [newMcpEnv, setNewMcpEnv] = useState('')

    useEffect(() => {
        if (window.settingsApi) {
            window.settingsApi.getSettings().then((cfg) => {
                setConfig(cfg || {})
                setMcpServers(cfg?.mcpServers || {})

                // Check model selections to see if custom field is needed
                const isCustomGemini = cfg?.geminiModel && !['gemini-2.5-flash', 'gemini-3.1-flash-lite', 'gemini-3.1-flash-lite-preview'].includes(cfg.geminiModel)
                setShowCustomGemini(isCustomGemini)

                const isCustomOpenAI = cfg?.openaiModel && !['gpt-4o', 'gpt-4o-mini', 'o1-preview', 'o1-mini'].includes(cfg.openaiModel)
                setShowCustomOpenAI(isCustomOpenAI)

                const isCustomAnthropic = cfg?.anthropicModel && !['claude-3-5-sonnet-latest', 'claude-3-opus-latest', 'claude-3-5-haiku-latest'].includes(cfg.anthropicModel)
                setShowCustomAnthropic(isCustomAnthropic)

                applyTheme(cfg)
            })
        }
    }, [])

    const applyTheme = (cfg: any) => {
        document.documentElement.setAttribute('data-theme', cfg.theme || 'dark')
        if (cfg.systemTextColor) {
            document.documentElement.style.setProperty('--text-main', cfg.systemTextColor)
        }
        if (cfg.theme === 'custom') {
            if (cfg.customBgStart && cfg.customBgEnd) {
                const gradient = `linear-gradient(135deg, ${cfg.customBgStart} 0%, ${cfg.customBgEnd} 100%)`
                document.documentElement.style.setProperty('--bg-gradient', gradient)
            }
            if (cfg.customPanelBg) {
                const rgb = hexToRgb(cfg.customPanelBg)
                document.documentElement.style.setProperty('--panel-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.75)`)
            }
        }
    }

    const hexToRgb = (hex: string) => {
        const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
        return result ? {
            r: parseInt(result[1], 16),
            g: parseInt(result[2], 16),
            b: parseInt(result[3], 16)
        } : { r: 15, g: 23, b: 42 }
    }

    const handleConfigChange = (key: string, value: any) => {
        setConfig((prev: any) => ({ ...prev, [key]: value }))
    }

    const handleSave = async () => {
        if (!config || !window.settingsApi) return
        
        const finalConfig = {
            ...config,
            mcpServers
        }

        const result = await window.settingsApi.saveSettings(finalConfig)
        if (result && result.success !== false) {
            alert('Settings saved successfully!')
        } else {
            alert('Failed to save settings: ' + (result?.message || 'Unknown error'))
        }
    }

    const handleReset = () => {
        if (window.settingsApi) {
            window.settingsApi.getSettings().then((cfg) => {
                setConfig(cfg)
                setMcpServers(cfg?.mcpServers || {})
                applyTheme(cfg)
            })
        }
    }

    const handleQuit = () => {
        if (window.settingsApi) {
            window.settingsApi.quitApp()
        }
    }

    const handleClose = () => {
        if (window.settingsApi) {
            window.settingsApi.closeSettings()
        }
    }

    const handleOpenWorkflows = () => {
        window.settingsApi?.openCustomWorkflows()
    }

    const handleReloadWorkflows = async () => {
        const result = await window.settingsApi?.reloadCustomWorkflows()
        if (result && result.success) {
            alert('Custom workflows reloaded!')
        }
    }

    const handleAddMcp = () => {
        if (!newMcpName.trim() || !newMcpCommand.trim()) {
            alert('Server Name and Command are required!')
            return
        }

        let envObj = {}
        if (newMcpEnv.trim()) {
            try {
                envObj = JSON.parse(newMcpEnv)
            } catch (e) {
                alert('Invalid Environment JSON!')
                return
            }
        }

        const argsArray = newMcpArgs.split(' ').filter((a) => a.trim() !== '')

        const updated = {
            ...mcpServers,
            [newMcpName.trim()]: {
                command: newMcpCommand.trim(),
                args: argsArray,
                env: envObj
            }
        }

        setMcpServers(updated)
        setNewMcpName('')
        setNewMcpCommand('')
        setNewMcpArgs('')
        setNewMcpEnv('')
    }

    const handleRemoveMcp = (name: string) => {
        const updated = { ...mcpServers }
        delete updated[name]
        setMcpServers(updated)
    }

    if (!config) {
        return <div style={{ color: 'white', padding: '20px' }}>Loading Settings...</div>
    }

    return (
        <div className="settings-container">
            <header className="settings-header drag-region">
                <div className="header-left">
                    <span className="settings-icon">⚙</span>
                    <div>
                        <h1>Settings</h1>
                        <p>Configure Mint assistant behavior and integrations.</p>
                    </div>
                </div>
                <button className="close-btn" onClick={handleClose} aria-label="Close">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                        stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <line x1="18" y1="6" x2="6" y2="18"></line>
                        <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                </button>
            </header>

            <main className="settings-body">
                <nav className="settings-sidebar" aria-label="Settings sections">
                    <button className={`tab-btn ${activeTab === 'sect-general' ? 'active' : ''}`} onClick={() => setActiveTab('sect-general')}>
                        <span>⚙</span><strong>General</strong>
                    </button>
                    <button className={`tab-btn ${activeTab === 'sect-audio' ? 'active' : ''}`} onClick={() => setActiveTab('sect-audio')}>
                        <span>🔊</span><strong>Audio & Voice</strong>
                    </button>
                    <button className={`tab-btn ${activeTab === 'sect-automation' ? 'active' : ''}`} onClick={() => setActiveTab('sect-automation')}>
                        <span>🤖</span><strong>Automation</strong>
                    </button>
                    <button className={`tab-btn ${activeTab === 'sect-theme' ? 'active' : ''}`} onClick={() => setActiveTab('sect-theme')}>
                        <span>🎨</span><strong>Theme & UI</strong>
                    </button>
                    <button className={`tab-btn ${activeTab === 'sect-plugins' ? 'active' : ''}`} onClick={() => setActiveTab('sect-plugins')}>
                        <span>🧩</span><strong>Plugins</strong>
                    </button>
                    <button className={`tab-btn ${activeTab === 'sect-shortcuts' ? 'active' : ''}`} onClick={() => setActiveTab('sect-shortcuts')}>
                        <span>⌨</span><strong>Shortcuts</strong>
                    </button>
                </nav>

                <div className="settings-content">
                    {/* General Settings */}
                    {activeTab === 'sect-general' && (
                        <div className="tab-pane active" id="sect-general">
                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">AI routing</p>
                                        <h2 className="section-title">Provider & Model</h2>
                                    </div>
                                    <p className="section-description">Choose which AI backend Mint uses, then set the default model for each provider.</p>
                                </div>

                                <div className="form-grid compact">
                                    <div className="setting-row wide">
                                        <label htmlFor="ai-provider-select">Active Provider</label>
                                        <select id="ai-provider-select" value={config.aiProvider || 'gemini'} onChange={(e) => handleConfigChange('aiProvider', e.target.value)}>
                                            <option value="gemini">Google Gemini (Cloud)</option>
                                            <option value="anthropic">Anthropic Claude</option>
                                            <option value="openai">OpenAI</option>
                                            <option value="ollama">Ollama (Local / Private)</option>
                                            <option value="huggingface">Hugging Face (Inference API)</option>
                                            <option value="local_openai">Local (LM Studio / OpenAI Compatible)</option>
                                        </select>
                                    </div>

                                    {/* Gemini Model */}
                                    <div className="setting-row">
                                        <label htmlFor="gemini-model-select">Gemini Model</label>
                                        <select id="gemini-model-select" value={showCustomGemini ? 'custom' : (config.geminiModel || 'gemini-2.5-flash')} onChange={(e) => {
                                            if (e.target.value === 'custom') {
                                                setShowCustomGemini(true)
                                            } else {
                                                setShowCustomGemini(false)
                                                handleConfigChange('geminiModel', e.target.value)
                                            }
                                        }}>
                                            <option value="gemini-2.5-flash">gemini-2.5-flash</option>
                                            <option value="gemini-3.1-flash-lite">gemini-3.1-flash-lite</option>
                                            <option value="gemini-3.1-flash-lite-preview">gemini-3.1-flash-lite-preview</option>
                                            <option value="custom">Custom...</option>
                                        </select>
                                    </div>
                                    {showCustomGemini && (
                                        <div className="setting-row" id="gemini-model-custom-row">
                                            <label htmlFor="gemini-model-custom">Custom Gemini Model</label>
                                            <input type="text" id="gemini-model-custom" value={config.geminiModel || ''} onChange={(e) => handleConfigChange('geminiModel', e.target.value)} placeholder="e.g. gemini-3.1-flash-lite-preview" />
                                        </div>
                                    )}

                                    {/* OpenAI Model */}
                                    <div className="setting-row">
                                        <label htmlFor="openai-model-select">OpenAI Model</label>
                                        <select id="openai-model-select" value={showCustomOpenAI ? 'custom' : (config.openaiModel || 'gpt-4o')} onChange={(e) => {
                                            if (e.target.value === 'custom') {
                                                setShowCustomOpenAI(true)
                                            } else {
                                                setShowCustomOpenAI(false)
                                                handleConfigChange('openaiModel', e.target.value)
                                            }
                                        }}>
                                            <option value="gpt-4o">gpt-4o</option>
                                            <option value="gpt-4o-mini">gpt-4o-mini</option>
                                            <option value="o1-preview">o1-preview</option>
                                            <option value="o1-mini">o1-mini</option>
                                            <option value="custom">Custom...</option>
                                        </select>
                                    </div>
                                    {showCustomOpenAI && (
                                        <div className="setting-row" id="openai-model-custom-row">
                                            <label htmlFor="openai-model-custom">Custom OpenAI Model</label>
                                            <input type="text" id="openai-model-custom" value={config.openaiModel || ''} onChange={(e) => handleConfigChange('openaiModel', e.target.value)} placeholder="e.g. gpt-4o" />
                                        </div>
                                    )}

                                    {/* Anthropic Model */}
                                    <div className="setting-row">
                                        <label htmlFor="anthropic-model-select">Anthropic Model</label>
                                        <select id="anthropic-model-select" value={showCustomAnthropic ? 'custom' : (config.anthropicModel || 'claude-3-5-sonnet-latest')} onChange={(e) => {
                                            if (e.target.value === 'custom') {
                                                setShowCustomAnthropic(true)
                                            } else {
                                                setShowCustomAnthropic(false)
                                                handleConfigChange('anthropicModel', e.target.value)
                                            }
                                        }}>
                                            <option value="claude-3-5-sonnet-latest">claude-3-5-sonnet-latest</option>
                                            <option value="claude-3-opus-latest">claude-3-opus-latest</option>
                                            <option value="claude-3-5-haiku-latest">claude-3-5-haiku-latest</option>
                                            <option value="custom">Custom...</option>
                                        </select>
                                    </div>
                                    {showCustomAnthropic && (
                                        <div className="setting-row" id="anthropic-model-custom-row">
                                            <label htmlFor="anthropic-model-custom">Custom Anthropic Model</label>
                                            <input type="text" id="anthropic-model-custom" value={config.anthropicModel || ''} onChange={(e) => handleConfigChange('anthropicModel', e.target.value)} placeholder="e.g. claude-3-5-sonnet-latest" />
                                        </div>
                                    )}

                                    <div className="setting-row">
                                        <label htmlFor="hf-model-name">Hugging Face Model</label>
                                        <input type="text" id="hf-model-name" value={config.hfModel || ''} onChange={(e) => handleConfigChange('hfModel', e.target.value)} placeholder="e.g. meta-llama/Meta-Llama-3-8B-Instruct" />
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="local-model-name">LM Studio Model</label>
                                        <input type="text" id="local-model-name" value={config.localModelName || ''} onChange={(e) => handleConfigChange('localModelName', e.target.value)} placeholder="e.g. local-model" />
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="ollama-model-input">Ollama Model</label>
                                        <input type="text" id="ollama-model-input" value={config.ollamaModel || ''} onChange={(e) => handleConfigChange('ollamaModel', e.target.value)} placeholder="e.g. llama3:latest" />
                                    </div>
                                </div>
                            </section>

                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Credentials</p>
                                        <h2 class="section-title">API Keys & Hosts</h2>
                                    </div>
                                    <p className="section-description">Cloud providers need API keys. Local providers need host URLs.</p>
                                </div>

                                <div className="form-grid">
                                    <div className="setting-row">
                                        <label htmlFor="api-key-input">Gemini API Key</label>
                                        <input type="password" id="api-key-input" value={config.apiKey || ''} onChange={(e) => handleConfigChange('apiKey', e.target.value)} placeholder="Enter Gemini API Key..." autoComplete="off" />
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="openai-api-key-input">OpenAI API Key</label>
                                        <input type="password" id="openai-api-key-input" value={config.openaiApiKey || ''} onChange={(e) => handleConfigChange('openaiApiKey', e.target.value)} placeholder="Enter OpenAI API Key..." autoComplete="off" />
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="anthropic-api-key-input">Anthropic API Key</label>
                                        <input type="password" id="anthropic-api-key-input" value={config.anthropicApiKey || ''} onChange={(e) => handleConfigChange('anthropicApiKey', e.target.value)} placeholder="Enter Anthropic API Key..." autoComplete="off" />
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="hf-api-key">Hugging Face API Key</label>
                                        <input type="password" id="hf-api-key" value={config.hfApiKey || ''} onChange={(e) => handleConfigChange('hfApiKey', e.target.value)} placeholder="Enter Hugging Face API Key..." autoComplete="off" />
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="local-api-base-url">LM Studio Base URL</label>
                                        <input type="text" id="local-api-base-url" value={config.localApiBaseUrl || ''} onChange={(e) => handleConfigChange('localApiBaseUrl', e.target.value)} placeholder="e.g. http://localhost:1234/v1" />
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="ollama-host-input">Ollama Host</label>
                                        <input type="text" id="ollama-host-input" value={config.ollamaHost || ''} onChange={(e) => handleConfigChange('ollamaHost', e.target.value)} placeholder="e.g. http://localhost:11434" />
                                    </div>
                                </div>
                            </section>

                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Desktop</p>
                                        <h2 className="section-title">Assistant Presence</h2>
                                    </div>
                                </div>
                                <div className="toggle-row">
                                    <div>
                                        <label htmlFor="show-desktop-widget">Show Desktop AI Candidate</label>
                                        <p className="hint">Show the mini AI character on your desktop.</p>
                                    </div>
                                    <label className="toggle-switch">
                                        <input type="checkbox" id="show-desktop-widget" checked={config.showDesktopWidget !== false} onChange={(e) => handleConfigChange('showDesktopWidget', e.target.checked)} />
                                        <span className="toggle-slider"></span>
                                    </label>
                                </div>
                            </section>
                        </div>
                    )}

                    {/* Audio & Voice Settings */}
                    {activeTab === 'sect-audio' && (
                        <div className="tab-pane active" id="sect-audio">
                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Speech</p>
                                        <h2 className="section-title">Voice Reply</h2>
                                    </div>
                                    <p className="section-description">Control spoken responses and TTS behavior.</p>
                                </div>

                                <div className="toggle-row">
                                    <div>
                                        <label htmlFor="enable-voice-reply">Enable Voice Reply</label>
                                        <p className="hint">Mint will speak responses out loud when this is enabled.</p>
                                    </div>
                                    <label className="toggle-switch">
                                        <input type="checkbox" id="enable-voice-reply" checked={config.enableVoiceReply !== false} onChange={(e) => handleConfigChange('enableVoiceReply', e.target.checked)} />
                                        <span className="toggle-slider"></span>
                                    </label>
                                </div>

                                <div className="form-grid single">
                                    <div className="setting-row">
                                        <label htmlFor="tts-provider-select">Voice Engine</label>
                                        <select id="tts-provider-select" value={config.ttsProvider || 'google'} onChange={(e) => handleConfigChange('ttsProvider', e.target.value)}>
                                            <option value="google">Google Cloud (Natural, Auto Lang)</option>
                                            <option value="native">OS Native (Supports Pitch)</option>
                                        </select>
                                    </div>
                                </div>

                                <div className="slider-stack">
                                    <div className="setting-row">
                                        <label htmlFor="tts-volume">Volume</label>
                                        <div className="slider-group">
                                            <input type="range" id="tts-volume" className="range-slider" min="0" max="1" step="0.1" value={config.ttsVolume !== undefined ? config.ttsVolume : 1} onChange={(e) => handleConfigChange('ttsVolume', parseFloat(e.target.value))} />
                                            <span className="range-value" id="tts-volume-val">{Math.round((config.ttsVolume !== undefined ? config.ttsVolume : 1) * 100)}%</span>
                                        </div>
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="tts-speed">Speed</label>
                                        <div className="slider-group">
                                            <input type="range" id="tts-speed" className="range-slider" min="0.5" max="2" step="0.1" value={config.ttsSpeed !== undefined ? config.ttsSpeed : 1} onChange={(e) => handleConfigChange('ttsSpeed', parseFloat(e.target.value))} />
                                            <span className="range-value" id="tts-speed-val">{(config.ttsSpeed !== undefined ? config.ttsSpeed : 1).toFixed(1)}x</span>
                                        </div>
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="tts-pitch">Pitch</label>
                                        <div className="slider-group">
                                            <input type="range" id="tts-pitch" className="range-slider" min="0" max="2" step="0.1" value={config.ttsPitch !== undefined ? config.ttsPitch : 1} onChange={(e) => handleConfigChange('ttsPitch', parseFloat(e.target.value))} />
                                            <span className="range-value" id="tts-pitch-val">{(config.ttsPitch !== undefined ? config.ttsPitch : 1).toFixed(1)}</span>
                                        </div>
                                        <p className="hint">Pitch applies to OS native voice only.</p>
                                    </div>
                                </div>
                            </section>
                        </div>
                    )}

                    {/* Automation Settings */}
                    {activeTab === 'sect-automation' && (
                        <div className="tab-pane active" id="sect-automation">
                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Browser</p>
                                        <h2 className="section-title">Automation Engine</h2>
                                    </div>
                                </div>
                                <div className="form-grid single">
                                    <div className="setting-row">
                                        <label htmlFor="automation-browser-select">Browser Engine</label>
                                        <select id="automation-browser-select" value={config.automationBrowser || 'chromium'} onChange={(e) => handleConfigChange('automationBrowser', e.target.value)}>
                                            <option value="chromium">Chromium (Bundled)</option>
                                            <option value="/usr/bin/firefox">Firefox (System - Linux)</option>
                                        </select>
                                    </div>
                                </div>
                            </section>

                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Awareness</p>
                                        <h2 className="section-title">Proactive Assistant</h2>
                                    </div>
                                    <p className="section-description">Tune screen analysis frequency and suggestion timing.</p>
                                </div>
                                <div className="slider-stack">
                                    <div className="setting-row">
                                        <label htmlFor="proactive-interval">Screen Capture Frequency</label>
                                        <div className="slider-group">
                                            <input type="range" id="proactive-interval" min="30" max="300" step="30" value={config.proactiveInterval || 60} onChange={(e) => handleConfigChange('proactiveInterval', parseInt(e.target.value))} className="range-slider" />
                                            <span className="range-value" id="proactive-interval-display">{config.proactiveInterval || 60} sec</span>
                                        </div>
                                        <p className="hint">Lower values respond faster but use more API calls.</p>
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="proactive-cooldown">Suggestion Cooldown</label>
                                        <div className="slider-group">
                                            <input type="range" id="proactive-cooldown" min="60" max="600" step="60" value={config.proactiveCooldown || 120} onChange={(e) => handleConfigChange('proactiveCooldown', parseInt(e.target.value))} className="range-slider" />
                                            <span className="range-value" id="proactive-cooldown-display">{Math.round((config.proactiveCooldown || 120) / 60)} min</span>
                                        </div>
                                        <p className="hint">Minimum time between repeat suggestions.</p>
                                    </div>
                                </div>
                            </section>

                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Rules</p>
                                        <h2 className="section-title">Custom Workflows</h2>
                                    </div>
                                </div>
                                <div className="toggle-row">
                                    <div>
                                        <label htmlFor="enable-custom-workflows">Enable Custom Workflows</label>
                                        <p className="hint">Run "If This Then Mint" rules from the workflow JSON file.</p>
                                    </div>
                                    <label className="toggle-switch">
                                        <input type="checkbox" id="enable-custom-workflows" checked={config.enableCustomWorkflows !== false} onChange={(e) => handleConfigChange('enableCustomWorkflows', e.target.checked)} />
                                        <span className="toggle-slider"></span>
                                    </label>
                                </div>
                                <div className="button-row">
                                    <button className="btn-secondary" onClick={handleOpenWorkflows}>Open workflows.json</button>
                                    <button className="btn-primary" onClick={handleReloadWorkflows}>Reload Rules</button>
                                </div>
                            </section>

                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Code mode</p>
                                        <h2 className="section-title">Agent Collaboration</h2>
                                    </div>
                                </div>
                                <div className="toggle-row">
                                    <div>
                                        <label htmlFor="enable-agent-collaboration">Enable Multi-Agent Review</label>
                                        <p className="hint">Allow a secondary model to review code written by the primary model.</p>
                                    </div>
                                    <label className="toggle-switch">
                                        <input type="checkbox" id="enable-agent-collaboration" checked={config.enableAgentCollaboration === true} onChange={(e) => handleConfigChange('enableAgentCollaboration', e.target.checked)} />
                                        <span className="toggle-slider"></span>
                                    </label>
                                </div>
                            </section>
                        </div>
                    )}

                    {/* Theme Settings */}
                    {activeTab === 'sect-theme' && (
                        <div className="tab-pane active" id="sect-theme">
                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Appearance</p>
                                        <h2 className="section-title">Theme</h2>
                                    </div>
                                </div>
                                <div className="theme-grid">
                                    {['dark', 'light', 'midnight', 'custom'].map((theme) => (
                                        <button
                                            key={theme}
                                            className={`theme-card ${config.theme === theme ? 'selected' : ''}`}
                                            onClick={() => {
                                                handleConfigChange('theme', theme)
                                                applyTheme({ ...config, theme })
                                            }}
                                        >
                                            <div className={`theme-preview ${theme}-preview`} />
                                            <span>{theme.charAt(0).toUpperCase() + theme.slice(1)}</span>
                                        </button>
                                    ))}
                                </div>

                                {config.theme === 'custom' && (
                                    <div className="custom-theme-panel">
                                        <div className="setting-row">
                                            <label>Background Gradient</label>
                                            <div className="color-range">
                                                <input type="color" value={config.customBgStart || '#0f172a'} onChange={(e) => {
                                                    handleConfigChange('customBgStart', e.target.value)
                                                    applyTheme({ ...config, customBgStart: e.target.value })
                                                }} />
                                                <span>→</span>
                                                <input type="color" value={config.customBgEnd || '#1e1b4b'} onChange={(e) => {
                                                    handleConfigChange('customBgEnd', e.target.value)
                                                    applyTheme({ ...config, customBgEnd: e.target.value })
                                                }} />
                                            </div>
                                        </div>
                                        <div className="setting-row">
                                            <label htmlFor="custom-panel-bg">Panel Background</label>
                                            <input type="color" value={config.customPanelBg || '#1e293b'} onChange={(e) => {
                                                handleConfigChange('customPanelBg', e.target.value)
                                                applyTheme({ ...config, customPanelBg: e.target.value })
                                            }} />
                                        </div>
                                    </div>
                                )}
                            </section>

                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Color</p>
                                        <h2 className="section-title">Accent & Text</h2>
                                    </div>
                                </div>
                                <div className="color-section">
                                    <div>
                                        <label>Accent Color Presets</label>
                                        <div className="color-presets">
                                            {['#8b5cf6', '#06b6d4', '#10b981', '#f59e0b', '#ef4444', '#ec4899'].map((presetColor) => (
                                                <button
                                                    key={presetColor}
                                                    className="color-dot"
                                                    style={{ backgroundColor: presetColor }}
                                                    onClick={() => {
                                                        handleConfigChange('accentColor', presetColor)
                                                        applyTheme({ ...config, accentColor: presetColor })
                                                    }}
                                                />
                                            ))}
                                        </div>
                                    </div>
                                    <div className="color-inputs">
                                        <label htmlFor="custom-color">Custom Accent</label>
                                        <input type="color" id="custom-color" value={config.accentColor || '#8b5cf6'} onChange={(e) => {
                                            handleConfigChange('accentColor', e.target.value)
                                            applyTheme({ ...config, accentColor: e.target.value })
                                        }} />
                                        <label htmlFor="system-text-color">System Text</label>
                                        <input type="color" id="system-text-color" value={config.systemTextColor || '#f8fafc'} onChange={(e) => {
                                            handleConfigChange('systemTextColor', e.target.value)
                                            applyTheme({ ...config, systemTextColor: e.target.value })
                                        }} />
                                    </div>
                                </div>
                            </section>

                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Surface</p>
                                        <h2 className="section-title">Interface Style</h2>
                                    </div>
                                </div>
                                <div className="form-grid">
                                    <div className="setting-row">
                                        <label htmlFor="glass-blur-select">Glass Blur</label>
                                        <select id="glass-blur-select" value={config.glassBlur || 'blur(16px)'} onChange={(e) => handleConfigChange('glassBlur', e.target.value)}>
                                            <option value="blur(4px)">Low (4px)</option>
                                            <option value="blur(16px)">Medium (16px) - Default</option>
                                            <option value="blur(32px)">High (32px)</option>
                                            <option value="none">Off (Solid)</option>
                                        </select>
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="font-family-select">Font Family</label>
                                        <select id="font-family-select" value={config.fontFamily || "'Outfit', sans-serif"} onChange={(e) => handleConfigChange('fontFamily', e.target.value)}>
                                            <option value="'Outfit', sans-serif">Outfit (Default)</option>
                                            <option value="'Mali', cursive">Mali (Cute Thai Font)</option>
                                            <option value="'Prompt', sans-serif">Prompt (Modern Thai)</option>
                                            <option value="'Sarabun', sans-serif">Sarabun (Formal Thai)</option>
                                        </select>
                                    </div>
                                    <div className="setting-row">
                                        <label htmlFor="font-size-select">Font Size</label>
                                        <select id="font-size-select" value={config.fontSize || '15px'} onChange={(e) => handleConfigChange('fontSize', e.target.value)}>
                                            <option value="14px">Small</option>
                                            <option value="15px">Medium (Default)</option>
                                            <option value="16px">Large</option>
                                            <option value="17px">Extra Large</option>
                                        </select>
                                    </div>
                                </div>
                            </section>
                        </div>
                    )}

                    {/* Plugins & MCP Settings */}
                    {activeTab === 'sect-plugins' && (
                        <div className="tab-pane active" id="sect-plugins">
                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Integrations</p>
                                        <h2 className="section-title">Built-in Plugins</h2>
                                    </div>
                                </div>
                                <div className="plugin-list">
                                    {[
                                        { id: 'spotify', icon: '🎵', name: 'Spotify', desc: 'Control playback with AI. Requires playerctl.' },
                                        { id: 'calendar', icon: '📅', name: 'Google Calendar', desc: 'Read and schedule events.' },
                                        { id: 'gmail', icon: '✉', name: 'Gmail', desc: 'Read email and create drafts safely.' },
                                        { id: 'notion', icon: '📝', name: 'Notion', desc: 'Create notes, pages, and read databases.' },
                                        { id: 'discord', icon: '💬', name: 'Discord RPC', desc: 'Show "Using Mint Assistant" on Discord status.' },
                                    ].map((plugin) => {
                                        const configKey = `plugin${plugin.id.charAt(0).toUpperCase() + plugin.id.slice(1)}Enabled`
                                        const isConnected = config[configKey] === true
                                        return (
                                            <div className="plugin-card" key={plugin.id}>
                                                <div className="plugin-icon">{plugin.icon}</div>
                                                <div className="plugin-info">
                                                    <div className="plugin-name">{plugin.name}</div>
                                                    <div className="plugin-desc">{plugin.desc}</div>
                                                </div>
                                                <div className="plugin-actions">
                                                    <button
                                                        className={`btn-connect ${isConnected ? 'connected' : ''}`}
                                                        onClick={() => handleConfigChange(configKey, !isConnected)}
                                                    >
                                                        {isConnected ? 'Connected' : 'Connect'}
                                                    </button>
                                                </div>
                                            </div>
                                        )
                                    })}
                                </div>
                            </section>

                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">External tools</p>
                                        <h2 className="section-title">MCP Servers</h2>
                                    </div>
                                    <p className="section-description">Connect Mint to tools like search, GitHub, or filesystem servers.</p>
                                </div>

                                <div className="mcp-list">
                                    {Object.keys(mcpServers).map((serverName) => (
                                        <div className="plugin-card" key={serverName} style={{ marginBottom: '10px', padding: '12px' }}>
                                            <div className="plugin-info">
                                                <div className="plugin-name" style={{ fontSize: '0.95rem' }}>{serverName}</div>
                                                <div className="plugin-desc" style={{ fontSize: '0.8rem', fontFamily: 'monospace' }}>
                                                    {mcpServers[serverName].command} {mcpServers[serverName].args?.join(' ')}
                                                </div>
                                            </div>
                                            <button className="btn-danger" onClick={() => handleRemoveMcp(serverName)} style={{ padding: '4px 10px', fontSize: '0.8rem' }}>
                                                Remove
                                            </button>
                                        </div>
                                    ))}
                                </div>

                                <div className="add-mcp-box">
                                    <h3>Add MCP Server</h3>
                                    <div className="form-grid">
                                        <input type="text" value={newMcpName} onChange={(e) => setNewMcpName(e.target.value)} placeholder="Server Name (e.g. google-search)" />
                                        <input type="text" value={newMcpCommand} onChange={(e) => setNewMcpCommand(e.target.value)} placeholder="Command (e.g. npx)" />
                                    </div>
                                    <input type="text" value={newMcpArgs} onChange={(e) => setNewMcpArgs(e.target.value)} placeholder="Arguments (e.g. -y @modelcontextprotocol/server-brave-search)" />
                                    <textarea value={newMcpEnv} onChange={(e) => setNewMcpEnv(e.target.value)} placeholder='Env JSON, e.g. {"BRAVE_API_KEY": "..."}'></textarea>
                                    <button className="btn-primary full-width" onClick={handleAddMcp}>Add MCP Server</button>
                                </div>
                            </section>
                        </div>
                    )}

                    {/* Shortcuts Settings */}
                    {activeTab === 'sect-shortcuts' && (
                        <div className="tab-pane active" id="sect-shortcuts">
                            <section className="setting-section">
                                <div className="section-heading">
                                    <div>
                                        <p className="section-kicker">Keyboard</p>
                                        <h2 className="section-title">Shortcuts</h2>
                                    </div>
                                </div>
                                <div className="shortcut-list">
                                    <div className="shortcut-item">
                                        <span>Show / Hide Mint Window</span>
                                        <div className="keys"><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>Space</kbd></div>
                                    </div>
                                    <div className="shortcut-item">
                                        <span>Open Spotlight</span>
                                        <div className="keys"><kbd>Alt</kbd><kbd>Space</kbd></div>
                                    </div>
                                    <div className="shortcut-item">
                                        <span>Close / Dismiss</span>
                                        <div className="keys"><kbd>Esc</kbd></div>
                                    </div>
                                </div>
                            </section>
                        </div>
                    )}
                </div>
            </main>

            <footer className="settings-footer">
                <button className="btn-danger" onClick={handleQuit}>Quit Application</button>
                <div className="footer-actions">
                    <button className="btn-secondary" onClick={handleReset}>Reset to Default</button>
                    <button className="btn-primary" onClick={handleSave}>Save Settings</button>
                </div>
            </footer>
        </div>
    )
}
