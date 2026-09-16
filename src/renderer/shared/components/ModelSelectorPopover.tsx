import React, { useState, useEffect, useRef, useMemo } from 'react'
import { useProviderModels } from '@/hooks/useProviderModels'
import { fetchProviderModels } from '@/tauri'
import { HF_MODELS } from '../constants/models'
import type { CustomProviderConfig } from '../types'

interface ModelSelectorPopoverProps {
  activeProvider: string
  activeModel: string
  availableProviders: string[]
  settingsConfig: any
  dynamicOllamaModels?: string[]
  onSelect: (provider: string, model: string) => void
  disabled?: boolean
}

interface ProviderGroup {
  id: string
  name: string
  icon: React.ReactNode
  models: string[]
}

interface FlattenedItem {
  providerId: string
  providerName: string
  model: string
}

// ─── SVG Icons for Providers (NO EMOJIS) ───────────────────────────────────────

function ProviderIcon({ provider }: { provider: string }) {
  const p = provider.toLowerCase()
  if (p === 'gemini') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
      </svg>
    )
  }
  if (p === 'anthropic') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M4.5 16.5c-1.5 1.26-2.5 3.19-2.5 5.5h20c0-2.31-1-4.24-2.5-5.5" />
        <path d="M12 2L2 22h20L12 2z" />
      </svg>
    )
  }
  if (p === 'openai') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="10" />
        <line x1="12" y1="2" x2="12" y2="22" />
        <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6" />
      </svg>
    )
  }
  if (p === 'deepseek') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="11" cy="11" r="8" />
        <line x1="21" y1="21" x2="16.65" y2="16.65" />
      </svg>
    )
  }
  if (p === 'openrouter') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <polyline points="16 3 21 3 21 8" />
        <line x1="4" y1="20" x2="21" y2="3" />
        <polyline points="21 16 21 21 16 21" />
        <line x1="15" y1="15" x2="21" y2="21" />
        <line x1="4" y1="4" x2="9" y2="9" />
      </svg>
    )
  }
  if (p === 'huggingface') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="10" />
        <path d="M8 14s1.5 2 4 2 4-2 4-2" />
        <line x1="9" y1="9" x2="9.01" y2="9" />
        <line x1="15" y1="9" x2="15.01" y2="9" />
      </svg>
    )
  }
  if (p === 'local_openai') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
        <line x1="8" y1="21" x2="16" y2="21" />
        <line x1="12" y1="17" x2="12" y2="21" />
      </svg>
    )
  }
  if (p === 'ollama') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
      </svg>
    )
  }
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14" />
    </svg>
  )
}

// ─── SVG Capability Badges (NO EMOJIS) ─────────────────────────────────────────

