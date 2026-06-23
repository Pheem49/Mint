import React, { useState, useEffect } from 'react'
import { getProfileValue, setProfileValue } from '../tauri'

const DEFAULT_CONFIG = {
  theme: 'dark',
  accentColor: '#10b981',
  systemTextColor: '#f8fafc',
  customBgStart: '#0f172a',
  customBgEnd: '#1e1b4b',
  customPanelBg: '#1e293b',
  glassBlur: 'blur(16px)',
  fontFamily: "'Outfit', sans-serif",
  fontSize: '18px',
  apiKey: '',
  aiProvider: 'gemini',
  geminiModel: 'gemini-2.5-flash',
  openaiModel: 'gpt-4o',
  openrouterModel: 'openai/gpt-4o-mini',
  deepseekModel: 'deepseek-v4-flash',
  anthropicModel: 'claude-3-5-sonnet-latest',
  ollamaModel: 'llama3:latest',
  language: 'th-TH',
  proactiveInterval: 60,
  proactiveCooldown: 120,
  enableVoiceReply: true,
  enableCustomWorkflows: true,
  enableAgentCollaboration: true,
  ttsProvider: 'google',
  ttsVolume: 1.0,
  ttsSpeed: 1.0,
  ttsPitch: 1.0,
  pluginSpotifyEnabled: true,
  pluginCalendarEnabled: false,
  pluginGmailEnabled: false,
  pluginNotionEnabled: false,
  pluginDiscordEnabled: false,
  showDesktopWidget: true,
  mcpServers: {} as Record<string, any>,
  hfModel: 'meta-llama/Meta-Llama-3-8B-Instruct',
  localApiBaseUrl: '',
  localModelName: 'local-model',
  ollamaHost: '',
  anthropicApiKey: '',
  openaiApiKey: '',
  openrouterApiKey: '',
  deepseekApiKey: '',
  hfApiKey: '',
  automationBrowser: 'chromium',
  browserDebugUrl: 'http://127.0.0.1:9222/json/list',
  browserExtensionContextUrl: 'http://127.0.0.1:3212/context',
  enableHeadlessTaskQueue: false,
  enableAutoUpdate: false,
  updaterEndpoint: '',
  updaterPublicKey: '',
  telegramBotToken: '',
  enableTelegramBridge: false,
  discordBotToken: '',
  discordApplicationId: '',
  enableDiscordBridge: false,
  slackBotToken: '',
  slackAppToken: '',
  enableSlackBridge: false,
  lineChannelAccessToken: '',
  lineChannelSecret: '',
  enableLineBridge: false,
  whatsappCloudAccessToken: '',
  whatsappPhoneNumberId: '',
  whatsappVerifyToken: '',
  whatsappAppSecret: '',
  enableWhatsappBridge: false
}

type TabType = 'sect-general' | 'sect-audio' | 'sect-automation' | 'sect-theme' | 'sect-plugins' | 'sect-shortcuts' | 'sect-memory'

const GEMINI_MODELS = [
  'gemini-2.5-flash',
  'gemini-2.5-pro',
  'gemini-2.0-flash',
  'gemini-1.5-flash',
  'gemini-1.5-pro',
  'gemini-3.1-flash-lite',
  'gemini-3.1-flash-lite-preview'
]

const OPENAI_MODELS = [
  'gpt-4o',
  'gpt-4o-mini',
  'o1',
  'o3-mini',
  'o1-preview',
  'o1-mini',
  'gpt-4-turbo'
]

const OPENROUTER_MODELS = [
  'openai/gpt-4o-mini',
  'openai/gpt-4o',
  'anthropic/claude-3.5-sonnet',
  'anthropic/claude-3.5-haiku',
  'google/gemini-2.5-flash',
  'meta-llama/llama-3.3-70b-instruct',
  'mistralai/mistral-large'
]

const DEEPSEEK_MODELS = [
  'deepseek-v4-flash',
  'deepseek-v4-pro',
  'deepseek-chat',
  'deepseek-reasoner'
]

const ANTHROPIC_MODELS = [
  'claude-3-7-sonnet-latest',
  'claude-3-5-sonnet-latest',
  'claude-3-5-haiku-latest',
  'claude-3-opus-latest'
]

const HF_MODELS = [
  'meta-llama/Llama-3.3-70B-Instruct',
  'meta-llama/Meta-Llama-3-8B-Instruct',
  'meta-llama/Llama-3.2-3B-Instruct',
  'Qwen/Qwen2.5-72B-Instruct',
  'Qwen/Qwen2.5-Coder-32B-Instruct',
  'mistralai/Mistral-7B-Instruct-v0.3',
  'google/gemma-2-9b-it'
]

const LOCAL_MODELS = [
  'local-model',
  'Qwen/Qwen2.5-7B-Instruct-GGUF',
  'meta-llama/Llama-3.2-3B-Instruct-GGUF',
  'lmstudio-community/gemma-2-9b-it-GGUF'
]

const OLLAMA_MODELS = [
  'llama3:latest',
  'llama3.1:latest',
  'llama3.2:latest',
  'gemma2:latest',
  'mistral:latest',
  'phi3:latest',
  'qwen2.5:latest'
]



