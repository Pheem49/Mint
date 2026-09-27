import React from 'react'
import { DEFAULT_CONFIG } from '../../constants/config'
import { 
  GEMINI_MODELS,
  OPENAI_MODELS,
  OPENROUTER_MODELS,
  DEEPSEEK_MODELS,
  ANTHROPIC_MODELS,
  HF_MODELS,
  LOCAL_MODELS,
  IMAGE_STUDIO_MODELS,
  IMAGE_GEN_PROVIDER_MODELS,
  VEO_STUDIO_MODELS,
  getModelMetadata,
} from '../../constants/models'
import type {
  CustomProviderConfig,
  CustomProviderModel,
  CustomProviderHeader,
} from '../../types'
import { setActiveModel } from '../../utils/modelManager'
import { providerLabel as aiProviderLabel } from '../../utils/providers'
import ApiKeyInput from './ApiKeyInput'
import SearchableModelCombobox from './SearchableModelCombobox'
import anthropicLogo from '@lobehub/icons-static-svg/icons/anthropic.svg?url'
import deepseekLogo from '@lobehub/icons-static-svg/icons/deepseek-color.svg?url'
import geminiLogo from '@lobehub/icons-static-svg/icons/gemini-color.svg?url'
import huggingFaceLogo from '@lobehub/icons-static-svg/icons/huggingface-color.svg?url'
import lmStudioLogo from '@lobehub/icons-static-svg/icons/lmstudio.svg?url'
import ollamaLogo from '@lobehub/icons-static-svg/icons/ollama.svg?url'
import openAiLogo from '@lobehub/icons-static-svg/icons/openai.svg?url'
import openRouterLogo from '@lobehub/icons-static-svg/icons/openrouter-color.svg?url'

const settingsProviderLogos: Record<string, { src: string; color?: string }> = {
  anthropic: { src: anthropicLogo, color: '#d97757' },
  deepseek: { src: deepseekLogo },
  gemini: { src: geminiLogo },
  huggingface: { src: huggingFaceLogo },
  local_openai: { src: lmStudioLogo, color: '#8b5cf6' },
  ollama: { src: ollamaLogo, color: '#f3f4f6' },
  openai: { src: openAiLogo, color: '#10a37f' },
  openrouter: { src: openRouterLogo },
}

function SettingsProviderLogo({ provider }: { provider: string }) {
  const logo = settingsProviderLogos[provider]
  if (!logo) return null

  if (logo.color) {
    return (
      <span
        className="settings-provider-logo is-monochrome"
        aria-hidden="true"
        style={{
          '--provider-brand-color': logo.color,
          maskImage: `url("${logo.src}")`,
          WebkitMaskImage: `url("${logo.src}")`,
        } as React.CSSProperties}
      />
    )
  }

  return <img className="settings-provider-logo" src={logo.src} alt="" aria-hidden="true" />
}

// One card per image-gen provider (mirrors the chat "Provider & Model"
// cards): a model dropdown plus either its own API-key field, or a note
// that it reuses a shared key.
const IMAGE_PROVIDERS: Array<{
  id: string
  label: string
  cardTitle: string
  keyField?: keyof typeof DEFAULT_CONFIG
  sharedKey?: string
}> = [
  { id: 'gemini', label: 'NanoBanana', cardTitle: 'Google NanoBanana (Gemini Images)', sharedKey: 'Gemini' },
  { id: 'dalle', label: 'DALL·E', cardTitle: 'OpenAI DALL·E', sharedKey: 'OpenAI' },
  { id: 'stability', label: 'Stability AI', cardTitle: 'Stability AI (Stable Diffusion)', keyField: 'stabilityApiKey' },
  { id: 'ideogram', label: 'Ideogram', cardTitle: 'Ideogram', keyField: 'ideogramApiKey' },
  { id: 'replicate', label: 'Replicate', cardTitle: 'Replicate (FLUX / SDXL / custom)', keyField: 'replicateApiKey' },
  { id: 'bfl', label: 'Black Forest Labs', cardTitle: 'Black Forest Labs (FLUX API)', keyField: 'bflApiKey' },
]

const imageProviderIcon = (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
    <circle cx="8.5" cy="8.5" r="1.5"></circle>
    <polyline points="21 15 16 10 5 21"></polyline>
  </svg>
)

const SEARCH_PROVIDER_LABELS: Record<string, string> = {
  brave: 'Brave',
  google: 'Google',
  searxng: 'SearXNG',
}

const IMAGE_PROVIDER_LABELS: Record<string, string> = {
  gemini: 'NanoBanana',
  dalle: 'DALL·E',
  stability: 'Stability',
  ideogram: 'Ideogram',
  replicate: 'Replicate',
  bfl: 'FLUX',
}

