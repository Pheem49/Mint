import React from 'react'
import { 
  DEFAULT_CONFIG,
  GEMINI_MODELS,
  OPENAI_MODELS,
  OPENROUTER_MODELS,
  DEEPSEEK_MODELS,
  ANTHROPIC_MODELS,
  HF_MODELS,
  LOCAL_MODELS
} from '../SettingsWindow'

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
  isDesktopApp = false
}: GeneralTabProps) {
  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">AI routing</p>
            <h2 className="section-title">Provider & Model</h2>
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
      </section>

      {isDesktopApp && (
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
        </section>
      )}

      {isDesktopApp && (
        <section className="setting-section">
          <div className="section-heading">
            <div>
              <p className="section-kicker">Desktop</p>
              <h2 className="section-title">Assistant Presence</h2>
            </div>
          </div>
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
        </section>
      )}
    </div>
  )
}
