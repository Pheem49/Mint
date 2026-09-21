import React, { useState, useEffect, useRef, useMemo, useCallback } from 'react'
import { useProviderModels } from '@/hooks/useProviderModels'
import { catalogPlatform } from '../platform'
import { HF_MODELS, getModelMetadata, isFreeModel, OPENROUTER_POPULAR_MODELS } from '../constants/models'
import type { CustomProviderConfig } from '../types'
import anthropicLogo from '@lobehub/icons-static-svg/icons/anthropic.svg?url'
import deepseekLogo from '@lobehub/icons-static-svg/icons/deepseek-color.svg?url'
import geminiLogo from '@lobehub/icons-static-svg/icons/gemini-color.svg?url'
import huggingFaceLogo from '@lobehub/icons-static-svg/icons/huggingface-color.svg?url'
import lmStudioLogo from '@lobehub/icons-static-svg/icons/lmstudio.svg?url'
import ollamaLogo from '@lobehub/icons-static-svg/icons/ollama.svg?url'
import openAiLogo from '@lobehub/icons-static-svg/icons/openai.svg?url'
import openRouterLogo from '@lobehub/icons-static-svg/icons/openrouter-color.svg?url'

interface ModelSelectorPopoverProps {
  activeProvider: string
  activeModel: string
  availableProviders: string[]
  settingsConfig: any
  dynamicOllamaModels?: string[]
  onSelect: (provider: string, model: string) => void
  disabled?: boolean
  onUpdateSettings?: (config: any) => void
}

interface ProviderGroup {
  id: string
  providerId: string
  name: string
  icon: React.ReactNode
  models: string[]
}

interface FlattenedItem {
  providerId: string
  providerName: string
  model: string
}

const Effort_LEVELS = [
  { id: 'low', label: 'Low' },
  { id: 'medium', label: 'Medium' },
  { id: 'high', label: 'High' },
  { id: 'extra_high', label: 'Extra High' },
] as const

function formatEffortDisplay(effort?: string): string {
  if (!effort) return 'Medium'
  const lower = effort.toLowerCase().replace(/[- ]/g, '_')
  if (lower === 'low') return 'Low'
  if (lower === 'high') return 'High'
  if (lower === 'extra_high' || lower === 'extra') return 'Extra High'
  return 'Medium'
}

// Brand marks are bundled locally so the selector never depends on a remote image host.
const providerLogos: Record<string, { src: string; color?: string }> = {
  anthropic: { src: anthropicLogo, color: '#d97757' },
  deepseek: { src: deepseekLogo },
  gemini: { src: geminiLogo },
  huggingface: { src: huggingFaceLogo },
  local_openai: { src: lmStudioLogo, color: '#8b5cf6' },
  ollama: { src: ollamaLogo, color: '#f3f4f6' },
  openai: { src: openAiLogo, color: '#10a37f' },
  openrouter: { src: openRouterLogo },
}

function ProviderIcon({ provider }: { provider: string }) {
  const p = provider.toLowerCase()
  const logo = providerLogos[p]
  if (logo) {
    if (logo.color) {
      return (
        <span
          className="provider-brand-logo is-monochrome"
          aria-hidden="true"
          style={{
            '--provider-brand-color': logo.color,
            maskImage: `url("${logo.src}")`,
            WebkitMaskImage: `url("${logo.src}")`,
          } as React.CSSProperties}
        />
      )
    }
    return <img className="provider-brand-logo" src={logo.src} alt="" aria-hidden="true" />
  }
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14" />
    </svg>
  )
}

function ModelBadge({ type, label }: { type: 'fast' | 'reasoning' | 'top' | 'vision' | 'free'; label: string }) {
  return (
    <span className={`model-tag-badge badge-${type}`}>
      {type === 'free' && (
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" />
          <line x1="7" y1="7" x2="7.01" y2="7" />
        </svg>
      )}
      {type === 'fast' && (
        <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
        </svg>
      )}
      {type === 'reasoning' && (
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="4" y="4" width="16" height="16" rx="2" />
          <rect x="9" y="9" width="6" height="6" />
          <line x1="9" y1="1" x2="9" y2="4" />
          <line x1="15" y1="1" x2="15" y2="4" />
          <line x1="9" y1="20" x2="9" y2="23" />
          <line x1="15" y1="20" x2="15" y2="23" />
          <line x1="20" y1="9" x2="23" y2="9" />
          <line x1="20" y1="14" x2="23" y2="14" />
          <line x1="1" y1="9" x2="4" y2="9" />
          <line x1="1" y1="14" x2="4" y2="14" />
        </svg>
      )}
      {type === 'top' && (
        <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
        </svg>
      )}
      {type === 'vision' && (
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="3" />
          <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
        </svg>
      )}
      <span>{label}</span>
    </span>
  )
}