function ModelBadge({ type, label }: { type: 'fast' | 'reasoning' | 'top' | 'vision'; label: string }) {
  return (
    <span className={`model-tag-badge badge-${type}`}>
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

function getModelBadges(modelId: string): Array<{ label: string; type: 'fast' | 'reasoning' | 'top' | 'vision' }> {
  const lower = modelId.toLowerCase()

  // 1. Dedicated Reasoning models
  if (
    lower.includes('reason') ||
    lower.includes('thinking') ||
    lower.includes('r1') ||
    lower.includes('o1') ||
    lower.includes('o3') ||
    lower.includes('qwq')
  ) {
    return [{ label: 'Reasoning', type: 'reasoning' }]
  }

  // 2. Vision / Multimodal models
  if (lower.includes('vision') || lower.includes('multimodal') || lower.includes('-vl')) {
    return [{ label: 'Vision', type: 'vision' }]
  }

  // 3. Fast / Lightweight models
  if (
    lower.includes('flash') ||
    lower.includes('haiku') ||
    lower.includes('mini') ||
    lower.includes('turbo') ||
    lower.includes('lite')
  ) {
    return [{ label: 'Fast', type: 'fast' }]
  }

  // 4. Flagship / Top-tier models
  if (
    lower === 'claude-sonnet-5' ||
    lower === 'claude-opus-5' ||
    lower === 'gemini-2.5-pro' ||
    lower === 'gpt-5.6-luna' ||
    lower === 'gpt-4o' ||
    lower === 'deepseek-chat' ||
    lower === 'deepseek-v4-pro'
  ) {
    return [{ label: 'Top', type: 'top' }]
  }

  return []
}

export default function ModelSelectorPopover({
  activeProvider,
  activeModel,
  availableProviders,
  settingsConfig,
  dynamicOllamaModels,
  onSelect,
  disabled = false,
}: ModelSelectorPopoverProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [search, setSearch] = useState('')
  const [highlightedIndex, setHighlightedIndex] = useState(0)
  const popoverRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  // ─── Live Dynamic Models from Hook ──────────────────────────────────────────
  const { models: dynamicGemini } = useProviderModels('gemini', settingsConfig?.apiKey || '')
  const { models: dynamicClaude } = useProviderModels('anthropic', settingsConfig?.anthropicApiKey || '')
  const { models: dynamicOpenAI } = useProviderModels('openai', settingsConfig?.openaiApiKey || '')
  const { models: dynamicOpenRouter } = useProviderModels('openrouter', settingsConfig?.openrouterApiKey || '')
  const { models: dynamicDeepSeek } = useProviderModels('deepseek', settingsConfig?.deepseekApiKey || '')
  const { models: dynamicLocal } = useProviderModels('local_openai', '', settingsConfig?.localApiBaseUrl || '')

  // ─── Live Dynamic Ollama Models (Self-Fetch Fallback) ───────────────────────
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
        const live = await fetchProviderModels('ollama', '')
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

  // ─── Assemble Provider Groups ───────────────────────────────────────────────
  const providerGroups = useMemo<ProviderGroup[]>(() => {
    const list: ProviderGroup[] = []

    const addGroup = (id: string, name: string, models: string[]) => {
      if (models.length === 0) return
      list.push({
        id,
        name,
        icon: <ProviderIcon provider={id} />,
        models,
      })
    }

    if (availableProviders.includes('gemini')) {
      addGroup('gemini', 'Google Gemini', dynamicGemini)
    }
    if (availableProviders.includes('anthropic')) {
      addGroup('anthropic', 'Anthropic Claude', dynamicClaude)
    }
    if (availableProviders.includes('openai')) {
      addGroup('openai', 'OpenAI', dynamicOpenAI)
    }
    if (availableProviders.includes('deepseek')) {
      addGroup('deepseek', 'DeepSeek', dynamicDeepSeek)
    }
    if (availableProviders.includes('openrouter')) {
      addGroup('openrouter', 'OpenRouter', dynamicOpenRouter)
    }
    if (availableProviders.includes('huggingface')) {
      addGroup('huggingface', 'Hugging Face', [...HF_MODELS])
    }
    if (availableProviders.includes('local_openai')) {
      addGroup('local_openai', 'LM Studio / Local', dynamicLocal)
    }
    if (availableProviders.includes('ollama')) {
      const ollamaList = (dynamicOllamaModels && dynamicOllamaModels.length > 0)
        ? dynamicOllamaModels
        : (internalOllamaModels.length > 0
            ? internalOllamaModels
            : (settingsConfig?.ollamaModel ? [settingsConfig.ollamaModel] : []))
      addGroup('ollama', 'Ollama', ollamaList)
    }

    // Custom providers
    const customProviders: CustomProviderConfig[] = settingsConfig?.customProviders || []
    for (const cp of customProviders) {
      const pid = `custom:${cp.id}`
      if (availableProviders.includes(pid)) {
        const cModels = (cp.models || []).map((m: any) => m.modelId).filter(Boolean)
        addGroup(pid, cp.displayName || cp.id, cModels)
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

  // ─── Filter Groups by Search Query ─────────────────────────────────────────
  const filteredGroups = useMemo(() => {
    const q = search.trim().toLowerCase()
    if (!q) return providerGroups

    const results: ProviderGroup[] = []

    for (const group of providerGroups) {
      const providerMatches = group.name.toLowerCase().includes(q) || group.id.toLowerCase().includes(q)
      if (providerMatches) {
        // Show all models in this provider
        results.push(group)
      } else {
        // Filter models
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

  // ─── Flattened Items for Keyboard Navigation ────────────────────────────────
  const flattenedItems = useMemo<FlattenedItem[]>(() => {
    const items: FlattenedItem[] = []
    for (const g of filteredGroups) {
      for (const m of g.models) {
        items.push({
          providerId: g.id,
          providerName: g.name,
          model: m,
        })
      }
    }
    return items
  }, [filteredGroups])

  // Keep highlighted index in range
  useEffect(() => {
    setHighlightedIndex(0)
  }, [search])

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
      setHighlightedIndex((prev) => (prev + 1) % flattenedItems.length)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (flattenedItems.length === 0) return
      setHighlightedIndex((prev) => (prev - 1 + flattenedItems.length) % flattenedItems.length)
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

  // Active item label for trigger pill
  const activeModelDisplay = useMemo(() => {
    if (!activeModel) return 'Select Model'
    return activeModel.split('/').pop() || activeModel
  }, [activeModel])

  let itemCounter = 0

  return (
    <div className="model-selector-container">
      {/* ─── Trigger Button ─── */}
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
          <ProviderIcon provider={activeProvider} />
        </span>
        <span className="model-selector-trigger-name">{activeModelDisplay}</span>
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

      {/* ─── Spotlight Floating Popover ─── */}
      {isOpen && (
        <div ref={popoverRef} className="model-spotlight-popover" onKeyDown={handleKeyDown}>
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
              placeholder="Search model or provider..."
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
              filteredGroups.map((group) => (
                <div key={group.id} className="model-spotlight-group">
                  <div className="model-spotlight-group-header">
                    <span className="group-header-icon">{group.icon}</span>
                    <span className="group-header-title">{group.name}</span>
                  </div>

                  <div className="model-spotlight-group-items">
                    {group.models.map((m) => {
                      const currentIndex = itemCounter++
                      const isSelected = activeProvider === group.id && activeModel === m
                      const isHighlighted = highlightedIndex === currentIndex
                      const badges = getModelBadges(m)

                      return (
                        <div
                          key={m}
                          data-item-index={currentIndex}
                          className={`model-spotlight-item ${isSelected ? 'is-active' : ''} ${isHighlighted ? 'is-highlighted' : ''}`}
                          onClick={() => {
                            onSelect(group.id, m)
                            setIsOpen(false)
                          }}
                          onMouseEnter={() => setHighlightedIndex(currentIndex)}
                          role="option"
                          aria-selected={isSelected}
                        >
                          <div className="model-item-left">
                            <span className="model-item-name">{m.split('/').pop() || m}</span>
                            {m.includes('/') && <span className="model-item-repo">{m}</span>}
                          </div>

                          <div className="model-item-right">
                            <div className="model-item-badges">
                              {badges.map((b) => (
                                <ModelBadge key={b.type} type={b.type} label={b.label} />
                              ))}
                            </div>

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
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  )
}
