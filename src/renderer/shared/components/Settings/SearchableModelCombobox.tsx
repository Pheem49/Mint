import React, { useState, useEffect, useRef, useMemo } from 'react'
import { OPENROUTER_POPULAR_MODELS, isFreeModel } from '../../constants/models'

export interface SearchableModelComboboxProps {
  value: string
  models: string[]
  onChange: (model: string) => void
  onCustomChange?: (customModel: string) => void
  customValue?: string
  placeholder?: string
}

export default function SearchableModelCombobox({
  value,
  models,
  onChange,
  onCustomChange,
  customValue,
  placeholder = 'Select or search OpenRouter model...',
}: SearchableModelComboboxProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [search, setSearch] = useState('')
  const [highlightedIndex, setHighlightedIndex] = useState(0)
  const [expandOther, setExpandOther] = useState(false)

  const containerRef = useRef<HTMLDivElement>(null)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  // Focus search input on open
  useEffect(() => {
    if (isOpen) {
      setSearch('')
      setHighlightedIndex(0)
      setTimeout(() => {
        searchInputRef.current?.focus()
      }, 50)
    }
  }, [isOpen])

  // Close on click outside & reset expand on close
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false)
      }
    }
    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside)
    } else {
      setExpandOther(false)
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [isOpen])

  // Partition models into Popular & Other
  const { popularList, otherList } = useMemo(() => {
    const popularSet = new Set<string>(OPENROUTER_POPULAR_MODELS)
    const popular: string[] = []
    for (const pop of OPENROUTER_POPULAR_MODELS) {
      if (models.includes(pop)) {
        popular.push(pop)
      }
    }
    const other = models.filter((m) => !popularSet.has(m))
    return { popularList: popular, otherList: other }
  }, [models])

  // Filter with search query
  const { filteredPopular, filteredOther } = useMemo(() => {
    const q = search.trim().toLowerCase()
    if (!q) {
      return { filteredPopular: popularList, filteredOther: otherList }
    }
    return {
      filteredPopular: popularList.filter((m) => m.toLowerCase().includes(q)),
      filteredOther: otherList.filter((m) => m.toLowerCase().includes(q)),
    }
  }, [popularList, otherList, search])

  // Auto expand when searching or manual toggle
  const isOtherExpanded = Boolean(search.trim()) || expandOther

  // Flatten for keyboard navigation
  const flatItems = useMemo(() => {
    const items: Array<{ type: 'model' | 'custom'; value: string; label: string }> = []
    for (const m of filteredPopular) {
      items.push({ type: 'model', value: m, label: m })
    }
    if (isOtherExpanded) {
      for (const m of filteredOther) {
        items.push({ type: 'model', value: m, label: m })
      }
    }
    items.push({ type: 'custom', value: 'custom', label: 'Custom Model...' })
    return items
  }, [filteredPopular, filteredOther, isOtherExpanded])

  // Scroll highlighted item into view
  useEffect(() => {
    if (!isOpen || !listRef.current) return
    const el = listRef.current.querySelector(`[data-combobox-idx="${highlightedIndex}"]`) as HTMLElement | null
    if (el) {
      el.scrollIntoView({ block: 'nearest' })
    }
  }, [highlightedIndex, isOpen])

  const handleSelect = (val: string) => {
    onChange(val)
    setIsOpen(false)
  }

  const handleCustomQuery = () => {
    const trimmed = search.trim()
    if (!trimmed) return
    if (onCustomChange) {
      onCustomChange(trimmed)
    }
    onChange('custom')
    setIsOpen(false)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!isOpen) {
      if (e.key === 'Enter' || e.key === 'ArrowDown' || e.key === ' ') {
        e.preventDefault()
        setIsOpen(true)
      }
      return
    }

    if (e.key === 'Escape') {
      e.preventDefault()
      setIsOpen(false)
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      setHighlightedIndex((prev) => (prev + 1 < flatItems.length ? prev + 1 : 0))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setHighlightedIndex((prev) => (prev - 1 >= 0 ? prev - 1 : flatItems.length - 1))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const item = flatItems[highlightedIndex]
      if (item) {
        handleSelect(item.value)
      }
    }
  }

  // Display text for closed trigger
  const displayLabel = useMemo(() => {
    if (value === 'custom') {
      return customValue ? `Custom: ${customValue}` : 'Custom Model...'
    }
    if (value) {
      return value
    }
    return placeholder
  }, [value, customValue, placeholder])

  const isCurrentFree = isFreeModel(value === 'custom' ? customValue || '' : value)

  let itemIdxCounter = 0

  return (
    <div className="model-combobox-container" ref={containerRef}>
      {/* Trigger Button */}
      <button
        type="button"
        className={`model-combobox-trigger ${isOpen ? 'is-open' : ''}`}
        onClick={() => setIsOpen(!isOpen)}
        onKeyDown={handleKeyDown}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
      >
        <div className="model-combobox-trigger-content">
          <span className="model-combobox-trigger-text">{displayLabel}</span>
          {isCurrentFree && (
            <span className="model-tag-badge badge-free">Free</span>
          )}
        </div>
        <svg
          className={`model-combobox-chevron ${isOpen ? 'is-open' : ''}`}
          width="14"
          height="14"
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

      {/* Dropdown Popover */}
      {isOpen && (
        <div className="model-combobox-dropdown" onKeyDown={handleKeyDown}>
          {/* Sticky Search Header */}
          <div className="model-combobox-search-box">
            <svg
              className="combobox-search-icon"
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
              className="model-combobox-search-input"
              placeholder="Search 300+ models or type to filter..."
              value={search}
              onChange={(e) => {
                setSearch(e.target.value)
                setHighlightedIndex(0)
              }}
            />
            {search && (
              <button
                type="button"
                className="combobox-clear-btn"
                onClick={() => {
                  setSearch('')
                  setHighlightedIndex(0)
                }}
                title="Clear"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="18" y1="6" x2="6" y2="18" />
                  <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            )}
          </div>

          {/* List Area */}
          <div className="model-combobox-list" ref={listRef} role="listbox">
            {/* Quick Custom Option if search doesn't exactly match */}
            {search.trim() && !models.includes(search.trim()) && (
              <div
                className="model-combobox-item custom-query-item"
                onClick={handleCustomQuery}
                role="option"
                aria-selected={false}
              >
                <div className="combobox-item-left">
                  <span className="combobox-item-name">Use custom model: <strong>{search.trim()}</strong></span>
                </div>
                <span className="combobox-custom-badge">Enter</span>
              </div>
            )}

            {/* Popular & Recommended Group */}
            {filteredPopular.length > 0 && (
              <div className="model-combobox-group">
                <div className="model-combobox-group-title">
                  <span>★ Popular & Recommended</span>
                </div>
                {filteredPopular.map((m) => {
                  const currIdx = itemIdxCounter++
                  const isSelected = value === m
                  const isHighlighted = highlightedIndex === currIdx
                  const free = isFreeModel(m)
                  return (
                    <div
                      key={m}
                      data-combobox-idx={currIdx}
                      className={`model-combobox-item ${isSelected ? 'is-selected' : ''} ${isHighlighted ? 'is-highlighted' : ''}`}
                      onClick={() => handleSelect(m)}
                      onMouseEnter={() => setHighlightedIndex(currIdx)}
                      role="option"
                      aria-selected={isSelected}
                    >
                      <div className="combobox-item-left">
                        <div className="combobox-item-row">
                          <span className="combobox-item-name">{m.split('/').pop() || m}</span>
                          {free && <span className="model-tag-badge badge-free">Free</span>}
                        </div>
                        {m.includes('/') && <span className="combobox-item-repo">{m}</span>}
                      </div>
                      {isSelected && (
                        <svg className="combobox-check-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                          <polyline points="20 6 9 17 4 12" />
                        </svg>
                      )}
                    </div>
                  )
                })}
              </div>
            )}

            {/* Other models Group */}
            {filteredOther.length > 0 && (
              <div className="model-combobox-group">
                <div className="model-combobox-group-title">
                  <span>Other models ({filteredOther.length})</span>
                </div>
                {!isOtherExpanded ? (
                  <div className="combobox-see-more-wrap">
                    <button
                      type="button"
                      className="combobox-see-more-btn"
                      onClick={() => setExpandOther(true)}
                    >
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                        <polyline points="6 9 12 15 18 9" />
                      </svg>
                      <span>Show all {filteredOther.length} other models</span>
                    </button>
                  </div>
                ) : (
                  <>
                    {filteredOther.map((m) => {
                      const currIdx = itemIdxCounter++
                      const isSelected = value === m
                      const isHighlighted = highlightedIndex === currIdx
                      const free = isFreeModel(m)
                      return (
                        <div
                          key={m}
                          data-combobox-idx={currIdx}
                          className={`model-combobox-item ${isSelected ? 'is-selected' : ''} ${isHighlighted ? 'is-highlighted' : ''}`}
                          onClick={() => handleSelect(m)}
                          onMouseEnter={() => setHighlightedIndex(currIdx)}
                          role="option"
                          aria-selected={isSelected}
                        >
                          <div className="combobox-item-left">
                            <div className="combobox-item-row">
                              <span className="combobox-item-name">{m.split('/').pop() || m}</span>
                              {free && <span className="model-tag-badge badge-free">Free</span>}
                            </div>
                            {m.includes('/') && <span className="combobox-item-repo">{m}</span>}
                          </div>
                          {isSelected && (
                            <svg className="combobox-check-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                              <polyline points="20 6 9 17 4 12" />
                            </svg>
                          )}
                        </div>
                      )
                    })}
                    {!search.trim() && (
                      <div className="combobox-see-more-wrap" style={{ marginTop: '4px' }}>
                        <button
                          type="button"
                          className="combobox-see-more-btn collapse-btn"
                          onClick={() => setExpandOther(false)}
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
            )}

            {/* Empty state */}
            {filteredPopular.length === 0 && filteredOther.length === 0 && !search.trim() && (
              <div className="model-combobox-empty">
                <span>No models available</span>
              </div>
            )}

            {/* Custom Model Option at bottom */}
            {(() => {
              const currIdx = itemIdxCounter++
              const isSelected = value === 'custom'
              const isHighlighted = highlightedIndex === currIdx
              return (
                <div
                  data-combobox-idx={currIdx}
                  className={`model-combobox-item custom-option-item ${isSelected ? 'is-selected' : ''} ${isHighlighted ? 'is-highlighted' : ''}`}
                  onClick={() => handleSelect('custom')}
                  onMouseEnter={() => setHighlightedIndex(currIdx)}
                  role="option"
                  aria-selected={isSelected}
                >
                  <div className="combobox-item-left">
                    <span className="combobox-item-name">Custom...</span>
                    <span className="combobox-item-repo">Enter a custom model identifier manually</span>
                  </div>
                  {isSelected && (
                    <svg className="combobox-check-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  )}
                </div>
              )
            })()}
          </div>
        </div>
      )}
    </div>
  )
}
