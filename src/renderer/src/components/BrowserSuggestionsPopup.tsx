import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import './browser-suggestions-popup.css'

const HOME_URL = 'https://www.google.com'

function normalizeUrl(value: string) {
  const trimmed = value.trim()
  if (!trimmed) return HOME_URL
  return /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`
}

function buildSuggestions(query: string) {
  const trimmed = query.trim()
  if (!trimmed) return []

  const suggestions = [
    {
      label: trimmed,
      detail: 'Search the web',
      value: `https://www.google.com/search?q=${encodeURIComponent(trimmed)}`,
      icon: '⌕',
    },
  ]

  if (!/^https?:\/\//i.test(trimmed) && !trimmed.includes('.')) {
    suggestions.push({
      label: `${trimmed}.com`,
      detail: 'Open website',
      value: normalizeUrl(`${trimmed}.com`),
      icon: '◎',
    })
  }

  if (/^https?:\/\//i.test(trimmed) || trimmed.includes('.')) {
    suggestions.push({
      label: trimmed,
      detail: 'Open address',
      value: normalizeUrl(trimmed),
      icon: '◎',
    })
  }

  return suggestions
}

export default function BrowserSuggestionsPopup() {
  const [query, setQuery] = useState('')
  const suggestions = useMemo(() => buildSuggestions(query), [query])

  useEffect(() => {
    void invoke<string>('get_mint_browser_suggestion_query').then(setQuery)
    let unlisten: (() => void) | undefined
    void listen<string>('mint-browser-suggestions-update', (event) => {
      setQuery(event.payload)
    }).then((cleanup) => { unlisten = cleanup })
    return () => unlisten?.()
  }, [])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void invoke('hide_mint_browser_suggestions')
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const selectSuggestion = async (value: string) => {
    await invoke('navigate_mint_browser', { url: value })
    await invoke('hide_mint_browser_suggestions')
  }

  return (
    <main className="browser-suggestions-popup" role="listbox" aria-label="Address suggestions">
      {suggestions.map((suggestion) => (
        <button
          key={suggestion.value}
          type="button"
          role="option"
          className="browser-suggestions-popup-item"
          onClick={() => void selectSuggestion(suggestion.value)}
        >
          <span className="browser-suggestions-popup-icon" aria-hidden="true">{suggestion.icon}</span>
          <span className="browser-suggestions-popup-label">{suggestion.label}</span>
          <span className="browser-suggestions-popup-detail">{suggestion.detail}</span>
        </button>
      ))}
    </main>
  )
}
