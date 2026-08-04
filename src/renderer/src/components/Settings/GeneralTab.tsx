import React from 'react'
import { 
  DEFAULT_CONFIG,
  GEMINI_MODELS,
  OPENAI_MODELS,
  OPENROUTER_MODELS,
  DEEPSEEK_MODELS,
  ANTHROPIC_MODELS,
  HF_MODELS,
  LOCAL_MODELS,
  CustomProviderConfig,
  CustomProviderModel,
  CustomProviderHeader,
} from '../SettingsWindow'
import { setActiveModel } from '../../../shared/utils/modelManager'

interface GeneralTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
  customGemini: string
  setCustomGemini: (val: string) => void
  customOpenAI: string
  setCustomOpenAI: (val: string) => void
  customOpenRouter: string
  setCustomOpenRouter: (val: string) => void
  customDeepSeek: string
  setCustomDeepSeek: (val: string) => void
  customAnthropic: string
  setCustomAnthropic: (val: string) => void
  customHF: string
  setCustomHF: (val: string) => void
  customLocal: string
  setCustomLocal: (val: string) => void
  customOllama: string
  setCustomOllama: (val: string) => void
  dynamicOllamaModels: string[]
  updateAvailable: boolean
  updateMessage: string
  handleCheckUpdates: () => void
  handleInstallUpdate: () => void
  isDesktopApp?: boolean
  onSaveWithoutClosing?: () => Promise<void>
}