function getModelBadges(modelId: string): Array<{ label: string; type: 'fast' | 'reasoning' | 'top' | 'vision' | 'free' }> {
  const badges: Array<{ label: string; type: 'fast' | 'reasoning' | 'top' | 'vision' | 'free' }> = []

  if (isFreeModel(modelId)) {
    badges.push({ label: 'FREE', type: 'free' })
  }

  const lower = modelId.toLowerCase()

  if (
    lower.includes('reason') ||
    lower.includes('thinking') ||
    lower.includes('r1') ||
    lower.includes('o1') ||
    lower.includes('o3') ||
    lower.includes('qwq')
  ) {
    badges.push({ label: 'Reasoning', type: 'reasoning' })
  }
  if (lower.includes('vision') || lower.includes('multimodal') || lower.includes('-vl')) {
    badges.push({ label: 'Vision', type: 'vision' })
  }
  if (
    lower.includes('flash') ||
    lower.includes('haiku') ||
    lower.includes('mini') ||
    lower.includes('turbo') ||
    lower.includes('lite')
  ) {
    badges.push({ label: 'Fast', type: 'fast' })
  }
  if (
    lower === 'claude-sonnet-5' ||
    lower === 'claude-opus-5' ||
    lower === 'gemini-2.5-pro' ||
    lower === 'gpt-5.6-luna' ||
    lower === 'gpt-4o' ||
    lower === 'deepseek-chat' ||
    lower === 'deepseek-v4-pro'
  ) {
    badges.push({ label: 'Top', type: 'top' })
  }
  return badges
}