const VIDEO_PROVIDER_LABELS: Record<string, string> = {
  veo: 'Veo',
}

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
  /** Dynamic model lists fetched from provider APIs. Falls back to static
   *  presets from `shared/constants/models.ts` when not provided. */
  dynamicGeminiModels?: string[]
  dynamicAnthropicModels?: string[]
  dynamicOpenAIModels?: string[]
  dynamicOpenRouterModels?: string[]
  dynamicDeepSeekModels?: string[]
  dynamicLocalModels?: string[]
  /** Dynamic image model lists fetched from provider APIs, keyed by listKey ('nanobanana', 'dalle', etc.).
   *  Falls back to static presets from `shared/constants/models.ts` when not provided. */
  dynamicImageModels?: Partial<Record<string, Array<{ value: string; label: string }>>>
  /** Dynamic video model lists fetched from provider APIs, keyed by provider ('veo').
   *  Falls back to static presets from `shared/constants/models.ts` when not provided. */
  dynamicVideoModels?: Partial<Record<string, Array<{ value: string; label: string }>>>
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
  dynamicGeminiModels = [...GEMINI_MODELS],
  dynamicAnthropicModels = [...ANTHROPIC_MODELS],
  dynamicOpenAIModels = [...OPENAI_MODELS],
  dynamicOpenRouterModels = [...OPENROUTER_MODELS],
  dynamicDeepSeekModels = [...DEEPSEEK_MODELS],
  dynamicLocalModels = [...LOCAL_MODELS],
  dynamicImageModels,
  dynamicVideoModels,
  updateAvailable,
  updateMessage,
  handleCheckUpdates,
  handleInstallUpdate,
  isDesktopApp = true,
  onSaveWithoutClosing
}: GeneralTabProps) {
  const [savedProviderId, setSavedProviderId] = React.useState<string | null>(null)
  const [providerLogoErrors, setProviderLogoErrors] = React.useState<Record<string, string>>({})
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

  const handleProviderLogoChange = (providerId: string, file?: File) => {
    if (!file) return
    const supportedTypes = ['image/png', 'image/jpeg', 'image/webp']
    if (!supportedTypes.includes(file.type)) {
      setProviderLogoErrors(prev => ({ ...prev, [providerId]: 'Choose a PNG, JPEG, or WebP image.' }))
      return
    }
    if (file.size > 256 * 1024) {
      setProviderLogoErrors(prev => ({ ...prev, [providerId]: 'The logo must be 256 KB or smaller.' }))
      return
    }

    const reader = new FileReader()
    reader.onload = () => {
      if (typeof reader.result !== 'string') {
        setProviderLogoErrors(prev => ({ ...prev, [providerId]: 'Could not read this image.' }))
        return
      }
      const provider = (config.customProviders ?? []).find(item => item.id === providerId)
      if (!provider) return
      const updated = (config.customProviders ?? []).map(item =>
        item.id === providerId ? { ...item, logoDataUrl: reader.result as string } : item
      )
      updateField('customProviders', updated)
      setProviderLogoErrors(prev => {
        const next = { ...prev }
        delete next[providerId]
        return next
      })
    }
    reader.onerror = () => {
      setProviderLogoErrors(prev => ({ ...prev, [providerId]: 'Could not read this image.' }))
    }
    reader.readAsDataURL(file)
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
      >
        <div
          className="section-heading-collapsible"
          onClick={() => toggleSection(key)}
          role="button"
          aria-expanded={isOpen}
          aria-label={`${isOpen ? 'Collapse' : 'Expand'} ${title}`}
        >
          <div>
            <p className="section-kicker">
              {kicker}
              {badgeContent}
            </p>
            <h2 className="section-title">
              {iconSVG}
              {title}
            </h2>
          </div>
          <svg
            className="section-chevron"
            width="18" height="18" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"
          >
            <polyline points="6 9 12 15 18 9"></polyline>
          </svg>
        </div>

        {isOpen && <div className="section-collapsible-body">{children}</div>}
      </section>
    )
  }

  const activeProviderLabel = config.aiProvider.startsWith('custom:')
    ? ((config.customProviders ?? []).find(p => `custom:${p.id}` === config.aiProvider)?.displayName || 'Custom')
    : aiProviderLabel(config.aiProvider)
  const activeSearchLabel = SEARCH_PROVIDER_LABELS[config.searchProvider] ?? config.searchProvider
  const activeImageLabel = IMAGE_PROVIDER_LABELS[config.imageGenProvider] ?? config.imageGenProvider
  const activeVideoLabel = VIDEO_PROVIDER_LABELS[config.videoGenProvider] ?? config.videoGenProvider
  const customProviderCount = (config.customProviders ?? []).length

  return (
    <div className="tab-pane active">
      {/* Accordion Quick Control Bar */}
      <div className="accordion-controls">
        <button type="button" onClick={() => setAllSections(true)}>Expand all</button>
        <button type="button" onClick={() => setAllSections(false)}>Collapse all</button>
      </div>

      {/* ── Section 1: AI Routing ── */}
      {renderCollapsibleSection(
        'ai_routing',
        'AI routing',
        'Provider & Model',
        'Choose which AI backend Mint uses, then set the default model.',
        <>
          <div className="form-grid compact">
            <div className="setting-row stacked">
              <label>Active provider</label>
              <div className="pill-segmented" role="radiogroup" aria-label="Active provider">
                {[
                  { id: 'gemini', label: aiProviderLabel('gemini'), title: 'Google Gemini (Cloud)' },
                  { id: 'anthropic', label: aiProviderLabel('anthropic'), title: 'Anthropic Claude' },
                  { id: 'openai', label: aiProviderLabel('openai'), title: 'OpenAI' },
                  { id: 'openrouter', label: aiProviderLabel('openrouter'), title: 'OpenRouter' },
                  { id: 'deepseek', label: aiProviderLabel('deepseek'), title: 'DeepSeek' },
                  { id: 'ollama', label: aiProviderLabel('ollama'), title: 'Ollama (Local / Private)' },
                  { id: 'huggingface', label: aiProviderLabel('huggingface'), title: 'Hugging Face (Inference API)' },
                  { id: 'local_openai', label: aiProviderLabel('local_openai'), title: 'Local (LM Studio / OpenAI Compatible)' },
                  ...(config.customProviders ?? []).map(cp => ({
                    id: `custom:${cp.id}`,
                    label: cp.displayName || cp.id,
                    title: `${cp.displayName || cp.id} (Custom)`,
                  })),
                ].map(o => (
                  <button
                    key={o.id}
                    type="button"
                    role="radio"
                    aria-checked={config.aiProvider === o.id}
                    title={o.title}
                    className={`pill-segmented-btn ${config.aiProvider === o.id ? 'active' : ''}`}
                    onClick={() => updateField('aiProvider', o.id)}
                  >
                    {o.label}
                  </button>
                ))}
              </div>
            </div>
            {config.aiProvider.startsWith('custom:') && (() => {
              const activeId = config.aiProvider.replace(/^custom:/, '')
              const cp = (config.customProviders ?? []).find(p => p.id === activeId)
              if (!cp || cp.models.length === 0) return null
              const currentModel = (config.customModelSelections ?? {})[activeId] ?? cp.models[0]?.modelId ?? ''
              return (
                <div className="setting-row wide">
                  <label>Active model</label>
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

            {(() => {
              const activeModelRaw = (
                config.aiProvider === 'gemini' ? config.geminiModel :
                config.aiProvider === 'openai' ? config.openaiModel :
                config.aiProvider === 'anthropic' ? config.anthropicModel :
                config.aiProvider === 'deepseek' ? config.deepseekModel :
                config.aiProvider === 'ollama' ? config.ollamaModel :
                config.aiProvider === 'openrouter' ? config.openrouterModel :
                config.aiProvider === 'huggingface' ? config.hfModel :
                config.aiProvider === 'local_openai' ? config.localModelName :
                (config.aiProvider?.startsWith('custom:') ? ((config.customModelSelections ?? {})[config.aiProvider.replace(/^custom:/, '')] ?? '') : '')
              ) || ''
              const activeModel = activeModelRaw.toLowerCase()
              const isReasoningOmit = (config.aiProvider === 'openai' || config.aiProvider === 'openrouter' || config.aiProvider === 'local_openai') &&
                (activeModel.startsWith('o1') || activeModel.startsWith('o3'))
              const isReasoningCoT = config.aiProvider === 'deepseek' ||
                activeModel.includes('deepseek') ||
                activeModel.includes('qwq') ||
                activeModel.includes('thinking') ||
                activeModel.includes('reasoner') ||
                activeModel.includes('r1')
              const isCodestral = activeModel.includes('codestral')

              const smartDefault = isReasoningCoT ? 0.6 : isCodestral ? 0.1 : 0.2
              const modelTemps = (config.modelTemperatures ?? {}) as Record<string, number>
              const customModelTemp = activeModelRaw ? modelTemps[activeModelRaw] : undefined
              const hasCustomModel = customModelTemp !== undefined
              const hasCustomGlobal = config.temperature !== null && config.temperature !== undefined
              const hasCustom = hasCustomModel || hasCustomGlobal

              const currentTemp = hasCustomModel
                ? customModelTemp!
                : hasCustomGlobal
                ? config.temperature!
                : smartDefault

              const modelDisplayName = (() => {
                if (!activeModelRaw) return ''
                const trimmed = activeModelRaw.trim()
                const base = trimmed.includes('/') ? trimmed.split('/').pop()! : trimmed
                const noTag = base.includes(':') ? base.split(':')[0]! : base
                const lower = noTag.toLowerCase()

                const KNOWN_MAP: Record<string, string> = {
                  'deepseek-v4-flash': 'DeepSeek V4 Flash',
                  'deepseek-v4-pro': 'DeepSeek V4 Pro',
                  'deepseek-chat': 'DeepSeek Chat',
                  'deepseek-reasoner': 'DeepSeek Reasoner',
                  'claude-sonnet-5': 'Claude Sonnet 5',
                  'claude-opus-5': 'Claude Opus 5',
                  'claude-sonnet-4.6': 'Claude Sonnet 4.6',
                  'claude-haiku-4.5': 'Claude Haiku 4.5',
                  'gemini-2.5-flash': 'Gemini 2.5 Flash',
                  'gemini-3.5-flash': 'Gemini 3.5 Flash',
                  'gemini-3.6-flash': 'Gemini 3.6 Flash',
                  'gpt-5.6-luna': 'GPT-5.6 Luna',
                  'gpt-5.6-terra': 'GPT-5.6 Terra',
                  'gpt-5.6-sol': 'GPT-5.6 Sol',
                  'gpt-4o': 'GPT-4o',
                  'gpt-4o-mini': 'GPT-4o Mini',
                  'llama3': 'Llama 3',
                }
                if (KNOWN_MAP[lower]) return KNOWN_MAP[lower]

                return noTag
                  .split(/[-_]/)
                  .filter(Boolean)
                  .map(w => w.charAt(0).toUpperCase() + w.slice(1))
                  .join(' ')
              })()

              const autoLabel = isReasoningOmit
                ? `Auto (Omitted for ${modelDisplayName || 'Reasoning'})`
                : isReasoningCoT
                ? 'Auto (0.60 for Reasoning)'
                : isCodestral
                ? 'Auto (0.10 for Codestral)'
                : 'Auto (0.20 for Coding)'

              const badgeLabel = hasCustomModel
                ? `Custom for ${modelDisplayName || activeModelRaw} (${customModelTemp!.toFixed(2)})`
                : hasCustomGlobal
                ? `Custom Global (${config.temperature!.toFixed(2)})`
                : autoLabel

              const handleReset = () => {
                if (activeModelRaw && modelTemps[activeModelRaw] !== undefined) {
                  const updated = { ...modelTemps }
                  delete updated[activeModelRaw]
                  updateField('modelTemperatures', updated)
                }
                if (config.temperature !== null && config.temperature !== undefined) {
                  updateField('temperature', null)
                }
              }

              const handleChange = (val: number) => {
                if (activeModelRaw) {
                  const updated = { ...modelTemps, [activeModelRaw]: val }
                  updateField('modelTemperatures', updated)
                } else {
                  updateField('temperature', val)
                }
              }

              return (
                <div className="setting-row stacked setting-feature-card">
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '0.35rem' }}>
                    <label style={{ margin: 0, fontWeight: 600, display: 'flex', alignItems: 'baseline', gap: '0.45rem' }}>
                      <span>Model temperature</span>
                      {modelDisplayName && (
                        <span style={{ fontSize: '0.8rem', fontWeight: 500, color: 'var(--text-muted)' }}>
                          · {modelDisplayName}
                        </span>
                      )}
                    </label>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                      <span className="section-current-badge" style={{ fontSize: '0.75rem' }}>
                        {hasCustomModel ? `Custom (${customModelTemp!.toFixed(2)})` : hasCustomGlobal ? `Custom Global (${config.temperature!.toFixed(2)})` : autoLabel}
                      </span>
                      {hasCustom && (
                        <button
                          type="button"
                          className="btn-secondary btn-small"
                          style={{ padding: '0.15rem 0.45rem', fontSize: '0.75rem' }}
                          onClick={handleReset}
                          title={`Reset ${modelDisplayName || activeModelRaw || 'model'} to smart default`}
                        >
                          Reset to Auto
                        </button>
                      )}
                    </div>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                    <input
                      type="range"
                      min="0"
                      max="1.5"
                      step="0.05"
                      value={currentTemp}
                      onChange={(e) => handleChange(parseFloat(e.target.value))}
                      style={{ flex: 1, accentColor: 'var(--accent, #10b981)', cursor: 'pointer' }}
                    />
                    <span style={{ fontSize: '0.85rem', minWidth: '2.5rem', textAlign: 'right', fontFamily: 'monospace', fontWeight: 600 }}>
                      {currentTemp.toFixed(2)}
                    </span>
                  </div>
                  <span style={{ fontSize: '0.75rem', color: 'var(--text-muted)', marginTop: '0.35rem', lineHeight: 1.4 }}>
                    Controls sampling entropy for <strong>{modelDisplayName || activeModelRaw}</strong>. {
                      isReasoningOmit
                        ? 'Reasoning models (o1/o3) automatically omit this parameter.'
                        : isReasoningCoT
                        ? 'Smart default is 0.60 to prevent repetitive reasoning loops.'
                        : isCodestral
                        ? 'Smart default is 0.10 for deterministic code completion.'
                        : 'Smart default is 0.20 for general coding and tool execution.'
                    }
                  </span>
                </div>
              )
            })()}

            {/* Thinking / Reasoning Configuration */}
            {(() => {
              const activeModelRaw = (() => {
                if (config.aiProvider === 'gemini') return config.geminiModel || 'gemini-2.5-flash'
                if (config.aiProvider === 'anthropic') return config.anthropicModel || 'claude-sonnet-5'
                if (config.aiProvider === 'openai') return config.openaiModel || 'gpt-5.6-luna'
                if (config.aiProvider === 'openrouter') return config.openrouterModel || 'openai/gpt-5.6-terra'
                if (config.aiProvider === 'deepseek') return config.deepseekModel || 'deepseek-chat'
                if (config.aiProvider === 'local_openai') return config.localModelName || 'local-model'
                if (config.aiProvider === 'ollama') return config.ollamaModel || 'llama3'
                if (config.aiProvider === 'huggingface') return config.hfModel || 'Qwen/Qwen3.6-27B'
                return ''
              })()

              const meta = getModelMetadata(activeModelRaw, config.aiProvider)
              const modelConfigs = (config.modelThinkingConfigs ?? {}) as Record<string, { enabled?: boolean; effort?: string }>
              const customModelThinking = activeModelRaw ? modelConfigs[activeModelRaw] : undefined
              const hasCustomModel = customModelThinking !== undefined

              const isThinkingSupported = meta.supportsThinking
              const currentEnabled = customModelThinking?.enabled !== undefined
                ? customModelThinking.enabled
                : (config.thinkingEnabled ?? true)

              const currentEffort = (customModelThinking?.effort || config.thinkingEffort || 'medium').toLowerCase()

              const handleResetThinking = () => {
                if (activeModelRaw && modelConfigs[activeModelRaw] !== undefined) {
                  const updated = { ...modelConfigs }
                  delete updated[activeModelRaw]
                  updateField('modelThinkingConfigs', updated)
                }
              }

              const handleToggleThinking = (enabled: boolean) => {
                if (activeModelRaw) {
                  const updated = {
                    ...modelConfigs,
                    [activeModelRaw]: { enabled, effort: currentEffort },
                  }
                  updateField('modelThinkingConfigs', updated)
                }
                updateField('thinkingEnabled', enabled)
              }

              const handleEffortChange = (effort: string) => {
                if (activeModelRaw) {
                  const updated = {
                    ...modelConfigs,
                    [activeModelRaw]: { enabled: currentEnabled, effort },
                  }
                  updateField('modelThinkingConfigs', updated)
                }
                updateField('thinkingEffort', effort)
              }

              const effortLabelMap: Record<string, string> = {
                low: 'Low',
                medium: 'Medium',
                high: 'High',
                extra_high: 'Extra High',
              }

              return (
                <div className="setting-row stacked setting-feature-card">
                  <div className="thinking-card-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '0.5rem' }}>
                    <label className="thinking-card-title" style={{ margin: 0, fontWeight: 600, display: 'flex', alignItems: 'center', gap: '0.45rem' }}>
                      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ color: 'var(--interactive-fg)' }}>
                        <path d="M12 2a7 7 0 0 1 7 7c0 2.38-1.19 4.47-3 5.74V17a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2v-2.26C6.19 13.47 5 11.38 5 9a7 7 0 0 1 7-7z" />
                        <path d="M9 21h6" />
                      </svg>
                      <span>Thinking / Reasoning</span>
                      {activeModelRaw && (
                        <span style={{ fontSize: '0.8rem', fontWeight: 500, color: 'var(--text-muted)' }}>
                          · {activeModelRaw.split('/').pop()}
                        </span>
                      )}
                    </label>
                    <div className="thinking-card-status" style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                      <span className="section-current-badge" style={{ fontSize: '0.75rem' }}>
                        {currentEnabled
                          ? `Thinking ON (${effortLabelMap[currentEffort] || 'Medium'})${!isThinkingSupported ? ' · Force' : ''}`
                          : 'Thinking OFF'}
                      </span>
                      {hasCustomModel && (
                        <button
                          type="button"
                          className="btn-secondary btn-small"
                          style={{ padding: '0.15rem 0.45rem', fontSize: '0.75rem' }}
                          onClick={handleResetThinking}
                          title={`Reset ${activeModelRaw} thinking configuration`}
                        >
                          Reset
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Toggle Row */}
                  <div className="thinking-card-toggle-row" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '0.4rem 0' }}>
                    <div>
                      <span style={{ fontSize: '0.82rem', fontWeight: 500 }}>Enable thinking</span>
                      <p style={{ margin: '2px 0 0 0', fontSize: '0.73rem', color: 'var(--text-muted)' }}>
                        {isThinkingSupported
                          ? 'Allow model to output chain-of-thought tokens.'
                          : currentEnabled
                          ? 'Chain-of-thought token generation enabled (force toggle).'
                          : 'Model not flagged as reasoning by default — toggle on to force enable.'}
                      </p>
                    </div>
                    <label className="settings-toggle-switch">
                      <input
                        type="checkbox"
                        checked={currentEnabled}
                        onChange={(e) => handleToggleThinking(e.target.checked)}
                      />
                      <span className="settings-toggle-slider"></span>
                    </label>
                  </div>

                  {/* Effort Level Row */}
                  {currentEnabled && (
                    <div style={{ marginTop: '0.5rem', paddingTop: '0.5rem', borderTop: '1px solid var(--border)' }}>
                      <div className="thinking-card-effort-row" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                        <span style={{ fontSize: '0.78rem', fontWeight: 600, color: 'var(--text-muted)', textTransform: 'none', letterSpacing: '0.04em' }}>
                          Reasoning Effort
                        </span>
                        <div className="thinking-card-effort-options" style={{ display: 'flex', gap: '4px' }}>
                          {(['low', 'medium', 'high', 'extra_high'] as const).map((eff) => (
                            <button
                              key={eff}
                              type="button"
                              className={`btn-secondary btn-small ${currentEffort === eff ? 'active' : ''}`}
                              style={{
                                padding: '0.2rem 0.6rem',
                                fontSize: '0.75rem',
                                fontWeight: currentEffort === eff ? 600 : 400,
                                background: currentEffort === eff ? 'rgba(16, 185, 129, 0.15)' : undefined,
                                borderColor: currentEffort === eff ? 'var(--accent, #10b981)' : undefined,
                                color: currentEffort === eff ? 'var(--accent, #10b981)' : undefined,
                              }}
                              onClick={() => handleEffortChange(eff)}
                            >
                              {effortLabelMap[eff]}
                            </button>
                          ))}
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              )
            })()}
          </div>

          <div className="provider-cards-container">
            {/* Google Gemini Card */}
            <div className={`provider-card ${config.aiProvider === 'gemini' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <SettingsProviderLogo provider="gemini" />
                  Google Gemini (Cloud)
                </div>
                <div className="provider-card-actions">
                  {config.aiProvider === 'gemini' ? (
                    <span className="provider-active-badge">Active</span>
                  ) : (
                    <button type="button" className="btn-secondary btn-small" onClick={() => updateField('aiProvider', 'gemini')}>
                      Set active
                    </button>
                  )}
                </div>
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>Gemini model</label>
                  <select 
                    value={dynamicGeminiModels.includes(config.geminiModel) ? config.geminiModel : 'custom'} 
                    onChange={(e) => updateField('geminiModel', e.target.value)}
                  >
                    {dynamicGeminiModels.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!dynamicGeminiModels.includes(config.geminiModel) || config.geminiModel === 'custom') && (
                  <div className="setting-row">
                    <label>Custom Gemini model</label>
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
                  <ApiKeyInput
                    value={config.apiKey}
                    onChange={(value) => updateField('apiKey', value)}
                    placeholder="Enter Gemini API Key..."
                  />
                </div>
              </div>
            </div>

            {/* Anthropic Claude Card */}
            <div className={`provider-card ${config.aiProvider === 'anthropic' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <SettingsProviderLogo provider="anthropic" />
                  Anthropic Claude
                </div>
                <div className="provider-card-actions">
                  {config.aiProvider === 'anthropic' ? (
                    <span className="provider-active-badge">Active</span>
                  ) : (
                    <button type="button" className="btn-secondary btn-small" onClick={() => updateField('aiProvider', 'anthropic')}>
                      Set active
                    </button>
                  )}
                </div>
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>Anthropic model</label>
                  <select 
                    value={dynamicAnthropicModels.includes(config.anthropicModel) ? config.anthropicModel : 'custom'} 
                    onChange={(e) => updateField('anthropicModel', e.target.value)}
                  >
                    {dynamicAnthropicModels.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!dynamicAnthropicModels.includes(config.anthropicModel) || config.anthropicModel === 'custom') && (
                  <div className="setting-row">
                    <label>Custom Anthropic model</label>
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
                  <ApiKeyInput
                    value={config.anthropicApiKey}
                    onChange={(value) => updateField('anthropicApiKey', value)}
                    placeholder="Enter Anthropic API Key..."
                  />
                </div>
              </div>
            </div>

            {/* OpenAI Card */}
            <div className={`provider-card ${config.aiProvider === 'openai' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <SettingsProviderLogo provider="openai" />
                  OpenAI
                </div>
                <div className="provider-card-actions">
                  {config.aiProvider === 'openai' ? (
                    <span className="provider-active-badge">Active</span>
                  ) : (
                    <button type="button" className="btn-secondary btn-small" onClick={() => updateField('aiProvider', 'openai')}>
                      Set active
                    </button>
                  )}
                </div>
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>OpenAI Model</label>
                  <select 
                    value={dynamicOpenAIModels.includes(config.openaiModel) ? config.openaiModel : 'custom'} 
                    onChange={(e) => updateField('openaiModel', e.target.value)}
                  >
                    {dynamicOpenAIModels.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!dynamicOpenAIModels.includes(config.openaiModel) || config.openaiModel === 'custom') && (
                  <div className="setting-row">
                    <label>Custom OpenAI model</label>
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
                  <ApiKeyInput
                    value={config.openaiApiKey}
                    onChange={(value) => updateField('openaiApiKey', value)}
                    placeholder="Enter OpenAI API Key..."
                  />
                </div>
              </div>
            </div>

            {/* OpenRouter Card */}
            <div className={`provider-card ${config.aiProvider === 'openrouter' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <SettingsProviderLogo provider="openrouter" />
                  OpenRouter
                </div>
                <div className="provider-card-actions">
                  {config.aiProvider === 'openrouter' ? (
                    <span className="provider-active-badge">Active</span>
                  ) : (
                    <button type="button" className="btn-secondary btn-small" onClick={() => updateField('aiProvider', 'openrouter')}>
                      Set active
                    </button>
                  )}
                </div>
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>OpenRouter Model</label>
                  <SearchableModelCombobox
                    value={dynamicOpenRouterModels.includes(config.openrouterModel) ? config.openrouterModel : 'custom'}
                    models={dynamicOpenRouterModels}
                    onChange={(val) => updateField('openrouterModel', val)}
                    onCustomChange={(val) => {
                      setCustomOpenRouter(val)
                      updateField('openrouterModel', 'custom')
                    }}
                    customValue={customOpenRouter}
                    placeholder="Select or search OpenRouter model..."
                  />
                </div>
                {(!dynamicOpenRouterModels.includes(config.openrouterModel) || config.openrouterModel === 'custom') && (
                  <div className="setting-row">
                    <label>Custom OpenRouter model</label>
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
                  <ApiKeyInput
                    value={config.openrouterApiKey}
                    onChange={(value) => updateField('openrouterApiKey', value)}
                    placeholder="Enter OpenRouter API Key..."
                  />
                </div>
              </div>
            </div>

            {/* DeepSeek Card */}
            <div className={`provider-card ${config.aiProvider === 'deepseek' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <SettingsProviderLogo provider="deepseek" />
                  DeepSeek
                </div>
                <div className="provider-card-actions">
                  {config.aiProvider === 'deepseek' ? (
                    <span className="provider-active-badge">Active</span>
                  ) : (
                    <button type="button" className="btn-secondary btn-small" onClick={() => updateField('aiProvider', 'deepseek')}>
                      Set active
                    </button>
                  )}
                </div>
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>DeepSeek Model</label>
                  <select
                    value={dynamicDeepSeekModels.includes(config.deepseekModel) ? config.deepseekModel : 'custom'}
                    onChange={(e) => updateField('deepseekModel', e.target.value)}
                  >
                    {dynamicDeepSeekModels.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!dynamicDeepSeekModels.includes(config.deepseekModel) || config.deepseekModel === 'custom') && (
                  <div className="setting-row">
                    <label>Custom DeepSeek model</label>
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
                  <ApiKeyInput
                    value={config.deepseekApiKey}
                    onChange={(value) => updateField('deepseekApiKey', value)}
                    placeholder="Enter DeepSeek API Key..."
                  />
                </div>
              </div>
            </div>

            {/* Hugging Face Card */}
            <div className={`provider-card ${config.aiProvider === 'huggingface' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <SettingsProviderLogo provider="huggingface" />
                  Hugging Face (Inference API)
                </div>
                <div className="provider-card-actions">
                  {config.aiProvider === 'huggingface' ? (
                    <span className="provider-active-badge">Active</span>
                  ) : (
                    <button type="button" className="btn-secondary btn-small" onClick={() => updateField('aiProvider', 'huggingface')}>
                      Set active
                    </button>
                  )}
                </div>
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>Hugging Face model</label>
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
                    <label>Custom Hugging Face model</label>
                    <input 
                      type="text" 
                      value={customHF} 
                      onChange={(e) => { setCustomHF(e.target.value); updateField('hfModel', 'custom') }} 
                      placeholder="e.g. meta-llama/Meta-Llama-3-8B-Instruct" 
                    />
                  </div>
                )}
                <div className="setting-row">
                  <label>Hugging Face API key</label>
                  <ApiKeyInput
                    value={config.hfApiKey}
                    onChange={(value) => updateField('hfApiKey', value)}
                    placeholder="Enter Hugging Face API key..."
                  />
                </div>
              </div>
            </div>

            {/* LM Studio Card */}
            <div className={`provider-card ${config.aiProvider === 'local_openai' ? 'active-provider' : ''}`}>
              <div className="provider-card-header">
                <div className="provider-card-title">
                  <SettingsProviderLogo provider="local_openai" />
                  LM Studio / Local OpenAI
                </div>
                <div className="provider-card-actions">
                  {config.aiProvider === 'local_openai' ? (
                    <span className="provider-active-badge">Active</span>
                  ) : (
                    <button type="button" className="btn-secondary btn-small" onClick={() => updateField('aiProvider', 'local_openai')}>
                      Set active
                    </button>
                  )}
                </div>
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>LM Studio Model</label>
                  <select 
                    value={dynamicLocalModels.includes(config.localModelName) ? config.localModelName : 'custom'} 
                    onChange={(e) => updateField('localModelName', e.target.value)}
                  >
                    {dynamicLocalModels.map(model => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                    <option value="custom">Custom...</option>
                  </select>
                </div>
                {(!dynamicLocalModels.includes(config.localModelName) || config.localModelName === 'custom') && (
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
                  <SettingsProviderLogo provider="ollama" />
                  Ollama (Local)
                </div>
                <div className="provider-card-actions">
                  {config.aiProvider === 'ollama' ? (
                    <span className="provider-active-badge">Active</span>
                  ) : (
                    <button type="button" className="btn-secondary btn-small" onClick={() => updateField('aiProvider', 'ollama')}>
                      Set active
                    </button>
                  )}
                </div>
              </div>
              <div className="provider-card-body">
                <div className="setting-row">
                  <label>Ollama model</label>
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
                    <label>Custom Ollama model</label>
                    <input 
                      type="text" 
                      value={customOllama} 
                      onChange={(e) => { setCustomOllama(e.target.value); updateField('ollamaModel', 'custom') }} 
                      placeholder="e.g. llama3:latest" 
                    />
                  </div>
                )}
                <div className="setting-row">
                  <label>Ollama host</label>
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
        </>,
        <span className="section-current-badge">{activeProviderLabel}</span>
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
              <div className="provider-card-actions">
                {config.searchProvider === 'brave' ? (
                  <span className="provider-active-badge">Active</span>
                ) : (
                  <button type="button" className="btn-secondary btn-small" onClick={() => updateField('searchProvider', 'brave')}>
                    Set active
                  </button>
                )}
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>Brave Search API key</label>
                <ApiKeyInput
                  value={config.braveSearchApiKey}
                  onChange={(value) => updateField('braveSearchApiKey', value)}
                  placeholder="Enter Brave Search API key..."
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
              <div className="provider-card-actions">
                {config.searchProvider === 'google' ? (
                  <span className="provider-active-badge">Active</span>
                ) : (
                  <button type="button" className="btn-secondary btn-small" onClick={() => updateField('searchProvider', 'google')}>
                    Set active
                  </button>
                )}
              </div>
            </div>
            <div className="provider-card-body">
              <div className="setting-row">
                <label>Google Search API key</label>
                <ApiKeyInput
                  value={config.googleSearchApiKey}
                  onChange={(value) => updateField('googleSearchApiKey', value)}
                  placeholder="Enter Google Search API key..."
                />
              </div>
              <div className="setting-row">
                <label>Google Search engine ID (CX)</label>
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
              <div className="provider-card-actions">
                {config.searchProvider === 'searxng' ? (
                  <span className="provider-active-badge">Active</span>
                ) : (
                  <button type="button" className="btn-secondary btn-small" onClick={() => updateField('searchProvider', 'searxng')}>
                    Set active
                  </button>
                )}
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
              <p className="hint">Self-hosted, no API key needed. The instance must have JSON output enabled (search.formats in settings.yml).</p>
            </div>
          </div>
        </div>,
        <span className="section-current-badge">{activeSearchLabel}</span>
      )}


      {/* ── Section 4: Image Generation ── */}
      {renderCollapsibleSection(
        'image_gen',
        'AI image creation',
        'Image Generation',
        'Choose which image generation provider Mint uses.',
        <>
          <div className="form-grid compact">
            <div className="setting-row stacked">
              <label>Active provider</label>
              <div className="pill-segmented" role="radiogroup" aria-label="Image Provider">
                {IMAGE_PROVIDERS.map(o => (
                  <button
                    key={o.id}
                    type="button"
                    role="radio"
                    aria-checked={config.imageGenProvider === o.id}
                    title={o.cardTitle}
                    className={`pill-segmented-btn ${config.imageGenProvider === o.id ? 'active' : ''}`}
                    onClick={() => updateField('imageGenProvider', o.id)}
                  >
                    {o.label}
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div className="provider-cards-container">
            {IMAGE_PROVIDERS.map(prov => {
              const entry = IMAGE_GEN_PROVIDER_MODELS[prov.id]
              const defaultOpts = entry ? (IMAGE_STUDIO_MODELS[entry.listKey] ?? []) : []
              const opts = (entry && dynamicImageModels?.[entry.listKey]) || defaultOpts
              const modelField = entry?.configField as keyof typeof DEFAULT_CONFIG | undefined
              const currentModel = modelField ? ((config as any)[modelField] || opts[0]?.value || '') : ''
              return (
                <div key={prov.id} className={`provider-card ${config.imageGenProvider === prov.id ? 'active-provider' : ''}`}>
                  <div className="provider-card-header">
                    <div className="provider-card-title">
                      {imageProviderIcon}
                      {prov.cardTitle}
                    </div>
                    <div className="provider-card-actions">
                      {config.imageGenProvider === prov.id ? (
                        <span className="provider-active-badge">Active</span>
                      ) : (
                        <button
                          type="button"
                          className="btn-secondary btn-small"
                          onClick={() => updateField('imageGenProvider', prov.id)}
                        >
                          Set active
                        </button>
                      )}
                    </div>
                  </div>
                  <div className="provider-card-body">
                    {modelField && opts.length > 0 && (
                      <div className="setting-row">
                        <label>{prov.label} Model</label>
                        <select value={currentModel} onChange={(e) => updateField(modelField, e.target.value)}>
                          {opts.map(m => (
                            <option key={m.value} value={m.value}>{m.label}</option>
                          ))}
                          {currentModel && !opts.some(m => m.value === currentModel) && (
                            <option key={currentModel} value={currentModel}>{currentModel}</option>
                          )}
                        </select>
                      </div>
                    )}
                    {prov.keyField ? (
                      <div className="setting-row">
                        <label>{prov.label} API Key</label>
                        <ApiKeyInput
                          value={(config as any)[prov.keyField] || ''}
                          onChange={(value) => updateField(prov.keyField!, value)}
                          placeholder={`Enter ${prov.label} API Key...`}
                        />
                      </div>
                    ) : (
                      <p className="hint">Uses your {prov.sharedKey} API key — no extra key needed.</p>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        </>,
        <span className="section-current-badge">{activeImageLabel}</span>
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
              <div className="provider-card-actions">
                {config.videoGenProvider === 'veo' ? (
                  <span className="provider-active-badge">Active</span>
                ) : (
                  <button type="button" className="btn-secondary btn-small" onClick={() => updateField('videoGenProvider', 'veo')}>
                    Set active
                  </button>
                )}
              </div>
            </div>
            <div className="provider-card-body">
              <p className="hint">Uses your Gemini API key — no extra key needed.</p>
              <div className="setting-row">
                <label>Default Veo model</label>
                {(() => {
                  const veoOpts = dynamicVideoModels?.veo || VEO_STUDIO_MODELS.veo || []
                  const currentVeoModel = config.veoModel || 'veo-3.1-generate-preview'
                  return (
                    <select
                      value={currentVeoModel}
                      onChange={(e) => {
                        updateField('veoModel', e.target.value)
                        setActiveModel('veoModel', e.target.value, 'video')
                      }}
                    >
                      {veoOpts.map((m) => (
                        <option key={m.value} value={m.value}>{m.label}</option>
                      ))}
                      {currentVeoModel && !veoOpts.some((m) => m.value === currentVeoModel) && (
                        <option key={currentVeoModel} value={currentVeoModel}>{currentVeoModel}</option>
                      )}
                    </select>
                  )
                })()}
              </div>
            </div>
          </div>
        </div>,
        <span className="section-current-badge">{activeVideoLabel}</span>
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
                      {cp.logoDataUrl ? (
                        <img className="settings-provider-logo custom-provider-logo" src={cp.logoDataUrl} alt="" />
                      ) : (
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                          <circle cx="12" cy="12" r="3" /><path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14" />
                        </svg>
                      )}
                      {cp.displayName || cp.id || 'Unnamed Provider'}
                    </div>
                    <div className="provider-card-actions">
                      {isActive ? (
                        <span className="provider-active-badge">Active</span>
                      ) : (
                        <button
                          type="button"
                          className="btn-secondary btn-small"
                          onClick={() => updateField('aiProvider', `custom:${cp.id}`)}
                        >
                          Set active
                        </button>
                      )}
                      <button
                        className="btn btn-primary btn-xs"
                        onClick={() => handleSaveProvider(cp.id)}
                        title="Save this provider settings"
                      >
                        {savedProviderId === cp.id ? 'Saved! ✓' : 'Save'}
                      </button>
                      <button
                        className="btn btn-danger btn-xs"
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
                      <label>Provider logo (optional)</label>
                      <div className="custom-provider-logo-picker">
                        <div className="custom-provider-logo-preview" aria-hidden="true">
                          {cp.logoDataUrl ? (
                            <img src={cp.logoDataUrl} alt="" />
                          ) : (
                            <span>Logo</span>
                          )}
                        </div>
                        <label className="btn btn-secondary btn-xs custom-provider-logo-select">
                          {cp.logoDataUrl ? 'Change image' : 'Choose image'}
                          <input
                            type="file"
                            accept="image/png,image/jpeg,image/webp"
                            onChange={(event) => {
                              handleProviderLogoChange(cp.id, event.currentTarget.files?.[0])
                              event.currentTarget.value = ''
                            }}
                          />
                        </label>
                        {cp.logoDataUrl && (
                          <button
                            type="button"
                            className="btn btn-secondary btn-xs"
                            onClick={() => {
                              updateCp({ logoDataUrl: undefined })
                              setProviderLogoErrors(prev => {
                                const next = { ...prev }
                                delete next[cp.id]
                                return next
                              })
                            }}
                          >
                            Remove
                          </button>
                        )}
                        <span className="custom-provider-logo-help">PNG, JPEG, or WebP · max 256 KB</span>
                        {providerLogoErrors[cp.id] && (
                          <span className="custom-provider-logo-error" role="alert">{providerLogoErrors[cp.id]}</span>
                        )}
                      </div>
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
                      <ApiKeyInput
                        value={cp.apiKey}
                        onChange={(value) => updateCp({ apiKey: value })}
                        placeholder="Optional. Leave empty if you manage auth via headers."
                      />
                    </div>

                    <div>
                      <label className="provider-sublist-label">Model list</label>
                      {cp.models.map((m, mIdx) => (
                        <div key={mIdx} className="provider-sublist-row">
                          <input
                            type="text"
                            value={m.modelId}
                            onChange={(e) => updateModel(mIdx, { modelId: e.target.value, displayName: e.target.value })}
                            placeholder="e.g. llama-3.1-8b"
                          />
                          <button
                            className="btn btn-danger btn-xs"
                            onClick={() => deleteModel(mIdx)}
                            title="Remove model"
                          >🗑</button>
                        </div>
                      ))}
                      <button
                        className="btn btn-secondary btn-xs sublist-add"
                        onClick={addModel}
                      >
                        + Add model
                      </button>
                    </div>

                    <div>
                      <label className="provider-sublist-label">Headers (optional)</label>
                      {cp.headers.map((h, hIdx) => (
                        <div key={hIdx} className="provider-sublist-row">
                          <input
                            type="text"
                            value={h.name}
                            onChange={(e) => updateHeader(hIdx, { name: e.target.value })}
                            placeholder="Header-Name"
                          />
                          <input
                            type="text"
                            value={h.value}
                            onChange={(e) => updateHeader(hIdx, { value: e.target.value })}
                            placeholder="value"
                          />
                          <button
                            className="btn btn-danger btn-xs"
                            onClick={() => deleteHeader(hIdx)}
                            title="Remove header"
                          >🗑</button>
                        </div>
                      ))}
                      <button
                        className="btn btn-secondary btn-xs sublist-add"
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

          <div className="setting-actions">
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
        </>,
        <span className="section-current-badge">{customProviderCount > 0 ? `${customProviderCount} added` : 'None'}</span>
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
              <label>Updater endpoint</label>
              <input type="text" value={config.updaterEndpoint} onChange={(e) => updateField('updaterEndpoint', e.target.value)} placeholder="https://updates.example.com/latest.json" />
            </div>
            <div className="setting-row">
              <label>Updater public key</label>
              <textarea value={config.updaterPublicKey} onChange={(e) => updateField('updaterPublicKey', e.target.value)} placeholder="Minisign public key" />
            </div>
          </div>
          <div className="setting-actions">
            <button type="button" className="btn-connect" onClick={handleCheckUpdates}>Check for updates</button>
            {updateAvailable && <button type="button" className="btn-primary" onClick={handleInstallUpdate}>Install signed update</button>}
          </div>
          {updateMessage && <p className="hint">{updateMessage}</p>}
        </>,
        <span className="section-current-badge">{config.enableAutoUpdate ? 'On' : 'Off'}</span>
      )}

      {/* ── Section 8: Assistant Presence ── */}
      {isDesktopApp && renderCollapsibleSection(
        'assistant_presence',
        'Desktop',
        'Assistant Presence',
        'Configure the mini AI character desktop presence widget.',
        <div className="toggle-row">
          <div>
            <label>Show Desktop AI candidate</label>
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
        </div>,
        <span className="section-current-badge">{config.showDesktopWidget ? 'On' : 'Off'}</span>
      )}
    </div>
  )
}