export default function GeneralTab({
  config,
  updateField,
  customGemini,
  setCustomGemini,
  customOpenAI,
  setCustomOpenAI,
  customOpenRouter,
  setCustomOpenRouter,
  customDeepSeek,
  setCustomDeepSeek,
  customAnthropic,
  setCustomAnthropic,
  customHF,
  setCustomHF,
  customLocal,
  setCustomLocal,
  customOllama,
  setCustomOllama,
  dynamicOllamaModels,
  updateAvailable,
  updateMessage,
  handleCheckUpdates,
  handleInstallUpdate,
  isDesktopApp = true,
  onSaveWithoutClosing
}: GeneralTabProps) {
  const [savedProviderId, setSavedProviderId] = React.useState<string | null>(null)
  const [openSections, setOpenSections] = React.useState<Record<string, boolean>>({
    ai_routing: true,
    search: false,
    productivity: false,
    image_gen: false,
    video_gen: false,
    custom_providers: false,
    desktop_updates: false,
    assistant_presence: false,
  })

  const toggleSection = (key: string) => {
    setOpenSections(prev => ({ ...prev, [key]: !prev[key] }))
  }

  const setAllSections = (isOpen: boolean) => {
    setOpenSections({
      ai_routing: isOpen,
      search: isOpen,
      productivity: isOpen,
      image_gen: isOpen,
      video_gen: isOpen,
      custom_providers: isOpen,
      desktop_updates: isOpen,
      assistant_presence: isOpen,
    })
  }

  const handleSaveProvider = async (providerId: string) => {
    if (onSaveWithoutClosing) {
      await onSaveWithoutClosing()
      setSavedProviderId(providerId)
      setTimeout(() => setSavedProviderId(null), 2000)
    }
  }

  const renderCollapsibleSection = (
    key: string,
    kicker: string,
    title: string,
    description: string,
    children: React.ReactNode,
    badgeContent?: React.ReactNode,
    iconSVG?: React.ReactNode
  ) => {
    const isOpen = openSections[key] ?? false

    return (
      <section 
        className={`setting-section collapsible-section ${isOpen ? 'is-open' : 'is-collapsed'}`}
        style={{
          border: '1px solid var(--border)',
          borderRadius: '12px',
          padding: '16px 20px',
          background: 'var(--surface-bg)',
          marginBottom: '14px',
          transition: 'all 0.25s ease',
          overflow: 'hidden'
        }}
      >
        <div 
          className="section-heading-collapsible"
          onClick={() => toggleSection(key)}
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            cursor: 'pointer',
            userSelect: 'none'
          }}
        >
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <p className="section-kicker" style={{ margin: 0 }}>{kicker}</p>
              {badgeContent}
            </div>
            <h2 className="section-title" style={{ margin: '2px 0 0 0', fontSize: '1.15rem', display: 'flex', alignItems: 'center', gap: '8px' }}>
              {iconSVG}
              {title}
            </h2>
            {description && <p className="section-description" style={{ margin: '4px 0 0 0', fontSize: '0.82rem' }}>{description}</p>}
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: 'var(--text-muted)' }}>
            <span style={{ fontSize: '0.75rem', fontWeight: '500' }}>{isOpen ? 'Collapse' : 'Expand'}</span>
            <svg 
              width="18" height="18" viewBox="0 0 24 24" fill="none" 
              stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"
              style={{ 
                transform: isOpen ? 'rotate(180deg)' : 'rotate(0deg)',
                transition: 'transform 0.25s ease'
              }}
            >
              <polyline points="6 9 12 15 18 9"></polyline>
            </svg>
          </div>
        </div>

        {isOpen && (
          <div className="section-collapsible-body" style={{ marginTop: '16px', paddingTop: '16px', borderTop: '1px solid var(--border)' }}>
            {children}
          </div>
        )}
      </section>
    )
  }

  return (
    <div className="tab-pane active">
      {/* Accordion Quick Control Bar */}
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', marginBottom: '14px' }}>
        <button 
          className="btn-secondary" 
          onClick={() => setAllSections(true)}
          style={{ fontSize: '0.78rem', padding: '4px 10px', borderRadius: '6px', cursor: 'pointer', border: '1px solid var(--border)', background: 'var(--surface-strong)', color: 'var(--text-main)' }}
        >
          Expand All
        </button>
        <button 
          className="btn-secondary" 
          onClick={() => setAllSections(false)}
          style={{ fontSize: '0.78rem', padding: '4px 10px', borderRadius: '6px', cursor: 'pointer', border: '1px solid var(--border)', background: 'var(--surface-strong)', color: 'var(--text-main)' }}
        >
          Collapse All
        </button>
      </div>

      {/* ── Section 1: AI Routing ── */}
      {renderCollapsibleSection(
        'ai_routing',
        'AI routing',
        'Provider & Model',
        'Choose which AI backend Mint uses, then set the default model.',
        <>
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
                {(config.customProviders ?? []).map(cp => (
                  <option key={`custom:${cp.id}`} value={`custom:${cp.id}`}>
                    {cp.displayName || cp.id} (Custom)
                  </option>
                ))}
              </select>
            </div>
            {config.aiProvider.startsWith('custom:') && (() => {
              const activeId = config.aiProvider.replace(/^custom:/, '')
              const cp = (config.customProviders ?? []).find(p => p.id === activeId)
              if (!cp || cp.models.length === 0) return null
              const currentModel = (config.customModelSelections ?? {})[activeId] ?? cp.models[0]?.modelId ?? ''
              return (
                <div className="setting-row wide">
                  <label>Active Model</label>
                  <select
                    value={currentModel}
                    onChange={(e) => updateField('customModelSelections', {
                      ...(config.customModelSelections ?? {}),
                      [activeId]: e.target.value
                    })}
                  >
                    {cp.models.map(m => (
                      <option key={m.modelId} value={m.modelId}>
                        {m.displayName || m.modelId}
                      </option>
                    ))}
                  </select>
                </div>
              )
            })()}
          </div>

          <div className="provider-cards-container">
            {/* Google Gemini Card */}
            <div className={`provider-card ${config.aiProvider === 'gemini' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
                  </svg>
                  Google Gemini (Cloud)
                </div>
                {config.aiProvider === 'gemini' && <span className="provider-active-badge">Active</span>}
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>Gemini Model</label>
                  <select 
                    value={(GEMINI_MODELS as readonly string[]).includes(config.geminiModel) ? config.geminiModel : 'custom'} 
                    onChange={(e) => updateField('geminiModel', e.target.value)}
                  >
                    {GEMINI_MODELS.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!(GEMINI_MODELS as readonly string[]).includes(config.geminiModel) || config.geminiModel === 'custom') && (
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
                  <label>Gemini API Key</label>
                  <input 
                    type="password" 
                    value={config.apiKey} 
                    onChange={(e) => updateField('apiKey', e.target.value)} 
                    placeholder="Enter Gemini API Key..." 
                  />
                </div>
              </div>
            </div>

            {/* Anthropic Claude Card */}
            <div className={`provider-card ${config.aiProvider === 'anthropic' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M4.5 16.5c-1.5 1.26-2.5 3.19-2.5 5.5h20c0-2.31-1-4.24-2.5-5.5"></path>
                    <path d="M12 2L2 22h20L12 2z"></path>
                  </svg>
                  Anthropic Claude
                </div>
                {config.aiProvider === 'anthropic' && <span className="provider-active-badge">Active</span>}
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>Anthropic Model</label>
                  <select 
                    value={(ANTHROPIC_MODELS as readonly string[]).includes(config.anthropicModel) ? config.anthropicModel : 'custom'} 
                    onChange={(e) => updateField('anthropicModel', e.target.value)}
                  >
                    {ANTHROPIC_MODELS.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!(ANTHROPIC_MODELS as readonly string[]).includes(config.anthropicModel) || config.anthropicModel === 'custom') && (
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
                  <label>Anthropic API Key</label>
                  <input 
                    type="password" 
                    value={config.anthropicApiKey} 
                    onChange={(e) => updateField('anthropicApiKey', e.target.value)} 
                    placeholder="Enter Anthropic API Key..." 
                  />
                </div>
              </div>
            </div>

            {/* OpenAI Card */}
            <div className={`provider-card ${config.aiProvider === 'openai' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="12" cy="12" r="10"></circle>
                    <line x1="12" y1="2" x2="12" y2="22"></line>
                    <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"></path>
                  </svg>
                  OpenAI
                </div>
                {config.aiProvider === 'openai' && <span className="provider-active-badge">Active</span>}
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>OpenAI Model</label>
                  <select 
                    value={(OPENAI_MODELS as readonly string[]).includes(config.openaiModel) ? config.openaiModel : 'custom'} 
                    onChange={(e) => updateField('openaiModel', e.target.value)}
                  >
                    {OPENAI_MODELS.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!(OPENAI_MODELS as readonly string[]).includes(config.openaiModel) || config.openaiModel === 'custom') && (
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
                  <label>OpenAI API Key</label>
                  <input 
                    type="password" 
                    value={config.openaiApiKey} 
                    onChange={(e) => updateField('openaiApiKey', e.target.value)} 
                    placeholder="Enter OpenAI API Key..." 
                  />
                </div>
              </div>
            </div>

            {/* OpenRouter Card */}
            <div className={`provider-card ${config.aiProvider === 'openrouter' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="16 3 21 3 21 8"></polyline>
                    <line x1="4" y1="20" x2="21" y2="3"></line>
                    <polyline points="21 16 21 21 16 21"></polyline>
                    <line x1="15" y1="15" x2="21" y2="21"></line>
                    <line x1="4" y1="4" x2="9" y2="9"></line>
                  </svg>
                  OpenRouter
                </div>
                {config.aiProvider === 'openrouter' && <span className="provider-active-badge">Active</span>}
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>OpenRouter Model</label>
                  <select
                    value={(OPENROUTER_MODELS as readonly string[]).includes(config.openrouterModel) ? config.openrouterModel : 'custom'}
                    onChange={(e) => updateField('openrouterModel', e.target.value)}
                  >
                    {OPENROUTER_MODELS.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!(OPENROUTER_MODELS as readonly string[]).includes(config.openrouterModel) || config.openrouterModel === 'custom') && (
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
                  <label>OpenRouter API Key</label>
                  <input
                    type="password"
                    value={config.openrouterApiKey}
                    onChange={(e) => updateField('openrouterApiKey', e.target.value)}
                    placeholder="Enter OpenRouter API Key..."
                  />
                </div>
              </div>
            </div>

            {/* DeepSeek Card */}
            <div className={`provider-card ${config.aiProvider === 'deepseek' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="11" cy="11" r="8"></circle>
                    <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                  </svg>
                  DeepSeek
                </div>
                {config.aiProvider === 'deepseek' && <span className="provider-active-badge">Active</span>}
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>DeepSeek Model</label>
                  <select
                    value={(DEEPSEEK_MODELS as readonly string[]).includes(config.deepseekModel) ? config.deepseekModel : 'custom'}
                    onChange={(e) => updateField('deepseekModel', e.target.value)}
                  >
                    {DEEPSEEK_MODELS.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!(DEEPSEEK_MODELS as readonly string[]).includes(config.deepseekModel) || config.deepseekModel === 'custom') && (
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
                  <label>DeepSeek API Key</label>
                  <input
                    type="password"
                    value={config.deepseekApiKey}
                    onChange={(e) => updateField('deepseekApiKey', e.target.value)}
                    placeholder="Enter DeepSeek API Key..."
                  />
                </div>
              </div>
            </div>

            {/* Hugging Face Card */}
            <div className={`provider-card ${config.aiProvider === 'huggingface' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="12" cy="12" r="10"></circle>
                    <path d="M8 14s1.5 2 4 2 4-2 4-2"></path>
                    <line x1="9" y1="9" x2="9.01" y2="9"></line>
                    <line x1="15" y1="9" x2="15.01" y2="9"></line>
                  </svg>
                  Hugging Face (Inference API)
                </div>
                {config.aiProvider === 'huggingface' && <span className="provider-active-badge">Active</span>}
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>Hugging Face Model</label>
                  <select 
                    value={(HF_MODELS as readonly string[]).includes(config.hfModel) ? config.hfModel : 'custom'} 
                    onChange={(e) => updateField('hfModel', e.target.value)}
                  >
                    {HF_MODELS.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!(HF_MODELS as readonly string[]).includes(config.hfModel) || config.hfModel === 'custom') && (
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
                  <label>Hugging Face API Key</label>
                  <input 
                    type="password" 
                    value={config.hfApiKey} 
                    onChange={(e) => updateField('hfApiKey', e.target.value)} 
                    placeholder="Enter Hugging Face API Key..." 
                  />
                </div>
              </div>
            </div>

            {/* LM Studio Card */}
            <div className={`provider-card ${config.aiProvider === 'local_openai' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
                    <line x1="8" y1="21" x2="16" y2="21"></line>
                    <line x1="12" y1="17" x2="12" y2="21"></line>
                  </svg>
                  LM Studio / Local OpenAI
                </div>
                {config.aiProvider === 'local_openai' && <span className="provider-active-badge">Active</span>}
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>LM Studio Model</label>
                  <select 
                    value={(LOCAL_MODELS as readonly string[]).includes(config.localModelName) ? config.localModelName : 'custom'} 
                    onChange={(e) => updateField('localModelName', e.target.value)}
                  >
                    {LOCAL_MODELS.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!(LOCAL_MODELS as readonly string[]).includes(config.localModelName) || config.localModelName === 'custom') && (
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
                  <label>LM Studio Base URL</label>
                  <input 
                    type="text" 
                    value={config.localApiBaseUrl} 
                    onChange={(e) => updateField('localApiBaseUrl', e.target.value)} 
                    placeholder="e.g. http://localhost:1234/v1" 
                  />
                </div>
              </div>
            </div>

            {/* Ollama Card */}
            <div className={`provider-card ${config.aiProvider === 'ollama' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
                  </svg>
                  Ollama (Local)
                </div>
                {config.aiProvider === 'ollama' && <span className="provider-active-badge">Active</span>}
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>Ollama Model</label>
                  <select 
                    value={dynamicOllamaModels.includes(config.ollamaModel) ? config.ollamaModel : 'custom'} 
                    onChange={(e) => updateField('ollamaModel', e.target.value)}
                  >
                    {dynamicOllamaModels.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    {!dynamicOllamaModels.includes(config.ollamaModel) && config.ollamaModel && config.ollamaModel !== 'custom' && (
                      <option value={config.ollamaModel}>{config.ollamaModel}</option>
                    )}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!dynamicOllamaModels.includes(config.ollamaModel) || config.ollamaModel === 'custom') && (
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
            </div>
          </div>
        </>
      )}

      {/* ── Section 2: Web Search ── */}
      {renderCollapsibleSection(
        'search',
        'Web search',
        'Search Provider',
        'Choose which search API Mint uses when performing web lookups.',
        <div className="provider-cards-container">
          {/* Brave Search */}
          <div className={`provider-card ${config.searchProvider === 'brave' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                </svg>
                Brave Search API
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.searchProvider === 'brave' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="searchProvider" style={{ display: 'none' }} checked={config.searchProvider === 'brave'} onChange={() => updateField('searchProvider', 'brave')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('searchProvider', 'brave')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>Brave Search API Key</label>
                <input
                  type="password"
                  value={config.braveSearchApiKey}
                  onChange={(e) => updateField('braveSearchApiKey', e.target.value)}
                  placeholder="Enter Brave Search API Key..."
                />
              </div>
            </div>
          </div>

          {/* Google Search */}
          <div className={`provider-card ${config.searchProvider === 'google' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="11" cy="11" r="8"/>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"/>
                </svg>
                Google Search API
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.searchProvider === 'google' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="searchProvider" style={{ display: 'none' }} checked={config.searchProvider === 'google'} onChange={() => updateField('searchProvider', 'google')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('searchProvider', 'google')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>Google Search API Key</label>
                <input
                  type="password"
                  value={config.googleSearchApiKey}
                  onChange={(e) => updateField('googleSearchApiKey', e.target.value)}
                  placeholder="Enter Google Search API Key..."
                />
              </div>
              <div className="setting-row">
                <label>Google Search Engine ID (CX)</label>
                <input
                  type="text"
                  value={config.googleSearchCx}
                  onChange={(e) => updateField('googleSearchCx', e.target.value)}
                  placeholder="Enter Google Custom Search Engine ID (CX)..."
                />
              </div>
            </div>
          </div>

          {/* SearXNG */}
          <div className={`provider-card ${config.searchProvider === 'searxng' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="11" cy="11" r="8"/>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"/>
                </svg>
                SearXNG (self-hosted)
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.searchProvider === 'searxng' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="searchProvider" style={{ display: 'none' }} checked={config.searchProvider === 'searxng'} onChange={() => updateField('searchProvider', 'searxng')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('searchProvider', 'searxng')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>SearXNG Instance URL</label>
                <input
                  type="text"
                  value={config.searxngBaseUrl}
                  onChange={(e) => updateField('searxngBaseUrl', e.target.value)}
                  placeholder="e.g. https://searx.example.com"
                />
              </div>
              <p className="hint" style={{ margin: 0 }}>Self-hosted, no API key needed. The instance must have JSON output enabled (search.formats in settings.yml).</p>
            </div>
          </div>
        </div>
      )}



      {/* ── Section 4: Image Generation ── */}
      {renderCollapsibleSection(
        'image_gen',
        'AI image creation',
        'Image Generation',
        'Choose which image generation provider Mint uses.',
        <div className="provider-cards-container">
          {/* Gemini Images */}
          <div className={`provider-card ${config.imageGenProvider === 'gemini' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
                Google NanoBanana (Gemini Images)
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.imageGenProvider === 'gemini' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="imageGenProvider" style={{ display: 'none' }} checked={config.imageGenProvider === 'gemini'} onChange={() => updateField('imageGenProvider', 'gemini')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('imageGenProvider', 'gemini')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <p className="hint" style={{ margin: 0 }}>Uses your Gemini API key — no extra key needed.</p>
            </div>
          </div>

          {/* DALL·E */}
          <div className={`provider-card ${config.imageGenProvider === 'dalle' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="12" cy="12" r="10"/>
                  <line x1="12" y1="2" x2="12" y2="22"/>
                  <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/>
                </svg>
                OpenAI DALL·E
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.imageGenProvider === 'dalle' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="imageGenProvider" style={{ display: 'none' }} checked={config.imageGenProvider === 'dalle'} onChange={() => updateField('imageGenProvider', 'dalle')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('imageGenProvider', 'dalle')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <p className="hint" style={{ margin: 0 }}>Uses your OpenAI API key — no extra key needed.</p>
            </div>
          </div>

          {/* Stability AI */}
          <div className={`provider-card ${config.imageGenProvider === 'stability' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M12 22C6.477 22 2 17.523 2 12S6.477 2 12 2s10 4.477 10 10-4.477 10-10 10z"/>
                  <path d="M8 12l2 2 4-4"/>
                </svg>
                Stability AI (Stable Diffusion)
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.imageGenProvider === 'stability' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="imageGenProvider" style={{ display: 'none' }} checked={config.imageGenProvider === 'stability'} onChange={() => updateField('imageGenProvider', 'stability')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('imageGenProvider', 'stability')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>Stability AI API Key</label>
                <input
                  type="password"
                  value={config.stabilityApiKey}
                  onChange={(e) => updateField('stabilityApiKey', e.target.value)}
                  placeholder="Enter Stability AI API Key..."
                />
              </div>
            </div>
          </div>

          {/* Ideogram */}
          <div className={`provider-card ${config.imageGenProvider === 'ideogram' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="3" y="3" width="18" height="18" rx="2"/>
                  <circle cx="8.5" cy="8.5" r="1.5"/>
                  <polyline points="21 15 16 10 5 21"/>
                </svg>
                Ideogram v3
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.imageGenProvider === 'ideogram' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="imageGenProvider" style={{ display: 'none' }} checked={config.imageGenProvider === 'ideogram'} onChange={() => updateField('imageGenProvider', 'ideogram')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('imageGenProvider', 'ideogram')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>Ideogram API Key</label>
                <input
                  type="password"
                  value={config.ideogramApiKey}
                  onChange={(e) => updateField('ideogramApiKey', e.target.value)}
                  placeholder="Enter Ideogram API Key..."
                />
              </div>
            </div>
          </div>

          {/* Replicate */}
          <div className={`provider-card ${config.imageGenProvider === 'replicate' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="16 3 21 3 21 8"/>
                  <line x1="4" y1="20" x2="21" y2="3"/>
                  <polyline points="21 16 21 21 16 21"/>
                  <line x1="15" y1="15" x2="21" y2="21"/>
                  <line x1="4" y1="4" x2="9" y2="9"/>
                </svg>
                Replicate (FLUX / SDXL / custom)
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.imageGenProvider === 'replicate' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="imageGenProvider" style={{ display: 'none' }} checked={config.imageGenProvider === 'replicate'} onChange={() => updateField('imageGenProvider', 'replicate')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('imageGenProvider', 'replicate')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>Replicate API Key</label>
                <input
                  type="password"
                  value={config.replicateApiKey}
                  onChange={(e) => updateField('replicateApiKey', e.target.value)}
                  placeholder="Enter Replicate API Key..."
                />
              </div>
            </div>
          </div>

          {/* Black Forest Labs */}
          <div className={`provider-card ${config.imageGenProvider === 'bfl' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
                </svg>
                Black Forest Labs (FLUX API)
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.imageGenProvider === 'bfl' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="imageGenProvider" style={{ display: 'none' }} checked={config.imageGenProvider === 'bfl'} onChange={() => updateField('imageGenProvider', 'bfl')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('imageGenProvider', 'bfl')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>Black Forest Labs API Key</label>
                <input
                  type="password"
                  value={config.bflApiKey}
                  onChange={(e) => updateField('bflApiKey', e.target.value)}
                  placeholder="Enter BFL API Key..."
                />
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── Section 5: Video Generation ── */}
      {renderCollapsibleSection(
        'video_gen',
        'AI video creation',
        'Video Generation',
        'Choose which video generation provider Mint uses.',
        <div className="provider-cards-container">
          {/* Veo */}
          <div className={`provider-card ${config.videoGenProvider === 'veo' ? 'active-provider' : ''}`}>
            <div className="provider-card-header">
              <div className="provider-card-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <polygon points="23 7 16 12 23 17 23 7" />
                  <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
                </svg>
                Google Veo (Gemini Videos)
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                {config.videoGenProvider === 'veo' && <span className="provider-active-badge">Active</span>}
                <label className="settings-toggle-switch" style={{ marginBottom: 0 }}>
                  <input type="radio" name="videoGenProvider" style={{ display: 'none' }} checked={config.videoGenProvider === 'veo'} onChange={() => updateField('videoGenProvider', 'veo')} />
                  <span className="settings-toggle-slider" style={{ cursor: 'pointer' }} onClick={() => updateField('videoGenProvider', 'veo')} />
                </label>
              </div>
            </div>
            <div className="provider-card-body">
              <p className="hint" style={{ margin: 0 }}>Uses your Gemini API key — no extra key needed.</p>
              <div className="setting-row" style={{ marginTop: '12px' }}>
                <label>Default Veo Model</label>
                <select
                  value={config.veoModel || 'veo-3.1-generate-preview'}
                  onChange={(e) => {
                    updateField('veoModel', e.target.value)
                    setActiveModel('veoModel', e.target.value, 'video')
                  }}
                  style={{ width: '100%', background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', color: '#fff', padding: '6px', borderRadius: '4px' }}
                >
                  <option value="veo-3.1-generate-preview">veo-3.1-generate-preview (Default)</option>
                  <option value="veo-3.1-fast-generate-preview">veo-3.1-fast-generate-preview</option>
                  <option value="veo-3.1-lite-generate-preview">veo-3.1-lite-generate-preview</option>
                </select>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── Section 6: Custom Providers ── */}
      {renderCollapsibleSection(
        'custom_providers',
        'Custom AI Routing',
        'Custom Providers',
        'Add any OpenAI-compatible endpoint as a custom provider. Configure its models and optional headers.',
        <>
          <div className="provider-cards-container">
            {(config.customProviders ?? []).map((cp, cpIdx) => {
              const isActive = config.aiProvider === `custom:${cp.id}`

              const updateCp = (patch: Partial<CustomProviderConfig>) => {
                const updated = (config.customProviders ?? []).map((p, i) =>
                  i === cpIdx ? { ...p, ...patch } : p
                )
                updateField('customProviders', updated)
              }

              const deleteCp = () => {
                const updated = (config.customProviders ?? []).filter((_, i) => i !== cpIdx)
                updateField('customProviders', updated)
                if (config.aiProvider === `custom:${cp.id}`) {
                  updateField('aiProvider', 'gemini')
                }
              }

              const updateModel = (mIdx: number, patch: Partial<CustomProviderModel>) => {
                const updatedModels = cp.models.map((m, i) => i === mIdx ? { ...m, ...patch } : m)
                updateCp({ models: updatedModels })
              }
              const deleteModel = (mIdx: number) => {
                updateCp({ models: cp.models.filter((_, i) => i !== mIdx) })
              }
              const addModel = () => {
                updateCp({ models: [...cp.models, { modelId: '', displayName: '' }] })
              }

              const updateHeader = (hIdx: number, patch: Partial<CustomProviderHeader>) => {
                const updatedHeaders = cp.headers.map((h, i) => i === hIdx ? { ...h, ...patch } : h)
                updateCp({ headers: updatedHeaders })
              }
              const deleteHeader = (hIdx: number) => {
                updateCp({ headers: cp.headers.filter((_, i) => i !== hIdx) })
              }
              const addHeader = () => {
                updateCp({ headers: [...cp.headers, { name: '', value: '' }] })
              }

              return (
                <div key={cpIdx} className={`provider-card ${isActive ? 'active-provider' : ''}`}>
                  <div className="provider-card-header">
                    <div className="provider-card-title">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <circle cx="12" cy="12" r="3" /><path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14" />
                      </svg>
                      {cp.displayName || cp.id || 'Unnamed Provider'}
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      {isActive && <span className="provider-active-badge">Active</span>}
                      <button
                        className="btn btn-primary"
                        style={{ padding: '3px 8px', fontSize: '12px', background: savedProviderId === cp.id ? '#10b981' : '' }}
                        onClick={() => handleSaveProvider(cp.id)}
                        title="Save this provider settings"
                      >
                        {savedProviderId === cp.id ? 'Saved! ✓' : 'Save'}
                      </button>
                      <button
                        className="btn btn-danger"
                        style={{ padding: '3px 8px', fontSize: '12px' }}
                        onClick={deleteCp}
                        title="Delete this provider"
                      >
                        Delete
                      </button>
                    </div>
                  </div>

                  <div className="provider-card-body">
                    <div className="setting-row">
                      <label>Name</label>
                      <input
                        type="text"
                        value={cp.displayName}
                        onChange={(e) => {
                          const name = e.target.value
                          const computedId = name
                            .toLowerCase()
                            .replace(/[^a-z0-9_-]/g, '-')
                            .replace(/-+/g, '-')
                            .replace(/^-|-$/g, '') || `provider-${cpIdx + 1}`
                          updateCp({ displayName: name, id: computedId })
                        }}
                        placeholder="e.g. DeepSeek"
                      />
                    </div>

                    <div className="setting-row">
                      <label>Base URL</label>
                      <input
                        type="text"
                        value={cp.baseUrl}
                        onChange={(e) => updateCp({ baseUrl: e.target.value })}
                        placeholder="https://api.myprovider.com/v1"
                      />
                    </div>

                    <div className="setting-row">
                      <label>API Key</label>
                      <input
                        type="password"
                        value={cp.apiKey}
                        onChange={(e) => updateCp({ apiKey: e.target.value })}
                        placeholder="Optional. Leave empty if you manage auth via headers."
                      />
                    </div>

                    <div style={{ marginTop: '12px' }}>
                      <label style={{ fontWeight: 600, fontSize: '13px', display: 'block', marginBottom: '8px' }}>Model list</label>
                      {cp.models.map((m, mIdx) => (
                        <div key={mIdx} style={{ display: 'flex', gap: '8px', marginBottom: '6px', alignItems: 'center' }}>
                          <input
                            type="text"
                            value={m.modelId}
                            onChange={(e) => updateModel(mIdx, { modelId: e.target.value, displayName: e.target.value })}
                            placeholder="e.g. llama-3.1-8b"
                            style={{ flex: 1 }}
                          />
                          <button
                            className="btn btn-danger"
                            style={{ padding: '4px 8px', fontSize: '12px', flexShrink: 0 }}
                            onClick={() => deleteModel(mIdx)}
                            title="Remove model"
                          >🗑</button>
                        </div>
                      ))}
                      <button
                        className="btn btn-secondary"
                        style={{ fontSize: '12px', marginTop: '4px' }}
                        onClick={addModel}
                      >
                        + Add model
                      </button>
                    </div>

                    <div style={{ marginTop: '12px' }}>
                      <label style={{ fontWeight: 600, fontSize: '13px', display: 'block', marginBottom: '8px' }}>Headers (optional)</label>
                      {cp.headers.map((h, hIdx) => (
                        <div key={hIdx} style={{ display: 'flex', gap: '8px', marginBottom: '6px', alignItems: 'center' }}>
                          <input
                            type="text"
                            value={h.name}
                            onChange={(e) => updateHeader(hIdx, { name: e.target.value })}
                            placeholder="Header-Name"
                            style={{ flex: 1 }}
                          />
                          <input
                            type="text"
                            value={h.value}
                            onChange={(e) => updateHeader(hIdx, { value: e.target.value })}
                            placeholder="value"
                            style={{ flex: 1 }}
                          />
                          <button
                            className="btn btn-danger"
                            style={{ padding: '4px 8px', fontSize: '12px', flexShrink: 0 }}
                            onClick={() => deleteHeader(hIdx)}
                            title="Remove header"
                          >🗑</button>
                        </div>
                      ))}
                      <button
                        className="btn btn-secondary"
                        style={{ fontSize: '12px', marginTop: '4px' }}
                        onClick={addHeader}
                      >
                        + Add header
                      </button>
                    </div>
                  </div>
                </div>
              )
            })}
          </div>

          <div className="setting-actions" style={{ marginTop: '16px' }}>
            <button
              className="btn btn-secondary"
              onClick={() => {
                const newId = `provider-${(config.customProviders ?? []).length + 1}`
                updateField('customProviders', [
                  ...(config.customProviders ?? []),
                  { id: newId, displayName: '', baseUrl: '', apiKey: '', models: [], headers: [] }
                ])
              }}
            >
              + Add custom provider
            </button>
          </div>
        </>
      )}

      {/* ── Section 7: Desktop Updates ── */}
      {isDesktopApp && renderCollapsibleSection(
        'desktop_updates',
        'Desktop updates',
        'Signed Tauri Channel',
        'Check and explicitly install signed Tauri releases from your configured update channel.',
        <>
          <div className="toggle-row">
            <div>
              <label>Check signed update channel</label>
              <p className="hint">This flag does not install updates automatically.</p>
            </div>
            <label className="settings-toggle-switch">
              <input
                type="checkbox"
                checked={config.enableAutoUpdate}
                onChange={(e) => updateField('enableAutoUpdate', e.target.checked)}
              />
              <span className="settings-toggle-slider"></span>
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
        </>
      )}

      {/* ── Section 8: Assistant Presence ── */}
      {isDesktopApp && renderCollapsibleSection(
        'assistant_presence',
        'Desktop',
        'Assistant Presence',
        'Configure the mini AI character desktop presence widget.',
        <div className="toggle-row">
          <div>
            <label>Show Desktop AI Candidate</label>
            <p className="hint">Show the mini AI character on your desktop.</p>
          </div>
          <label className="settings-toggle-switch">
            <input 
              type="checkbox" 
              checked={config.showDesktopWidget} 
              onChange={(e) => updateField('showDesktopWidget', e.target.checked)} 
            />
            <span className="settings-toggle-slider"></span>
          </label>
        </div>
      )}
    </div>
  )
}