export default function SettingsWindow() {
  const [activeTab, setActiveTab] = useState<TabType>('sect-general')
  const [config, setConfig] = useState(DEFAULT_CONFIG)
  
  // Custom user profile / memory state
  const [userName, setUserName] = useState('')
  const [userPreferences, setUserPreferences] = useState('')
  
  // Custom model helpers for all providers
  const [customGemini, setCustomGemini] = useState('')
  const [customOpenAI, setCustomOpenAI] = useState('')
  const [customOpenRouter, setCustomOpenRouter] = useState('')
  const [customDeepSeek, setCustomDeepSeek] = useState('')
  const [customAnthropic, setCustomAnthropic] = useState('')
  const [customHF, setCustomHF] = useState('')
  const [customLocal, setCustomLocal] = useState('')
  const [customOllama, setCustomOllama] = useState('')

  // New MCP Server Form state
  const [mcpName, setMcpName] = useState('')
  const [mcpCmd, setMcpCmd] = useState('')
  const [mcpArgs, setMcpArgs] = useState('')
  const [mcpEnv, setMcpEnv] = useState('')
  const [updateMessage, setUpdateMessage] = useState('')
  const [updateAvailable, setUpdateAvailable] = useState(false)



  // Load settings on mount
  useEffect(() => {
    async function loadSettings() {
      try {
        const nameVal = await getProfileValue('name')
        const prefVal = await getProfileValue('preferences')
        setUserName(nameVal || '')
        setUserPreferences(prefVal || '')
      } catch (e) {
        console.error("Failed to load user profile memory:", e)
      }

      if (window.settingsApi) {
        const loaded = await window.settingsApi.getSettings()
        const merged = { ...DEFAULT_CONFIG, ...loaded }
        setConfig(merged)

        // Set custom text helpers
        if (!GEMINI_MODELS.includes(merged.geminiModel)) {
          setCustomGemini(merged.geminiModel)
        }
        if (!OPENAI_MODELS.includes(merged.openaiModel)) {
          setCustomOpenAI(merged.openaiModel)
        }
        if (!OPENROUTER_MODELS.includes(merged.openrouterModel)) {
          setCustomOpenRouter(merged.openrouterModel)
        }
        if (!DEEPSEEK_MODELS.includes(merged.deepseekModel)) {
          setCustomDeepSeek(merged.deepseekModel)
        }
        if (!ANTHROPIC_MODELS.includes(merged.anthropicModel)) {
          setCustomAnthropic(merged.anthropicModel)
        }
        if (!HF_MODELS.includes(merged.hfModel)) {
          setCustomHF(merged.hfModel)
        }
        if (!LOCAL_MODELS.includes(merged.localModelName)) {
          setCustomLocal(merged.localModelName)
        }
        if (!OLLAMA_MODELS.includes(merged.ollamaModel)) {
          setCustomOllama(merged.ollamaModel)
        }

        applyThemeStyles(merged)
      }
    }
    loadSettings()
  }, [])

  const applyThemeStyles = (cfg: typeof DEFAULT_CONFIG) => {
    document.documentElement.setAttribute('data-theme', cfg.theme)
    document.documentElement.style.setProperty('--accent', cfg.accentColor)
    document.documentElement.style.setProperty('--accent-hover', lightenColor(cfg.accentColor, 20))
    document.documentElement.style.setProperty('--text-main', cfg.systemTextColor)
    document.documentElement.style.setProperty('--glass-blur', cfg.glassBlur)
    document.body.style.fontFamily = cfg.fontFamily
    document.documentElement.style.fontSize = cfg.fontSize

    if (cfg.theme === 'custom') {
      if (cfg.customBgStart && cfg.customBgEnd) {
        const gradient = `linear-gradient(135deg, ${cfg.customBgStart} 0%, ${cfg.customBgEnd} 100%)`
        document.documentElement.style.setProperty('--bg-color', cfg.customBgStart)
        document.documentElement.style.setProperty('--bg-gradient', gradient)
      }
      if (cfg.customPanelBg) {
        const rgb = hexToRgb(cfg.customPanelBg)
        document.documentElement.style.setProperty('--panel-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.75)`)
        document.documentElement.style.setProperty('--panel-raised', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.82)`)
        document.documentElement.style.setProperty('--panel-soft', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.46)`)
        document.documentElement.style.setProperty('--chrome-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.88)`)
        document.documentElement.style.setProperty('--surface-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.62)`)
        document.documentElement.style.setProperty('--surface-strong', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.86)`)
        document.documentElement.style.setProperty('--input-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.72)`)
      }
    } else {
      [
        '--bg-color',
        '--bg-gradient',
        '--panel-bg',
        '--panel-raised',
        '--panel-soft',
        '--chrome-bg',
        '--surface-bg',
        '--surface-strong',
        '--input-bg'
      ].forEach(name => document.documentElement.style.removeProperty(name))
    }
  }

  const lightenColor = (hex: string, amount: number) => {
    const clean = hex.replace('#', '')
    if (clean.length !== 6) return hex
    const num = parseInt(clean, 16)
    const r = Math.min(255, (num >> 16) + amount)
    const g = Math.min(255, ((num >> 8) & 0x00FF) + amount)
    const b = Math.min(255, (num & 0x0000FF) + amount)
    return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`
  }

  const hexToRgb = (hex: string) => {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
    return result ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16)
    } : { r: 15, g: 23, b: 42 }
  }

  const handleSave = async () => {
    const finalConfig = { ...config }
    
    // Process custom models
    if (config.geminiModel === 'custom') {
      finalConfig.geminiModel = customGemini || 'gemini-2.5-flash'
    }
    if (config.openaiModel === 'custom') {
      finalConfig.openaiModel = customOpenAI || 'gpt-4o'
    }
    if (config.openrouterModel === 'custom') {
      finalConfig.openrouterModel = customOpenRouter || 'openai/gpt-4o-mini'
    }
    if (config.deepseekModel === 'custom') {
      finalConfig.deepseekModel = customDeepSeek || 'deepseek-v4-flash'
    }
    if (config.anthropicModel === 'custom') {
      finalConfig.anthropicModel = customAnthropic || 'claude-3-5-sonnet-latest'
    }
    if (config.hfModel === 'custom') {
      finalConfig.hfModel = customHF || 'meta-llama/Meta-Llama-3-8B-Instruct'
    }
    if (config.localModelName === 'custom') {
      finalConfig.localModelName = customLocal || 'local-model'
    }
    if (config.ollamaModel === 'custom') {
      finalConfig.ollamaModel = customOllama || 'llama3:latest'
    }

    try {
      await setProfileValue('name', userName)
      await setProfileValue('preferences', userPreferences)
    } catch (e) {
      console.error("Failed to save user profile memory:", e)
    }

    if (window.settingsApi) {
      await window.settingsApi.saveSettings(finalConfig)
      applyThemeStyles(finalConfig)
      window.settingsApi.closeSettings()
    }
  }

  const handleReset = async () => {
    if (confirm('Reset all settings to default?')) {
      setConfig(DEFAULT_CONFIG)
      setCustomGemini('')
      setCustomOpenAI('')
      setCustomAnthropic('')
      setCustomHF('')
      setCustomLocal('')
      setCustomOllama('')
      setUserName('')
      setUserPreferences('')
      try {
        await setProfileValue('name', '')
        await setProfileValue('preferences', '')
      } catch (e) {
        console.error("Failed to reset profile memory:", e)
      }
      applyThemeStyles(DEFAULT_CONFIG)
    }
  }

  const handleClose = () => {
    window.settingsApi?.closeSettings()
  }

  const handleQuit = () => {
    if (confirm('Are you sure you want to exit Mint?')) {
      window.settingsApi?.quitApp()
    }
  }

  const handleOpenWorkflows = () => {
    window.settingsApi?.openCustomWorkflows()
  }

  const handleReloadWorkflows = async () => {
    if (window.settingsApi) {
      const res = await window.settingsApi.reloadCustomWorkflows()
      alert(res?.success ? 'Workflows reloaded successfully!' : 'Workflow reload failed.')
    }
  }

  const handleAddMcpServer = () => {
    if (!mcpName.trim() || !mcpCmd.trim()) {
      alert('Please provide at least a server name and command.')
      return
    }

    let parsedEnv = {}
    if (mcpEnv.trim()) {
      try {
        parsedEnv = JSON.parse(mcpEnv)
      } catch (e) {
        alert('Invalid JSON in Environment variable field.')
        return
      }
    }

    // Split args nicely or pass as string array
    const argList = mcpArgs.split(/\s+/).filter(Boolean)

    const updatedMcp = {
      ...config.mcpServers,
      [mcpName.trim()]: {
        command: mcpCmd.trim(),
        args: argList,
        env: parsedEnv
      }
    }

    setConfig({
      ...config,
      mcpServers: updatedMcp
    })

    // Reset fields
    setMcpName('')
    setMcpCmd('')
    setMcpArgs('')
    setMcpEnv('')
  }

  const handleRemoveMcpServer = (name: string) => {
    const updated = { ...config.mcpServers }
    delete updated[name]
    setConfig({
      ...config,
      mcpServers: updated
    })
  }

  const handleConnectPlugin = async (plugin: string) => {
    if (plugin === 'discord') {
      try {
        await window.api.executeProactiveAction({ type: 'plugin', pluginName: 'discord', target: '' })
        alert('Discord Rich Presence updated.')
      } catch (reason) {
        alert(String(reason))
      }
      return
    }
    alert(`Configure ${plugin} credentials, then invoke the plugin from Mint chat.`)
  }

  const handleCheckUpdates = async () => {
    try {
      const update = await window.settingsApi.checkForUpdates()
      setUpdateAvailable(Boolean(update.available))
      setUpdateMessage(update.available ? `Mint ${update.version} is available.` : 'Mint is up to date.')
    } catch (reason) {
      setUpdateMessage(String(reason))
    }
  }

  const handleInstallUpdate = async () => {
    if (!confirm('Install the signed Mint update now?')) return
    try {
      setUpdateMessage(await window.settingsApi.installAvailableUpdate())
      setUpdateAvailable(false)
    } catch (reason) {
      setUpdateMessage(String(reason))
    }
  }

  const updateField = (field: keyof typeof DEFAULT_CONFIG, value: any) => {
    const updated = { ...config, [field]: value }
    setConfig(updated)
    applyThemeStyles(updated)
  }

  return (
    <div className="settings-container">
      <header className="settings-header drag-region">
        <div className="header-left">
          <span className="settings-icon" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="3"></circle>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
            </svg>
          </span>
          <div>
            <h1>Settings</h1>
            <p>Configure Mint assistant behavior and integrations.</p>
          </div>
        </div>
        <button className="close-btn" onClick={handleClose} aria-label="Close">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
              stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </header>

      <main className="settings-body">
        <nav className="settings-sidebar" aria-label="Settings sections">
          <button className={`tab-btn ${activeTab === 'sect-general' ? 'active' : ''}`} onClick={() => setActiveTab('sect-general')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="3"></circle>
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
              </svg>
            </span>
            <strong>General</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-memory' ? 'active' : ''}`} onClick={() => setActiveTab('sect-memory')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10z"></path>
                <path d="M12 6a3 3 0 1 0 0 6 3 3 0 0 0 0-6z"></path>
                <path d="M6 18a6 6 0 0 1 12 0"></path>
              </svg>
            </span>
            <strong>Memory & Profile</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-audio' ? 'active' : ''}`} onClick={() => setActiveTab('sect-audio')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon>
                <path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path>
              </svg>
            </span>
            <strong>Audio & Voice</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-automation' ? 'active' : ''}`} onClick={() => setActiveTab('sect-automation')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"></polygon>
              </svg>
            </span>
            <strong>Automation</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-theme' ? 'active' : ''}`} onClick={() => setActiveTab('sect-theme')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="5"></circle>
                <line x1="12" y1="1" x2="12" y2="3"></line>
                <line x1="12" y1="21" x2="12" y2="23"></line>
                <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line>
                <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line>
                <line x1="1" y1="12" x2="3" y2="12"></line>
                <line x1="21" y1="12" x2="23" y2="12"></line>
                <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line>
                <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line>
              </svg>
            </span>
            <strong>Theme & UI</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-plugins' ? 'active' : ''}`} onClick={() => setActiveTab('sect-plugins')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"></path>
              </svg>
            </span>
            <strong>Plugins</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-shortcuts' ? 'active' : ''}`} onClick={() => setActiveTab('sect-shortcuts')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="4" width="20" height="16" rx="2" ry="2"></rect>
                <line x1="6" y1="8" x2="6" y2="8"></line>
                <line x1="10" y1="8" x2="10" y2="8"></line>
                <line x1="14" y1="8" x2="14" y2="8"></line>
                <line x1="18" y1="8" x2="18" y2="8"></line>
                <line x1="6" y1="12" x2="6" y2="12"></line>
                <line x1="10" y1="12" x2="10" y2="12"></line>
                <line x1="14" y1="12" x2="14" y2="12"></line>
                <line x1="18" y1="12" x2="18" y2="12"></line>
                <line x1="7" y1="16" x2="17" y2="16"></line>
              </svg>
            </span>
            <strong>Shortcuts</strong>
          </button>
        </nav>

        <div className="settings-content">
          {/* GENERAL SECTION */}
          {activeTab === 'sect-general' && (
            <div className="tab-pane active">
              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">AI routing</p>
                    <h2 class="section-title">Provider & Model</h2>
                  </div>
                  <p className="section-description">Choose which AI backend Mint uses, then set the default model.</p>
                </div>

                <div className="form-grid compact">
                  <div className="setting-row wide">
                    <label>Active Provider</label>
                    <select value={config.aiProvider} onChange={(e) => updateField('aiProvider', e.target.value)}>
                      <option value="gemini">Google Gemini (Cloud)</option>
                      <option value="anthropic">Anthropic Claude</option>
                      <option value="openai">OpenAI</option>
                      <option value="openrouter">OpenRouter</option>
                      <option value="deepseek">DeepSeek</option>
                      <option value="ollama">Ollama (Local / Private)</option>
                      <option value="huggingface">Hugging Face (Inference API)</option>
                      <option value="local_openai">Local (LM Studio / OpenAI Compatible)</option>
                    </select>
                  </div>

                      <div className="setting-row">
                        <label>Gemini Model</label>
                        <select 
                          value={GEMINI_MODELS.includes(config.geminiModel) ? config.geminiModel : 'custom'} 
                          onChange={(e) => updateField('geminiModel', e.target.value)}
                        >
                          {GEMINI_MODELS.map(model => (
                            <option key={model} value={model}>{model}</option>
                          ))}
                          <option value="custom">Custom...</option>
                        </select>
                      </div>
                      {(!GEMINI_MODELS.includes(config.geminiModel) || config.geminiModel === 'custom') && (
                        <div className="setting-row">
                          <label>Custom Gemini Model</label>
                          <input 
                            type="text" 
                            value={customGemini} 
                            onChange={(e) => { setCustomGemini(e.target.value); updateField('geminiModel', 'custom') }} 
                            placeholder="e.g. gemini-3.1-flash-lite-preview" 
                          />
                        </div>
                      )}

                      <div className="setting-row">
                        <label>OpenAI Model</label>
                        <select 
                          value={OPENAI_MODELS.includes(config.openaiModel) ? config.openaiModel : 'custom'} 
                          onChange={(e) => updateField('openaiModel', e.target.value)}
                        >
                          {OPENAI_MODELS.map(model => (
                            <option key={model} value={model}>{model}</option>
                          ))}
                          <option value="custom">Custom...</option>
                        </select>
                      </div>
                      {(!OPENAI_MODELS.includes(config.openaiModel) || config.openaiModel === 'custom') && (
                        <div className="setting-row">
                          <label>Custom OpenAI Model</label>
                          <input 
                            type="text" 
                            value={customOpenAI} 
                            onChange={(e) => { setCustomOpenAI(e.target.value); updateField('openaiModel', 'custom') }} 
                            placeholder="e.g. gpt-4o" 
                          />
                        </div>
                      )}

                      <div className="setting-row">
                        <label>OpenRouter Model</label>
                        <select
                          value={OPENROUTER_MODELS.includes(config.openrouterModel) ? config.openrouterModel : 'custom'}
                          onChange={(e) => updateField('openrouterModel', e.target.value)}
                        >
                          {OPENROUTER_MODELS.map(model => (
                            <option key={model} value={model}>{model}</option>
                          ))}
                          <option value="custom">Custom...</option>
                        </select>
                      </div>
                      {(!OPENROUTER_MODELS.includes(config.openrouterModel) || config.openrouterModel === 'custom') && (
                        <div className="setting-row">
                          <label>Custom OpenRouter Model</label>
                          <input
                            type="text"
                            value={customOpenRouter}
                            onChange={(e) => { setCustomOpenRouter(e.target.value); updateField('openrouterModel', 'custom') }}
                            placeholder="e.g. anthropic/claude-3.5-sonnet"
                          />
                        </div>
                      )}

                      <div className="setting-row">
                        <label>DeepSeek Model</label>
                        <select
                          value={DEEPSEEK_MODELS.includes(config.deepseekModel) ? config.deepseekModel : 'custom'}
                          onChange={(e) => updateField('deepseekModel', e.target.value)}
                        >
                          {DEEPSEEK_MODELS.map(model => (
                            <option key={model} value={model}>{model}</option>
                          ))}
                          <option value="custom">Custom...</option>
                        </select>
                      </div>
                      {(!DEEPSEEK_MODELS.includes(config.deepseekModel) || config.deepseekModel === 'custom') && (
                        <div className="setting-row">
                          <label>Custom DeepSeek Model</label>
                          <input
                            type="text"
                            value={customDeepSeek}
                            onChange={(e) => { setCustomDeepSeek(e.target.value); updateField('deepseekModel', 'custom') }}
                            placeholder="e.g. deepseek-v4-pro"
                          />
                        </div>
                      )}

                      <div className="setting-row">
                        <label>Anthropic Model</label>
                        <select 
                          value={ANTHROPIC_MODELS.includes(config.anthropicModel) ? config.anthropicModel : 'custom'} 
                          onChange={(e) => updateField('anthropicModel', e.target.value)}
                        >
                          {ANTHROPIC_MODELS.map(model => (
                            <option key={model} value={model}>{model}</option>
                          ))}
                          <option value="custom">Custom...</option>
                        </select>
                      </div>
                      {(!ANTHROPIC_MODELS.includes(config.anthropicModel) || config.anthropicModel === 'custom') && (
                        <div className="setting-row">
                          <label>Custom Anthropic Model</label>
                          <input 
                            type="text" 
                            value={customAnthropic} 
                            onChange={(e) => { setCustomAnthropic(e.target.value); updateField('anthropicModel', 'custom') }} 
                            placeholder="e.g. claude-3-5-sonnet-latest" 
                          />
                        </div>
                      )}

                      <div className="setting-row">
                        <label>Hugging Face Model</label>
                        <select 
                          value={HF_MODELS.includes(config.hfModel) ? config.hfModel : 'custom'} 
                          onChange={(e) => updateField('hfModel', e.target.value)}
                        >
                          {HF_MODELS.map(model => (
                            <option key={model} value={model}>{model}</option>
                          ))}
                          <option value="custom">Custom...</option>
                        </select>
                      </div>
                      {(!HF_MODELS.includes(config.hfModel) || config.hfModel === 'custom') && (
                        <div className="setting-row">
                          <label>Custom Hugging Face Model</label>
                          <input 
                            type="text" 
                            value={customHF} 
                            onChange={(e) => { setCustomHF(e.target.value); updateField('hfModel', 'custom') }} 
                            placeholder="e.g. meta-llama/Meta-Llama-3-8B-Instruct" 
                          />
                        </div>
                      )}

                      <div className="setting-row">
                        <label>LM Studio Model</label>
                        <select 
                          value={LOCAL_MODELS.includes(config.localModelName) ? config.localModelName : 'custom'} 
                          onChange={(e) => updateField('localModelName', e.target.value)}
                        >
                          {LOCAL_MODELS.map(model => (
                            <option key={model} value={model}>{model}</option>
                          ))}
                          <option value="custom">Custom...</option>
                        </select>
                      </div>
                      {(!LOCAL_MODELS.includes(config.localModelName) || config.localModelName === 'custom') && (
                        <div className="setting-row">
                          <label>Custom LM Studio Model</label>
                          <input 
                            type="text" 
                            value={customLocal} 
                            onChange={(e) => { setCustomLocal(e.target.value); updateField('localModelName', 'custom') }} 
                            placeholder="e.g. local-model" 
                          />
                        </div>
                      )}

                      <div className="setting-row">
                        <label>Ollama Model</label>
                        <select 
                          value={OLLAMA_MODELS.includes(config.ollamaModel) ? config.ollamaModel : 'custom'} 
                          onChange={(e) => updateField('ollamaModel', e.target.value)}
                        >
                          {OLLAMA_MODELS.map(model => (
                            <option key={model} value={model}>{model}</option>
                          ))}
                          <option value="custom">Custom...</option>
                        </select>
                      </div>
                      {(!OLLAMA_MODELS.includes(config.ollamaModel) || config.ollamaModel === 'custom') && (
                        <div className="setting-row">
                          <label>Custom Ollama Model</label>
                          <input 
                            type="text" 
                            value={customOllama} 
                            onChange={(e) => { setCustomOllama(e.target.value); updateField('ollamaModel', 'custom') }} 
                            placeholder="e.g. llama3:latest" 
                          />
                        </div>
                      )}
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Desktop updates</p>
                    <h2 className="section-title">Signed Tauri Channel</h2>
                  </div>
                  <p className="section-description">Check and explicitly install signed Tauri releases from your configured update channel.</p>
                </div>
                <div className="toggle-row">
                  <div>
                    <label>Check signed update channel</label>
                    <p className="hint">This flag does not install updates automatically.</p>
                  </div>
                  <label className="toggle-switch">
                    <input
                      type="checkbox"
                      checked={config.enableAutoUpdate}
                      onChange={(e) => updateField('enableAutoUpdate', e.target.checked)}
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>
                <div className="form-grid single">
                  <div className="setting-row">
                    <label>Updater Endpoint</label>
                    <input type="text" value={config.updaterEndpoint} onChange={(e) => updateField('updaterEndpoint', e.target.value)} placeholder="https://updates.example.com/latest.json" />
                  </div>
                  <div className="setting-row">
                    <label>Updater Public Key</label>
                    <textarea value={config.updaterPublicKey} onChange={(e) => updateField('updaterPublicKey', e.target.value)} placeholder="Minisign public key" />
                  </div>
                </div>
                <div className="setting-actions">
                  <button type="button" className="btn-connect" onClick={handleCheckUpdates}>Check for updates</button>
                  {updateAvailable && <button type="button" className="btn-primary" onClick={handleInstallUpdate}>Install signed update</button>}
                </div>
                {updateMessage && <p className="hint">{updateMessage}</p>}
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Credentials</p>
                    <h2 class="section-title">API Keys & Hosts</h2>
                  </div>
                </div>

                <div className="form-grid">
                  <div className="setting-row">
                    <label>Gemini API Key</label>
                    <input 
                      type="password" 
                      value={config.apiKey} 
                      onChange={(e) => updateField('apiKey', e.target.value)} 
                      placeholder="Enter Gemini API Key..." 
                    />
                  </div>
                  <div className="setting-row">
                    <label>OpenAI API Key</label>
                    <input 
                      type="password" 
                      value={config.openaiApiKey} 
                      onChange={(e) => updateField('openaiApiKey', e.target.value)} 
                      placeholder="Enter OpenAI API Key..." 
                    />
                  </div>
                  <div className="setting-row">
                    <label>OpenRouter API Key</label>
                    <input
                      type="password"
                      value={config.openrouterApiKey}
                      onChange={(e) => updateField('openrouterApiKey', e.target.value)}
                      placeholder="Enter OpenRouter API Key..."
                    />
                  </div>
                  <div className="setting-row">
                    <label>DeepSeek API Key</label>
                    <input
                      type="password"
                      value={config.deepseekApiKey}
                      onChange={(e) => updateField('deepseekApiKey', e.target.value)}
                      placeholder="Enter DeepSeek API Key..."
                    />
                  </div>
                  <div className="setting-row">
                    <label>Anthropic API Key</label>
                    <input 
                      type="password" 
                      value={config.anthropicApiKey} 
                      onChange={(e) => updateField('anthropicApiKey', e.target.value)} 
                      placeholder="Enter Anthropic API Key..." 
                    />
                  </div>
                  <div className="setting-row">
                    <label>Hugging Face API Key</label>
                    <input 
                      type="password" 
                      value={config.hfApiKey} 
                      onChange={(e) => updateField('hfApiKey', e.target.value)} 
                      placeholder="Enter Hugging Face API Key..." 
                    />
                  </div>
                  <div className="setting-row">
                    <label>LM Studio Base URL</label>
                    <input 
                      type="text" 
                      value={config.localApiBaseUrl} 
                      onChange={(e) => updateField('localApiBaseUrl', e.target.value)} 
                      placeholder="e.g. http://localhost:1234/v1" 
                    />
                  </div>
                  <div className="setting-row">
                    <label>Ollama Host</label>
                    <input 
                      type="text" 
                      value={config.ollamaHost} 
                      onChange={(e) => updateField('ollamaHost', e.target.value)} 
                      placeholder="e.g. http://localhost:11434" 
                    />
                  </div>
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Desktop</p>
                    <h2 class="section-title">Assistant Presence</h2>
                  </div>
                </div>
                <div className="toggle-row">
                  <div>
                    <label>Show Desktop AI Candidate</label>
                    <p className="hint">Show the mini AI character on your desktop.</p>
                  </div>
                  <label className="toggle-switch">
                    <input 
                      type="checkbox" 
                      checked={config.showDesktopWidget} 
                      onChange={(e) => updateField('showDesktopWidget', e.target.checked)} 
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>
              </section>
            </div>
          )}

          {/* MEMORY SECTION */}
          {activeTab === 'sect-memory' && (
            <div className="tab-pane active animate-fade-in">
              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Cross-session Memory</p>
                    <h2 className="section-title">User Profile & Preferences</h2>
                  </div>
                  <p className="section-description">
                    Information and preferences stored here will be remembered across all conversations. Mint automatically learns and updates your profile details from your chats in the background, but you can also edit them manually.
                  </p>
                </div>

                <div className="form-grid single">
                  <div className="setting-row">
                    <label>Your Name / Nickname</label>
                    <div className="memory-field-container">
                      <input 
                        type="text" 
                        value={userName} 
                        onChange={(e) => setUserName(e.target.value)} 
                        placeholder="e.g. Pheem, Jane" 
                      />
                      <p className="hint">Used by the assistant to address you in conversation.</p>
                    </div>
                  </div>

                  <div className="setting-row">
                    <label>Custom Instructions & Preferences</label>
                    <div className="memory-field-container">
                      <textarea 
                        value={userPreferences} 
                        onChange={(e) => setUserPreferences(e.target.value)} 
                        placeholder="e.g. Explain coding concepts step-by-step. Prefer TypeScript. Talk in Thai. Keep explanations concise." 
                        style={{ minHeight: '150px', resize: 'vertical' }}
                      />
                      <p className="hint">Preferences, guidelines, or persona instructions for the assistant.</p>
                    </div>
                  </div>
                </div>
              </section>
            </div>
          )}

          {/* AUDIO SECTION */}
          {activeTab === 'sect-audio' && (
            <div className="tab-pane active">
              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Speech</p>
                    <h2 class="section-title">Voice Reply</h2>
                  </div>
                  <p className="section-description">Control spoken responses and TTS behavior.</p>
                </div>

                <div className="toggle-row">
                  <div>
                    <label>Enable Voice Reply</label>
                    <p className="hint">Mint will speak responses out loud when this is enabled.</p>
                  </div>
                  <label className="toggle-switch">
                    <input 
                      type="checkbox" 
                      checked={config.enableVoiceReply} 
                      onChange={(e) => updateField('enableVoiceReply', e.target.checked)} 
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>

                <div className="form-grid single">
                  <div className="setting-row">
                    <label>Voice Engine</label>
                    <select value={config.ttsProvider} onChange={(e) => updateField('ttsProvider', e.target.value)}>
                      <option value="google">Google Cloud (Natural, Auto Lang)</option>
                      <option value="native">OS Native (Supports Pitch)</option>
                    </select>
                  </div>
                </div>

                <div className="slider-stack">
                  <div className="setting-row">
                    <label>Volume</label>
                    <div className="slider-group">
                      <input 
                        type="range" 
                        min="0" 
                        max="1" 
                        step="0.1" 
                        value={config.ttsVolume} 
                        onChange={(e) => updateField('ttsVolume', parseFloat(e.target.value))} 
                        className="range-slider" 
                      />
                      <span className="range-value">{Math.round(config.ttsVolume * 100)}%</span>
                    </div>
                  </div>
                  <div className="setting-row">
                    <label>Speed</label>
                    <div className="slider-group">
                      <input 
                        type="range" 
                        min="0.5" 
                        max="2" 
                        step="0.1" 
                        value={config.ttsSpeed} 
                        onChange={(e) => updateField('ttsSpeed', parseFloat(e.target.value))} 
                        className="range-slider" 
                      />
                      <span className="range-value">{parseFloat(String(config.ttsSpeed)).toFixed(1)}x</span>
                    </div>
                  </div>
                  <div className="setting-row">
                    <label>Pitch</label>
                    <div className="slider-group">
                      <input 
                        type="range" 
                        min="0" 
                        max="2" 
                        step="0.1" 
                        value={config.ttsPitch} 
                        onChange={(e) => updateField('ttsPitch', parseFloat(e.target.value))} 
                        className="range-slider" 
                      />
                      <span className="range-value">{parseFloat(String(config.ttsPitch)).toFixed(1)}</span>
                    </div>
                    <p className="hint">Pitch applies to OS native voice only.</p>
                  </div>
                </div>
              </section>
            </div>
          )}

          {/* AUTOMATION SECTION */}
          {activeTab === 'sect-automation' && (
            <div className="tab-pane active">
              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Browser</p>
                    <h2 class="section-title">Automation Engine</h2>
                  </div>
                </div>
                <div className="form-grid single">
                  <div className="setting-row">
                    <label>Browser Engine</label>
                    <select value={config.automationBrowser} onChange={(e) => updateField('automationBrowser', e.target.value)}>
                      <option value="chromium">Chromium (Bundled)</option>
                      <option value="/usr/bin/firefox">Firefox (System - Linux)</option>
                    </select>
                  </div>
                  <div className="setting-row">
                    <label>Chromium DevTools Endpoint</label>
                    <input type="text" value={config.browserDebugUrl} onChange={(e) => updateField('browserDebugUrl', e.target.value)} />
                    <p className="hint">Required for native tab reading and selector clicks.</p>
                  </div>
                  <div className="setting-row">
                    <label>Browser Extension Context Endpoint</label>
                    <input type="text" value={config.browserExtensionContextUrl} onChange={(e) => updateField('browserExtensionContextUrl', e.target.value)} />
                    <p className="hint">Fallback endpoint used when Chromium remote debugging is unavailable.</p>
                  </div>
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Background tasks</p>
                    <h2 className="section-title">Native Headless Queue</h2>
                  </div>
                </div>
                <div className="toggle-row">
                  <div>
                    <label>Process queued tasks automatically</label>
                    <p className="hint">Allow the bounded Rust worker to process pending tasks every 15 seconds.</p>
                  </div>
                  <label className="toggle-switch">
                    <input
                      type="checkbox"
                      checked={config.enableHeadlessTaskQueue}
                      onChange={(e) => updateField('enableHeadlessTaskQueue', e.target.checked)}
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Awareness</p>
                    <h2 class="section-title">Proactive Assistant</h2>
                  </div>
                  <p className="section-description">Tune screen analysis frequency and suggestion timing.</p>
                </div>
                <div className="slider-stack">
                  <div className="setting-row">
                    <label>Screen Capture Frequency</label>
                    <div className="slider-group">
                      <input 
                        type="range" 
                        min="30" 
                        max="300" 
                        step="30" 
                        value={config.proactiveInterval} 
                        onChange={(e) => updateField('proactiveInterval', parseInt(e.target.value))} 
                        className="range-slider" 
                      />
                      <span className="range-value">{config.proactiveInterval} sec</span>
                    </div>
                    <p className="hint">Lower values respond faster but use more API calls.</p>
                  </div>
                  <div className="setting-row">
                    <label>Suggestion Cooldown</label>
                    <div className="slider-group">
                      <input 
                        type="range" 
                        min="60" 
                        max="600" 
                        step="60" 
                        value={config.proactiveCooldown} 
                        onChange={(e) => updateField('proactiveCooldown', parseInt(e.target.value))} 
                        className="range-slider" 
                      />
                      <span className="range-value">{Math.round(config.proactiveCooldown / 60)} min</span>
                    </div>
                    <p className="hint">Minimum time between repeat suggestions.</p>
                  </div>
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Messaging</p>
                    <h2 className="section-title">Native Channel Bridges</h2>
                  </div>
                  <p className="section-description">Configure credentials used directly by the Rust channel workers.</p>
                </div>
                <div className="form-grid">
                  <label className="toggle-row"><span>Telegram Bot API</span><input type="checkbox" checked={config.enableTelegramBridge} onChange={(e) => updateField('enableTelegramBridge', e.target.checked)} /></label>
                  <input type="password" placeholder="Telegram bot token" value={config.telegramBotToken} onChange={(e) => updateField('telegramBotToken', e.target.value)} />
                  <label className="toggle-row"><span>Discord Gateway</span><input type="checkbox" checked={config.enableDiscordBridge} onChange={(e) => updateField('enableDiscordBridge', e.target.checked)} /></label>
                  <input type="password" placeholder="Discord bot token" value={config.discordBotToken} onChange={(e) => updateField('discordBotToken', e.target.value)} />
                  <input type="text" placeholder="Discord application ID for Rich Presence" value={config.discordApplicationId} onChange={(e) => updateField('discordApplicationId', e.target.value)} />
                  <label className="toggle-row"><span>Slack Socket Mode</span><input type="checkbox" checked={config.enableSlackBridge} onChange={(e) => updateField('enableSlackBridge', e.target.checked)} /></label>
                  <input type="password" placeholder="Slack bot token (xoxb-...)" value={config.slackBotToken} onChange={(e) => updateField('slackBotToken', e.target.value)} />
                  <input type="password" placeholder="Slack app token (xapp-...)" value={config.slackAppToken} onChange={(e) => updateField('slackAppToken', e.target.value)} />
                  <label className="toggle-row"><span>LINE Webhook</span><input type="checkbox" checked={config.enableLineBridge} onChange={(e) => updateField('enableLineBridge', e.target.checked)} /></label>
                  <input type="password" placeholder="LINE channel access token" value={config.lineChannelAccessToken} onChange={(e) => updateField('lineChannelAccessToken', e.target.value)} />
                  <input type="password" placeholder="LINE channel secret" value={config.lineChannelSecret} onChange={(e) => updateField('lineChannelSecret', e.target.value)} />
                  <label className="toggle-row"><span>WhatsApp Cloud API</span><input type="checkbox" checked={config.enableWhatsappBridge} onChange={(e) => updateField('enableWhatsappBridge', e.target.checked)} /></label>
                  <input type="password" placeholder="WhatsApp Cloud access token" value={config.whatsappCloudAccessToken} onChange={(e) => updateField('whatsappCloudAccessToken', e.target.value)} />
                  <input type="text" placeholder="WhatsApp phone number ID" value={config.whatsappPhoneNumberId} onChange={(e) => updateField('whatsappPhoneNumberId', e.target.value)} />
                  <input type="password" placeholder="WhatsApp verify token" value={config.whatsappVerifyToken} onChange={(e) => updateField('whatsappVerifyToken', e.target.value)} />
                  <input type="password" placeholder="WhatsApp app secret (optional HMAC validation)" value={config.whatsappAppSecret} onChange={(e) => updateField('whatsappAppSecret', e.target.value)} />
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Rules</p>
                    <h2 class="section-title">Custom Workflows</h2>
                  </div>
                </div>
                <div className="toggle-row">
                  <div>
                    <label>Enable Custom Workflows</label>
                    <p className="hint">Run "If This Then Mint" rules from the workflow JSON file.</p>
                  </div>
                  <label className="toggle-switch">
                    <input 
                      type="checkbox" 
                      checked={config.enableCustomWorkflows} 
                      onChange={(e) => updateField('enableCustomWorkflows', e.target.checked)} 
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>
                <div className="button-row" style={{ display: 'flex', gap: '10px', marginTop: '10px' }}>
                  <button className="btn btn-secondary" onClick={handleOpenWorkflows}>Open workflows.json</button>
                  <button className="btn btn-primary" onClick={handleReloadWorkflows}>Reload Rules</button>
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Code mode</p>
                    <h2 class="section-title">Agent Collaboration</h2>
                  </div>
                </div>
                <div className="toggle-row">
                  <div>
                    <label>Enable Multi-Agent Review</label>
                    <p className="hint">Allow a secondary model to review code written by the primary model.</p>
                  </div>
                  <label className="toggle-switch">
                    <input 
                      type="checkbox" 
                      checked={config.enableAgentCollaboration} 
                      onChange={(e) => updateField('enableAgentCollaboration', e.target.checked)} 
                    />
                    <span className="toggle-slider"></span>
                  </label>
                </div>
              </section>
            </div>
          )}

          {/* THEME & UI SECTION */}
          {activeTab === 'sect-theme' && (
            <div className="tab-pane active">
              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Appearance</p>
                    <h2 class="section-title">Theme</h2>
                  </div>
                </div>
                <div className="theme-grid" style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '12px' }}>
                  {(['dark', 'light', 'midnight', 'custom'] as const).map(t => (
                    <button 
                      key={t}
                      className={`theme-card ${config.theme === t ? 'active' : ''}`} 
                      onClick={() => updateField('theme', t)}
                    >
                      <div className={`theme-preview ${t}-preview`}></div>
                      <span style={{ textTransform: 'capitalize' }}>{t}</span>
                    </button>
                  ))}
                </div>

                {config.theme === 'custom' && (
                  <div className="custom-theme-panel" style={{ marginTop: '15px' }}>
                    <div className="setting-row">
                      <label>Background Gradient</label>
                      <div className="color-range" style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                        <input type="color" value={config.customBgStart} onChange={(e) => updateField('customBgStart', e.target.value)} />
                        <span style={{ display: 'inline-flex', alignItems: 'center', color: 'var(--text-soft)' }}>
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                            <line x1="5" y1="12" x2="19" y2="12"></line>
                            <polyline points="12 5 19 12 12 19"></polyline>
                          </svg>
                        </span>
                        <input type="color" value={config.customBgEnd} onChange={(e) => updateField('customBgEnd', e.target.value)} />
                      </div>
                    </div>
                    <div className="setting-row">
                      <label>Panel Background</label>
                      <input type="color" value={config.customPanelBg} onChange={(e) => updateField('customPanelBg', e.target.value)} />
                    </div>
                  </div>
                )}
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Color</p>
                    <h2 class="section-title">Accent & Text</h2>
                  </div>
                </div>
                <div className="color-section">
                  <div>
                    <label>Accent Color</label>
                    <div className="color-presets">
                      {['#8b5cf6', '#06b6d4', '#10b981', '#f59e0b', '#ef4444', '#ec4899'].map(c => (
                        <button 
                          key={c}
                          className="color-dot" 
                          style={{ backgroundColor: c, border: config.accentColor === c ? '2px solid white' : 'none' }} 
                          onClick={() => updateField('accentColor', c)}
                        />
                      ))}
                    </div>
                  </div>
                  <div className="color-inputs" style={{ display: 'flex', gap: '15px', marginTop: '12px' }}>
                    <div>
                      <label>Custom Accent</label>
                      <input type="color" value={config.accentColor} onChange={(e) => updateField('accentColor', e.target.value)} style={{ display: 'block', marginTop: '4px' }} />
                    </div>
                    <div>
                      <label>System Text</label>
                      <input type="color" value={config.systemTextColor} onChange={(e) => updateField('systemTextColor', e.target.value)} style={{ display: 'block', marginTop: '4px' }} />
                    </div>
                  </div>
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Surface</p>
                    <h2 class="section-title">Interface Style</h2>
                  </div>
                </div>
                <div className="form-grid">
                  <div className="setting-row">
                    <label>Glass Blur</label>
                    <select value={config.glassBlur} onChange={(e) => updateField('glassBlur', e.target.value)}>
                      <option value="blur(4px)">Low (4px)</option>
                      <option value="blur(16px)">Medium (16px) - Default</option>
                      <option value="blur(32px)">High (32px)</option>
                      <option value="none">Off (Solid)</option>
                    </select>
                  </div>
                  <div className="setting-row">
                    <label>Font Family</label>
                    <select value={config.fontFamily} onChange={(e) => updateField('fontFamily', e.target.value)}>
                      <option value="'Outfit', sans-serif">Outfit (Default)</option>
                      <option value="'Mali', cursive">Mali (Cute Thai Font)</option>
                      <option value="'Prompt', sans-serif">Prompt (Modern Thai)</option>
                      <option value="'Sarabun', sans-serif">Sarabun (Formal Thai)</option>
                    </select>
                  </div>
                  <div className="setting-row">
                    <label>Font Size</label>
                    <select value={config.fontSize} onChange={(e) => updateField('fontSize', e.target.value)}>
                      <option value="16px">Small</option>
                      <option value="18px">Medium (Default)</option>
                      <option value="22px">Large</option>
                      <option value="26px">Extra Large</option>
                      <option value="30px">Extra Extra Large</option>
                    </select>
                  </div>
                </div>
              </section>
            </div>
          )}

          {/* PLUGINS SECTION */}
          {activeTab === 'sect-plugins' && (
            <div className="tab-pane active">
              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Integrations</p>
                    <h2 class="section-title">Built-in Plugins</h2>
                  </div>
                </div>
                <div className="plugin-list">
                  {[
                    { key: 'spotify', name: 'Spotify', desc: 'Control playback with AI. Requires playerctl.', icon: '🎵' },
                    { key: 'calendar', name: 'Google Calendar', desc: 'Read and schedule events.', icon: '📅' },
                    { key: 'gmail', name: 'Gmail', desc: 'Read email and create drafts safely.', icon: '✉' },
                    { key: 'notion', name: 'Notion', desc: 'Create notes, pages, and read databases.', icon: '📝' },
                    { key: 'discord', name: 'Discord RPC', desc: 'Show "Using Mint Assistant" on Discord status.', icon: '💬' },
                  ].map(p => (
                    <div className="plugin-card" key={p.key}>
                      <div className="plugin-icon">{p.icon}</div>
                      <div className="plugin-info">
                        <div className="plugin-name">{p.name}</div>
                        <div className="plugin-desc">{p.desc}</div>
                      </div>
                      <div className="plugin-actions">
                        <button className="btn-connect" onClick={() => handleConnectPlugin(p.key)}>Connect</button>
                      </div>
                    </div>
                  ))}
                </div>
              </section>

              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">External tools</p>
                    <h2 class="section-title">MCP Servers</h2>
                  </div>
                  <p className="section-description">Connect Mint to tools like search, GitHub, or filesystem servers.</p>
                </div>

                <div className="mcp-list">
                  {Object.entries(config.mcpServers || {}).map(([name, srv]: [string, any]) => (
                    <div className="plugin-card" key={name} style={{ marginBottom: '10px' }}>
                      <div className="plugin-icon">⚙</div>
                      <div className="plugin-info">
                        <div className="plugin-name">{name}</div>
                        <div className="plugin-desc" style={{ fontFamily: 'monospace', fontSize: '0.8rem' }}>
                          Command: {srv.command} {srv.args?.join(' ')}
                        </div>
                      </div>
                      <div className="plugin-actions">
                        <button className="btn btn-danger" onClick={() => handleRemoveMcpServer(name)}>Remove</button>
                      </div>
                    </div>
                  ))}
                </div>

                <div className="add-mcp-box">
                  <h3>Add MCP Server</h3>
                  <div className="form-grid">
                    <input type="text" placeholder="Server Name (e.g. google-search)" value={mcpName} onChange={(e) => setMcpName(e.target.value)} />
                    <input type="text" placeholder="Command (e.g. npx)" value={mcpCmd} onChange={(e) => setMcpCmd(e.target.value)} />
                  </div>
                  <input type="text" placeholder="Arguments (e.g. -y @modelcontextprotocol/server-brave-search)" value={mcpArgs} onChange={(e) => setMcpArgs(e.target.value)} style={{ width: '100%', marginTop: '8px' }} />
                  <textarea placeholder='Env JSON, e.g. {"BRAVE_API_KEY": "..."}' value={mcpEnv} onChange={(e) => setMcpEnv(e.target.value)} style={{ width: '100%', marginTop: '8px', height: '60px' }}></textarea>
                  <button className="btn-primary full-width" onClick={handleAddMcpServer} style={{ marginTop: '10px', width: '100%' }}>Add MCP Server</button>
                </div>
              </section>
            </div>
          )}

          {/* KEYBOARD SHORTCUTS SECTION */}
          {activeTab === 'sect-shortcuts' && (
            <div className="tab-pane active">
              <section className="setting-section">
                <div className="section-heading">
                  <div>
                    <p className="section-kicker">Keyboard</p>
                    <h2 class="section-title">Shortcuts</h2>
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
