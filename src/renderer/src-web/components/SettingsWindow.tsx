import React, { useState, useEffect } from 'react'
import { getLocalApiBase, isTauriRuntime, getProfileValue, setProfileValue, setActiveModel, authUpdateProfile } from '../tauri'
import { useAuthUser } from '../../shared/components/AuthGate'
import GeneralTab from './Settings/GeneralTab'
import ProfileTab from './Settings/ProfileTab'
import MemoryTab from './Settings/MemoryTab'
import AudioTab from './Settings/AudioTab'
import AutomationTab from './Settings/AutomationTab'
import ThemeTab from './Settings/ThemeTab'
import PluginsTab from './Settings/PluginsTab'
import AgentsTab from './Settings/AgentsTab'
import {
  GEMINI_MODELS,
  OPENAI_MODELS,
  OPENROUTER_MODELS,
  DEEPSEEK_MODELS,
  ANTHROPIC_MODELS,
  HF_MODELS,
  LOCAL_MODELS,
  OLLAMA_MODELS,
} from '../../shared/constants/models'

// ─── Custom Provider Types ────────────────────────────────────────────────────

export interface CustomProviderModel {
  modelId: string
  displayName: string
}

export interface CustomProviderHeader {
  name: string
  value: string
}

export interface CustomProviderConfig {
  id: string
  displayName: string
  baseUrl: string
  apiKey: string
  models: CustomProviderModel[]
  headers: CustomProviderHeader[]
}

import { DEFAULT_CONFIG } from '../../shared/constants/config'
export { DEFAULT_CONFIG }

type TabType = 'sect-general' | 'sect-profile' | 'sect-audio' | 'sect-automation' | 'sect-theme' | 'sect-plugins' | 'sect-shortcuts' | 'sect-memory' | 'sect-agents'

export {
  GEMINI_MODELS,
  OPENAI_MODELS,
  OPENROUTER_MODELS,
  DEEPSEEK_MODELS,
  ANTHROPIC_MODELS,
  HF_MODELS,
  LOCAL_MODELS,
  OLLAMA_MODELS,
} from '../../shared/constants/models'