export default function ModelSelectorPopover({
  activeProvider,
  activeModel,
  availableProviders,
  settingsConfig,
  dynamicOllamaModels,
  onSelect,
  disabled = false,
  onUpdateSettings,
}: ModelSelectorPopoverProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [search, setSearch] = useState('')
  const [highlightedIndex, setHighlightedIndex] = useState(0)
  const [expandOtherModels, setExpandOtherModels] = useState(false)

  // Local state tracking model thinking overrides for responsive updates
  const [modelThinkingOverrides, setModelThinkingOverrides] = useState<
    Record<string, { enabled?: boolean; effort?: string }>
  >({})

  // Keep overrides in sync when settingsConfig arrives
  useEffect(() => {
    if (settingsConfig?.modelThinkingConfigs) {
      setModelThinkingOverrides(settingsConfig.modelThinkingConfigs)
    }
  }, [settingsConfig?.modelThinkingConfigs])

  const popoverRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  // Dynamic models from hook
  const { models: dynamicGemini } = useProviderModels('gemini', settingsConfig?.apiKey || '')
  const { models: dynamicClaude } = useProviderModels('anthropic', settingsConfig?.anthropicApiKey || '')
  const { models: dynamicOpenAI } = useProviderModels('openai', settingsConfig?.openaiApiKey || '')
  const { models: dynamicOpenRouter } = useProviderModels('openrouter', settingsConfig?.openrouterApiKey || '')
  const { models: dynamicDeepSeek } = useProviderModels('deepseek', settingsConfig?.deepseekApiKey || '')
  const { models: dynamicLocal } = useProviderModels('local_openai', '', settingsConfig?.localApiBaseUrl || '')

  // Ollama self-fetch fallback
  const [internalOllamaModels, setInternalOllamaModels] = useState<string[]>([])

  useEffect(() => {
    if (dynamicOllamaModels && dynamicOllamaModels.length > 0) {
      setInternalOllamaModels(dynamicOllamaModels)
      return
    }
    let cancelled = false
    const fetchOllama = async () => {
      const host = settingsConfig?.ollamaHost || 'http://localhost:11434'
      const cleanHost = host.endsWith('/') ? host.slice(0, -1) : host
      try {
        const controller = new AbortController()
        const timer = setTimeout(() => controller.abort(), 2000)
        const res = await fetch(`${cleanHost}/api/tags`, { signal: controller.signal })
        clearTimeout(timer)
        if (res.ok) {
          const data = await res.json()
          if (!cancelled && data && Array.isArray(data.models) && data.models.length > 0) {
            setInternalOllamaModels(data.models.map((m: any) => m.name))
            return
          }
        }
      } catch {}

      try {
        const live = await catalogPlatform.fetchProviderModels('ollama', '')
        if (!cancelled && live && live.length > 0) {
          setInternalOllamaModels(live)
          return
        }
      } catch {}

      if (!cancelled && settingsConfig?.ollamaModel) {
        setInternalOllamaModels([settingsConfig.ollamaModel])
      }
    }
    fetchOllama()
    return () => {
      cancelled = true
    }
  }, [dynamicOllamaModels, settingsConfig?.ollamaHost, settingsConfig?.ollamaModel])

  // Assemble Provider Groups
  const providerGroups = useMemo<ProviderGroup[]>(() => {
    const list: ProviderGroup[] = []

    const addGroup = (id: string, providerId: string, name: string, models: string[], logoDataUrl?: string) => {
      if (models.length === 0) return
      list.push({
        id,
        providerId,
        name,
        icon: logoDataUrl
          ? <img className="provider-brand-logo" src={logoDataUrl} alt="" aria-hidden="true" />
          : <ProviderIcon provider={providerId} />,
        models,
      })
    }

    if (availableProviders.includes('gemini')) {
      addGroup('gemini', 'gemini', 'Google Gemini', dynamicGemini)
    }
    if (availableProviders.includes('anthropic')) {
      addGroup('anthropic', 'anthropic', 'Anthropic Claude', dynamicClaude)
    }
    if (availableProviders.includes('openai')) {
      addGroup('openai', 'openai', 'OpenAI', dynamicOpenAI)
    }
    if (availableProviders.includes('deepseek')) {
      addGroup('deepseek', 'deepseek', 'DeepSeek', dynamicDeepSeek)
    }
    if (availableProviders.includes('openrouter')) {
      const popularSet = new Set<string>(OPENROUTER_POPULAR_MODELS)
      const popularModels: string[] = []
      for (const pop of OPENROUTER_POPULAR_MODELS) {
        if (dynamicOpenRouter.includes(pop)) {
          popularModels.push(pop)
        }
      }
      const otherModels = dynamicOpenRouter.filter((m) => !popularSet.has(m))

      if (popularModels.length > 0) {
        addGroup('openrouter-popular', 'openrouter', 'OpenRouter • Popular & Recommended', popularModels)
        if (otherModels.length > 0) {
          addGroup('openrouter-other', 'openrouter', `OpenRouter • Other models (${otherModels.length})`, otherModels)
        }
      } else {
        addGroup('openrouter', 'openrouter', 'OpenRouter', dynamicOpenRouter)
      }
    }
    if (availableProviders.includes('huggingface')) {
      addGroup('huggingface', 'huggingface', 'Hugging Face', [...HF_MODELS])
    }
    if (availableProviders.includes('local_openai')) {
      addGroup('local_openai', 'local_openai', 'LM Studio / Local', dynamicLocal)
    }
    if (availableProviders.includes('ollama')) {
      const ollamaList = (dynamicOllamaModels && dynamicOllamaModels.length > 0)
        ? dynamicOllamaModels
        : (internalOllamaModels.length > 0
            ? internalOllamaModels
            : (settingsConfig?.ollamaModel ? [settingsConfig.ollamaModel] : []))
      addGroup('ollama', 'ollama', 'Ollama', ollamaList)
    }

    const customProviders: CustomProviderConfig[] = settingsConfig?.customProviders || []
    for (const cp of customProviders) {
      const pid = `custom:${cp.id}`
      if (availableProviders.includes(pid)) {
        const cModels = (cp.models || []).map((m: any) => m.modelId).filter(Boolean)
        addGroup(pid, pid, cp.displayName || cp.id, cModels, cp.logoDataUrl)
      }
    }

    return list
  }, [
    availableProviders,
    dynamicGemini,
    dynamicClaude,
    dynamicOpenAI,
    dynamicDeepSeek,
    dynamicOpenRouter,
    dynamicLocal,
    dynamicOllamaModels,
    internalOllamaModels,
    settingsConfig?.customProviders,
    settingsConfig?.ollamaModel,
  ])

  // Filter Groups by Search Query
  const filteredGroups = useMemo(() => {
    const q = search.trim().toLowerCase()
    if (!q) return providerGroups

    const results: ProviderGroup[] = []
    for (const group of providerGroups) {
      const providerMatches =
        group.name.toLowerCase().includes(q) ||
        group.id.toLowerCase().includes(q) ||
        group.providerId.toLowerCase().includes(q)
      if (providerMatches) {
        results.push(group)
      } else {
        const matchedModels = group.models.filter((m) => m.toLowerCase().includes(q))
        if (matchedModels.length > 0) {
          results.push({
            ...group,
            models: matchedModels,
          })
        }
      }
    }
    return results
  }, [providerGroups, search])

  // Automatically expand other models when searching, or manual toggle
  const isOtherExpanded = Boolean(search.trim()) || expandOtherModels

  // Flattened Items for Keyboard Navigation
  const flattenedItems = useMemo<FlattenedItem[]>(() => {
    const items: FlattenedItem[] = []
    for (const g of filteredGroups) {
      if (g.id === 'openrouter-other' && !isOtherExpanded) {
        continue
      }
      for (const m of g.models) {
        items.push({
          providerId: g.providerId,
          providerName: g.name,
          model: m,
        })
      }
    }
    return items
  }, [filteredGroups, isOtherExpanded])

  // Model currently inspected in the right detail pane
  const [inspectedItem, setInspectedItem] = useState<{ providerId: string; model: string } | null>(null)

  // Initialize or reset inspected item and collapsed state on open
  useEffect(() => {
    if (isOpen) {
      if (activeModel) {
        setInspectedItem({ providerId: activeProvider, model: activeModel })
      } else if (flattenedItems.length > 0) {
        setInspectedItem({ providerId: flattenedItems[0].providerId, model: flattenedItems[0].model })
      }
    } else {
      setExpandOtherModels(false)
    }
  }, [isOpen, activeProvider, activeModel])

  // Reset highlight on search
  useEffect(() => {
    setHighlightedIndex(0)
    if (flattenedItems.length > 0) {
      setInspectedItem({ providerId: flattenedItems[0].providerId, model: flattenedItems[0].model })
    }
  }, [search])

  // Helper to resolve a model's thinking status
  const getThinkingConfig = useCallback((model: string, provider: string) => {
    const meta = getModelMetadata(model, provider)
    const override = modelThinkingOverrides[model]
    if (override) {
      return {
        supported: meta.supportsThinking,
        enabled: override.enabled ?? true,
        effort: (override.effort || 'medium').toLowerCase(),
      }
    }
    return {
      supported: meta.supportsThinking,
      enabled: meta.supportsThinking ? (settingsConfig?.thinkingEnabled ?? true) : false,
      effort: (settingsConfig?.thinkingEffort || 'medium').toLowerCase(),
    }
  }, [modelThinkingOverrides, settingsConfig?.thinkingEnabled, settingsConfig?.thinkingEffort])

  // Persist thinking change
  const persistThinking = async (model: string, enabled: boolean, effort: string) => {
    const updatedOverrides = {
      ...modelThinkingOverrides,
      [model]: { enabled, effort },
    }
    setModelThinkingOverrides(updatedOverrides)

    const updatedConfig = {
      ...(settingsConfig || {}),
      modelThinkingConfigs: updatedOverrides,
    }

    if (model === activeModel) {
      updatedConfig.thinkingEnabled = enabled
      updatedConfig.thinkingEffort = effort
    }

    if (onUpdateSettings) {
      onUpdateSettings(updatedConfig)
    }

    const api = (window as any).settingsApi
    if (api?.saveSettings) {
      try {
        await api.saveSettings(updatedConfig)
      } catch (err) {
        console.error('Failed to save thinking config:', err)
      }
    }
  }

  // Click Outside to Close
  useEffect(() => {
    if (!isOpen) return

    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node
      if (
        popoverRef.current &&
        !popoverRef.current.contains(target) &&
        triggerRef.current &&
        !triggerRef.current.contains(target)
      ) {
        setIsOpen(false)
      }
    }

    window.addEventListener('mousedown', handleClickOutside)
    return () => window.removeEventListener('mousedown', handleClickOutside)
  }, [isOpen])

  // Auto-focus search on open
  useEffect(() => {
    if (isOpen) {
      setSearch('')
      setTimeout(() => {
        searchInputRef.current?.focus()
      }, 50)
    }
  }, [isOpen])

  // Scroll active item into view
  useEffect(() => {
    if (!isOpen || !listRef.current) return
    const el = listRef.current.querySelector<HTMLElement>(`[data-item-index="${highlightedIndex}"]`)
    if (el) {
      el.scrollIntoView({ block: 'nearest' })
    }
  }, [highlightedIndex, isOpen])

  // Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!isOpen) return

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (flattenedItems.length === 0) return
      const nextIdx = (highlightedIndex + 1) % flattenedItems.length
      setHighlightedIndex(nextIdx)
      setInspectedItem({ providerId: flattenedItems[nextIdx].providerId, model: flattenedItems[nextIdx].model })
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (flattenedItems.length === 0) return
      const nextIdx = (highlightedIndex - 1 + flattenedItems.length) % flattenedItems.length
      setHighlightedIndex(nextIdx)
      setInspectedItem({ providerId: flattenedItems[nextIdx].providerId, model: flattenedItems[nextIdx].model })
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const item = flattenedItems[highlightedIndex]
      if (item) {
        onSelect(item.providerId, item.model)
        setIsOpen(false)
      }
    } else if (e.key === 'Escape') {
      e.preventDefault()
      setIsOpen(false)
      triggerRef.current?.focus()
    }
  }

  // Active item label & effort for trigger pill
  const activeModelDisplay = useMemo(() => {
    if (!activeModel) return 'Select Model'
    return activeModel.split('/').pop() || activeModel
  }, [activeModel])

  const activeThinking = useMemo(() => {
    return getThinkingConfig(activeModel, activeProvider)
  }, [activeModel, activeProvider, getThinkingConfig])

  // Inspected model details
  const currentInspected = inspectedItem || (activeModel ? { providerId: activeProvider, model: activeModel } : null)
  const inspectedMeta = useMemo(() => {
    if (!currentInspected) return null
    return getModelMetadata(currentInspected.model, currentInspected.providerId)
  }, [currentInspected])

  const inspectedThinking = useMemo(() => {
    if (!currentInspected) return { supported: false, enabled: false, effort: 'medium' }
    return getThinkingConfig(currentInspected.model, currentInspected.providerId)
  }, [currentInspected, getThinkingConfig])

  const inspectedBadges = useMemo(() => {
    if (!currentInspected) return []
    return getModelBadges(currentInspected.model)
  }, [currentInspected])

  const customLogoForProvider = (providerId: string) =>
    (settingsConfig?.customProviders as CustomProviderConfig[] | undefined)
      ?.find((provider) => `custom:${provider.id}` === providerId)?.logoDataUrl

  let itemCounter = 0

  return (
    <div className="model-selector-container">
      {/* ─── Trigger Button in Chat Bar ─── */}
      <button
        ref={triggerRef}
        type="button"
        className={`model-selector-trigger ${isOpen ? 'active' : ''}`}
        onClick={() => setIsOpen(!isOpen)}
        disabled={disabled}
        title={`Model: ${activeModel || 'None'} (${activeProvider})`}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
      >
        <span className="model-selector-trigger-icon">
          {customLogoForProvider(activeProvider)
            ? <img className="provider-brand-logo" src={customLogoForProvider(activeProvider)} alt="" aria-hidden="true" />
            : <ProviderIcon provider={activeProvider} />}
        </span>
        <span className="model-selector-trigger-name">{activeModelDisplay}</span>
        {activeThinking.enabled && (
          <span className="model-selector-trigger-effort">
            {formatEffortDisplay(activeThinking.effort)}
          </span>
        )}
        <svg
          className={`model-selector-trigger-chevron ${isOpen ? 'open' : ''}`}
          width="11"
          height="11"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>

      {/* ─── Spotlight 2-Column Master-Detail Floating Popover ─── */}
      {isOpen && (
        <div ref={popoverRef} className="model-spotlight-popover" onKeyDown={handleKeyDown}>
          {/* ─── Left Column: Model List ─── */}
          <div className="model-spotlight-left-pane">
            {/* Search Header */}
            <div className="model-spotlight-search-bar">
              <svg
                className="search-icon"
                width="13"
                height="13"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <circle cx="11" cy="11" r="8" />
                <line x1="21" y1="21" x2="16.65" y2="16.65" />
              </svg>
              <input
                ref={searchInputRef}
                type="text"
                className="model-spotlight-input"
                placeholder="Search models..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
              {search && (
                <button
                  type="button"
                  className="search-clear-btn"
                  onClick={() => setSearch('')}
                  title="Clear search"
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              )}
            </div>

            {/* Grouped Model List */}
            <div ref={listRef} className="model-spotlight-list" role="listbox">
              {filteredGroups.length === 0 ? (
                <div className="model-spotlight-empty">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="11" cy="11" r="8" />
                    <line x1="21" y1="21" x2="16.65" y2="16.65" />
                  </svg>
                  <span>No models found matching &quot;{search}&quot;</span>
                </div>
              ) : (
                filteredGroups.map((group) => {
                  const isOtherGroup = group.id === 'openrouter-other'
                  const isCollapsed = isOtherGroup && !isOtherExpanded

                  return (
                    <div key={group.id} className="model-spotlight-group">
                      <div className="model-spotlight-group-header">
                        <span className="group-header-icon">{group.icon}</span>
                        <span className="group-header-title">{group.name}</span>
                      </div>

                      {isCollapsed ? (
                        <div className="model-spotlight-see-more-wrap">
                          <button
                            type="button"
                            className="model-spotlight-see-more-btn"
                            onClick={() => setExpandOtherModels(true)}
                          >
                            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                              <polyline points="6 9 12 15 18 9" />
                            </svg>
                            <span>Show all {group.models.length} other models</span>
                          </button>
                        </div>
                      ) : (
                        <>
                          <div className="model-spotlight-group-items">
                            {group.models.map((m) => {
                              const currentIndex = itemCounter++
                              const isSelected = activeProvider === group.providerId && activeModel === m
                              const isInspected = currentInspected?.model === m && currentInspected?.providerId === group.providerId
                              const itemThinking = getThinkingConfig(m, group.providerId)

                              return (
                                <div
                                  key={m}
                                  data-item-index={currentIndex}
                                  className={`model-spotlight-item ${isSelected ? 'is-active' : ''} ${isInspected ? 'is-inspected' : ''}`}
                                  onClick={() => {
                                    onSelect(group.providerId, m)
                                  }}
                                  onMouseEnter={() => {
                                    setHighlightedIndex(currentIndex)
                                    setInspectedItem({ providerId: group.providerId, model: m })
                                  }}
                                  role="option"
                                  aria-selected={isSelected}
                                >
                                  <div className="model-item-left">
                                    <div style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
                                      <span className="model-item-provider-icon" aria-hidden="true">
                                        {group.icon}
                                      </span>
                                      <span className="model-item-name">{m.split('/').pop() || m}</span>
                                      {isFreeModel(m) && (
                                        <span className="model-tag-badge badge-free">Free</span>
                                      )}
                                    </div>
                                    {m.includes('/') && <span className="model-item-repo">{m}</span>}
                                  </div>

                                  <div className="model-item-right">
                                    {itemThinking.enabled && (
                                      <span className="model-item-effort-pill">
                                        {formatEffortDisplay(itemThinking.effort)}
                                      </span>
                                    )}

                                    {isSelected && (
                                      <svg
                                        className="model-item-check"
                                        width="14"
                                        height="14"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="var(--accent, #10b981)"
                                        strokeWidth="2.5"
                                        strokeLinecap="round"
                                        strokeLinejoin="round"
                                      >
                                        <polyline points="20 6 9 17 4 12" />
                                      </svg>
                                    )}
                                  </div>
                                </div>
                              )
                            })}
                          </div>

                          {isOtherGroup && !search.trim() && (
                            <div className="model-spotlight-see-more-wrap" style={{ marginTop: '4px' }}>
                              <button
                                type="button"
                                className="model-spotlight-see-more-btn collapse-btn"
                                onClick={() => setExpandOtherModels(false)}
                              >
                                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                                  <polyline points="18 15 12 9 6 15" />
                                </svg>
                                <span>Collapse other models</span>
                              </button>
                            </div>
                          )}
                        </>
                      )}
                    </div>
                  )
                })
              )}
            </div>
          </div>

          {/* ─── Right Column: Model Detail & Thinking Controls ─── */}
          <div className="model-spotlight-right-pane">
            {currentInspected && inspectedMeta ? (
              <div className="model-detail-container">
                {/* Header: Icon + Name */}
                <div className="model-detail-header">
                  <div className="model-detail-icon">
                    {customLogoForProvider(currentInspected.providerId)
                      ? <img className="provider-brand-logo" src={customLogoForProvider(currentInspected.providerId)} alt="" aria-hidden="true" />
                      : <ProviderIcon provider={currentInspected.providerId} />}
                  </div>
                  <div className="model-detail-title-wrap">
                    <span className="model-detail-title">{currentInspected.model.split('/').pop() || currentInspected.model}</span>
                    {currentInspected.model.includes('/') && (
                      <span className="model-detail-subtitle">{currentInspected.model}</span>
                    )}
                  </div>
                </div>

                {/* Description */}
                <p className="model-detail-description">
                  {inspectedMeta.description}
                </p>

                {/* Badges / Capabilities */}
                <div className="model-detail-tags">
                  <span className="model-detail-context-badge">
                    {inspectedMeta.contextWindow}
                  </span>
                  {inspectedBadges.map((b) => (
                    <ModelBadge key={b.type} type={b.type} label={b.label} />
                  ))}
                </div>

                {/* Divider */}
                <div className="model-detail-divider" />

                {/* Options Section */}
                <div className="model-detail-section-title">Options</div>
                <div className="model-detail-option-row">
                  <div className="option-row-left">
                    <svg
                      width="15"
                      height="15"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      className="option-icon"
                    >
                      <path d="M12 2a7 7 0 0 1 7 7c0 2.38-1.19 4.47-3 5.74V17a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2v-2.26C6.19 13.47 5 11.38 5 9a7 7 0 0 1 7-7z" />
                      <path d="M9 21h6" />
                    </svg>
                    <span className="option-label">Thinking</span>
                  </div>

                  <button
                    type="button"
                    role="switch"
                    aria-checked={inspectedThinking.enabled}
                    className={`model-detail-switch ${inspectedThinking.enabled ? 'checked' : ''}`}
                    onClick={() => {
                      persistThinking(
                        currentInspected.model,
                        !inspectedThinking.enabled,
                        inspectedThinking.effort
                      )
                    }}
                    title={
                      inspectedThinking.enabled
                        ? 'Disable thinking'
                        : 'Enable thinking'
                    }
                  >
                    <span className="model-detail-switch-thumb" />
                  </button>
                </div>

                {!inspectedThinking.supported && (
                  <div className="model-unsupported-note" style={{ color: inspectedThinking.enabled ? 'var(--accent, #10b981)' : undefined }}>
                    {inspectedThinking.enabled
                      ? 'Thinking enabled (force toggle)'
                      : 'Not flagged as reasoning model by default (toggle to force enable)'}
                  </div>
                )}

                {/* Effort Section (visible when Thinking is ON) */}
                {inspectedThinking.enabled && (
                  <div className="model-effort-section">
                    <div className="model-detail-section-title">Effort</div>
                    <div className="model-effort-list">
                      {Effort_LEVELS.map((level) => {
                        const isCurrent = inspectedThinking.effort === level.id
                        return (
                          <button
                            key={level.id}
                            type="button"
                            className={`model-effort-item ${isCurrent ? 'is-selected' : ''}`}
                            onClick={() => {
                              persistThinking(currentInspected.model, true, level.id)
                            }}
                          >
                            <span className="effort-item-label">{level.label}</span>
                            {isCurrent && (
                              <svg
                                className="effort-item-check"
                                width="14"
                                height="14"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="var(--accent, #10b981)"
                                strokeWidth="2.5"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <polyline points="20 6 9 17 4 12" />
                              </svg>
                            )}
                          </button>
                        )
                      })}
                    </div>
                  </div>
                )}
              </div>
            ) : (
              <div className="model-detail-empty">
                <span>Select a model to view options</span>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