export default function SettingsWindow() {
  const [activeTab, setActiveTab] = useState<TabType>('sect-general')
  const [config, setConfig] = useState(DEFAULT_CONFIG)
  
  // Custom user profile / memory state
  const [userName, setUserName] = useState('')
  const [userPreferences, setUserPreferences] = useState('')

  // Shared Mint account profile (name + avatar), saved together with the
  // rest of settings via the "Save Settings" button.
  const { user: authUser, refreshUser } = useAuthUser()
  const [profileName, setProfileName] = useState('')
  const [profileImageUrl, setProfileImageUrl] = useState('')

  useEffect(() => {
    setProfileName(authUser?.name || '')
    setProfileImageUrl(authUser?.image || '')
  }, [authUser])

  // Custom model helpers for all providers
  const [customGemini, setCustomGemini] = useState('')
  const [customOpenAI, setCustomOpenAI] = useState('')
  const [customOpenRouter, setCustomOpenRouter] = useState('')
  const [customDeepSeek, setCustomDeepSeek] = useState('')
  const [customAnthropic, setCustomAnthropic] = useState('')
  const [customHF, setCustomHF] = useState('')
  const [customLocal, setCustomLocal] = useState('')
  const [customOllama, setCustomOllama] = useState('')
  const [dynamicOllamaModels, setDynamicOllamaModels] = useState<string[]>(OLLAMA_MODELS)

  // New MCP Server Form state
  const [mcpName, setMcpName] = useState('')
  const [mcpCmd, setMcpCmd] = useState('')
  const [mcpArgs, setMcpArgs] = useState('')
  const [mcpEnv, setMcpEnv] = useState('')
  const [mcpIcon, setMcpIcon] = useState('')
  const [updateMessage, setUpdateMessage] = useState('')
  const [updateAvailable, setUpdateAvailable] = useState(false)

  const isDesktopApp = isTauriRuntime()

  // Load settings on mount
  useEffect(() => {
    async function loadSettings() {
      try {
        let loadedConfig: any = null
        if (window.settingsApi) {
          loadedConfig = await window.settingsApi.getSettings()
        } else {
          const saved = await getProfileValue('user-settings')
          if (saved) {
            loadedConfig = JSON.parse(saved)
          }
        }
        
        if (loadedConfig) {
          const merged = { ...DEFAULT_CONFIG, ...loadedConfig }
          setConfig(merged)
          
          // sync helper custom models
          if (merged.geminiModel && !GEMINI_MODELS.includes(merged.geminiModel)) {
            setCustomGemini(merged.geminiModel)
          }
          if (merged.openaiModel && !OPENAI_MODELS.includes(merged.openaiModel)) {
            setCustomOpenAI(merged.openaiModel)
          }
          if (merged.openrouterModel && !OPENROUTER_MODELS.includes(merged.openrouterModel)) {
            setCustomOpenRouter(merged.openrouterModel)
          }
          if (merged.deepseekModel && !DEEPSEEK_MODELS.includes(merged.deepseekModel)) {
            setCustomDeepSeek(merged.deepseekModel)
          }
          if (merged.anthropicModel && !ANTHROPIC_MODELS.includes(merged.anthropicModel)) {
            setCustomAnthropic(merged.anthropicModel)
          }
          if (merged.hfModel && !HF_MODELS.includes(merged.hfModel)) {
            setCustomHF(merged.hfModel)
          }
          if (merged.localModelName && !LOCAL_MODELS.includes(merged.localModelName)) {
            setCustomLocal(merged.localModelName)
          }
          if (merged.ollamaModel && !OLLAMA_MODELS.includes(merged.ollamaModel)) {
            setCustomOllama(merged.ollamaModel)
          }
          
          applyThemeStyles(merged)
        }
        
        // load name & pref
        const nameVal = await getProfileValue('name')
        if (nameVal) setUserName(nameVal)
        const prefVal = await getProfileValue('preferences')
        if (prefVal) setUserPreferences(prefVal)
      } catch (e) {
        console.error("Failed to load settings:", e)
      }
    }
    loadSettings()
  }, [])

  useEffect(() => {
    const fetchOllamaModels = async () => {
      const host = config.ollamaHost || 'http://127.0.0.1:11434';
      const cleanHost = host.replace(/\/$/, '');
      try {
        const res = await fetch(`${cleanHost}/api/tags`);
        if (res.ok) {
          const data = await res.json();
          if (data && Array.isArray(data.models)) {
            const names = data.models.map((m: any) => m.name);
            setDynamicOllamaModels(names);
            return;
          }
        }
      } catch (err) {
        console.warn("Failed to fetch local Ollama models:", err);
      }
      setDynamicOllamaModels(OLLAMA_MODELS);
    };

    fetchOllamaModels();
  }, [config.ollamaHost]);

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

    try {
      const updated = await authUpdateProfile(profileName, profileImageUrl)
      refreshUser(updated)
    } catch (e) {
      console.error("Failed to save account profile:", e)
    }

    const getActiveModelName = (provider: string, cfg: typeof finalConfig) => {
      switch (provider) {
        case 'gemini': return cfg.geminiModel
        case 'openai': return cfg.openaiModel
        case 'openrouter': return cfg.openrouterModel
        case 'deepseek': return cfg.deepseekModel
        case 'anthropic': return cfg.anthropicModel
        case 'huggingface': return cfg.hfModel
        case 'local_openai': return cfg.localModelName
        case 'ollama': return cfg.ollamaModel
        default: return ''
      }
    }
    const oldModel = getActiveModelName(config.aiProvider, config)
    const newModel = getActiveModelName(finalConfig.aiProvider, finalConfig)
    if (config.aiProvider !== finalConfig.aiProvider || oldModel !== newModel) {
      setActiveModel(finalConfig.aiProvider, newModel).catch(() => {})
    }

    if (window.settingsApi) {
      await window.settingsApi.saveSettings(finalConfig)
      applyThemeStyles(finalConfig)
      window.settingsApi.closeSettings()
    } else {
      try {
        await setProfileValue('user-settings', JSON.stringify(finalConfig))
        applyThemeStyles(finalConfig)
      } catch (e) {
        console.error("Failed to save user settings:", e)
      }
    }
  }

  const handleSaveWithoutClosing = async () => {
    const finalConfig = { ...config }
    
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
    } else {
      try {
        await setProfileValue('user-settings', JSON.stringify(finalConfig))
        applyThemeStyles(finalConfig)
      } catch (e) {
        console.error("Failed to save user settings:", e)
      }
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
    if (!isDesktopApp) return
    if (confirm('Are you sure you want to exit Mint?')) {
      window.settingsApi?.quitApp()
    }
  }

  const handleOpenWorkflows = () => {
    if (!isDesktopApp) {
      alert('Workflow files can only be opened from the desktop app. The web app can still save workflow settings through the Local API.')
      return
    }
    window.settingsApi?.openCustomWorkflows()
  }

  const handleReloadWorkflows = async () => {
    if (!isDesktopApp) {
      alert('Reloading workflow files is only available in the desktop app.')
      return
    }
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

    const argList = mcpArgs.split(/\s+/).filter(Boolean)

    const updatedMcp = {
      ...config.mcpServers,
      [mcpName.trim()]: {
        command: mcpCmd.trim(),
        args: argList,
        env: parsedEnv,
        icon: mcpIcon.trim() || undefined
      }
    }

    setConfig({
      ...config,
      mcpServers: updatedMcp
    })

    setMcpName('')
    setMcpCmd('')
    setMcpArgs('')
    setMcpEnv('')
    setMcpIcon('')
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
    if (!isDesktopApp && plugin === 'discord') {
      alert('Discord Rich Presence requires the desktop app.')
      return
    }
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
    if (!isDesktopApp) {
      setUpdateMessage('Desktop updates are only available in the Tauri app.')
      return
    }
    try {
      const update = await window.settingsApi.checkForUpdates()
      setUpdateAvailable(Boolean(update.available))
      setUpdateMessage(update.available ? `Mint ${update.version} is available.` : 'Mint is up to date.')
    } catch (reason) {
      setUpdateMessage(String(reason))
    }
  }

  const handleInstallUpdate = async () => {
    if (!isDesktopApp) {
      setUpdateMessage('Desktop updates are only available in the Tauri app.')
      return
    }
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
          <span className="settings-icon">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="3"></circle>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
            </svg>
          </span>
          <div>
            <h1>Settings</h1>
            <p>{isDesktopApp ? 'Configure Mint assistant behavior and integrations.' : `Web settings through Local API: ${getLocalApiBase()}`}</p>
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
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="3"></circle>
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
              </svg>
            </span>
            <strong>General</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-profile' ? 'active' : ''}`} onClick={() => setActiveTab('sect-profile')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
                <circle cx="12" cy="7" r="4"></circle>
              </svg>
            </span>
            <strong>Profile</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-memory' ? 'active' : ''}`} onClick={() => setActiveTab('sect-memory')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"></path>
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
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
                <line x1="8" y1="21" x2="16" y2="21"></line>
                <line x1="12" y1="17" x2="12" y2="21"></line>
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
                <polygon points="12 2 2 7 12 12 22 7 12 2"></polygon>
                <polyline points="2 17 12 22 22 17"></polyline>
                <polyline points="2 12 12 17 22 12"></polyline>
              </svg>
            </span>
            <strong>Plugins</strong>
          </button>
          <button className={`tab-btn ${activeTab === 'sect-agents' ? 'active' : ''}`} onClick={() => setActiveTab('sect-agents')}>
            <span style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"></path>
                <circle cx="9" cy="7" r="4"></circle>
                <path d="M23 21v-2a4 4 0 0 0-3-3.87"></path>
                <path d="M16 3.13a4 4 0 0 1 0 7.75"></path>
              </svg>
            </span>
            <strong>Multi-Agent (Beta)</strong>
          </button>
        </nav>

        <div className="settings-content">
          {!isDesktopApp && (
            <div className="web-safe-notice">
              <strong>Web mode</strong>
              <span>Desktop-only controls are hidden or disabled here. Settings are saved through the Local API server.</span>
            </div>
          )}

          {activeTab === 'sect-general' && (
            <GeneralTab
              config={config}
              updateField={updateField}
              customGemini={customGemini}
              setCustomGemini={setCustomGemini}
              customOpenAI={customOpenAI}
              setCustomOpenAI={setCustomOpenAI}
              customOpenRouter={customOpenRouter}
              setCustomOpenRouter={setCustomOpenRouter}
              customDeepSeek={customDeepSeek}
              setCustomDeepSeek={setCustomDeepSeek}
              customAnthropic={customAnthropic}
              setCustomAnthropic={setCustomAnthropic}
              customHF={customHF}
              setCustomHF={setCustomHF}
              customLocal={customLocal}
              setCustomLocal={setCustomLocal}
              customOllama={customOllama}
              setCustomOllama={setCustomOllama}
              dynamicOllamaModels={dynamicOllamaModels}
              updateAvailable={updateAvailable}
              updateMessage={updateMessage}
              handleCheckUpdates={handleCheckUpdates}
              handleInstallUpdate={handleInstallUpdate}
              isDesktopApp={isDesktopApp}
              onSaveWithoutClosing={handleSaveWithoutClosing}
            />
          )}

          {activeTab === 'sect-profile' && (
            <ProfileTab
              name={profileName}
              setName={setProfileName}
              imageUrl={profileImageUrl}
              setImageUrl={setProfileImageUrl}
            />
          )}

          {activeTab === 'sect-memory' && (
            <MemoryTab
              userName={userName}
              setUserName={setUserName}
              userPreferences={userPreferences}
              setUserPreferences={setUserPreferences}
            />
          )}

          {activeTab === 'sect-audio' && (
            <AudioTab
              config={config}
              updateField={updateField}
            />
          )}

          {activeTab === 'sect-automation' && (
            <AutomationTab
              config={config}
              updateField={updateField}
              handleOpenWorkflows={handleOpenWorkflows}
              handleReloadWorkflows={handleReloadWorkflows}
              isDesktopApp={isDesktopApp}
            />
          )}

          {activeTab === 'sect-theme' && (
            <ThemeTab
              config={config}
              updateField={updateField}
            />
          )}

          {activeTab === 'sect-plugins' && (
            <PluginsTab
              config={config}
              updateField={updateField}
              mcpName={mcpName}
              setMcpName={setMcpName}
              mcpCmd={mcpCmd}
              setMcpCmd={setMcpCmd}
              mcpArgs={mcpArgs}
              setMcpArgs={setMcpArgs}
              mcpEnv={mcpEnv}
              setMcpEnv={setMcpEnv}
              mcpIcon={mcpIcon}
              setMcpIcon={setMcpIcon}
              handleAddMcpServer={handleAddMcpServer}
              handleRemoveMcpServer={handleRemoveMcpServer}
              handleConnectPlugin={handleConnectPlugin}
            />
          )}

          {activeTab === 'sect-agents' && (
            <AgentsTab
              config={config}
              updateField={updateField}
              dynamicOllamaModels={dynamicOllamaModels}
            />
          )}
        </div>
      </main>

      <footer className="settings-footer">
        <button type="button" className="btn-danger" onClick={handleQuit} disabled={!isDesktopApp} style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
            <polyline points="16 17 21 12 16 7"></polyline>
            <line x1="21" y1="12" x2="9" y2="12"></line>
          </svg>
          Quit Application
        </button>
        <div className="footer-actions">
          <button type="button" className="btn-secondary" onClick={handleReset} style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="1 4 1 10 7 10"></polyline>
              <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"></path>
            </svg>
            Reset Defaults
          </button>
          <button type="button" className="btn-primary" onClick={(e) => { e.preventDefault(); handleSave(); }} style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path>
              <polyline points="17 21 17 13 7 13 7 21"></polyline>
              <polyline points="7 3 7 8 15 8"></polyline>
            </svg>
            Save Settings
          </button>
        </div>
      </footer>
    </div>
  )
}
