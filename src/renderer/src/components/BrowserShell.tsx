import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './browser-shell.css'

const HOME_URL = 'https://www.google.com'

function normalizeUrl(value: string) {
  const trimmed = value.trim()
  if (!trimmed) return HOME_URL
  return /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`
}

function pageLabel(value: string) {
  try {
    return new URL(value).hostname.replace(/^www\./, '') || 'New tab'
  } catch {
    return 'New tab'
  }
}

export default function BrowserShell() {
  const [address, setAddress] = useState(HOME_URL)
  const [draft, setDraft] = useState(HOME_URL)
  const [addressFocused, setAddressFocused] = useState(false)

  const setAddressFocus = (focused: boolean) => {
    setAddressFocused(focused)
    if (focused && draft.trim()) {
      void invoke('show_mint_browser_suggestions', { query: draft })
    } else {
      window.setTimeout(() => void invoke('hide_mint_browser_suggestions'), 220)
    }
  }

  useEffect(() => {
    let unlisten: (() => void) | undefined
    void listen<string>('mint-browser-address', (event) => {
      setAddress(event.payload)
      setDraft(event.payload)
    }).then((cleanup) => { unlisten = cleanup })
    return () => unlisten?.()
  }, [])

  const navigate = async (value = draft) => {
    const next = normalizeUrl(value)
    setAddress(next)
    setDraft(next)
    setAddressFocused(false)
    await invoke('hide_mint_browser_suggestions')
    await invoke('navigate_mint_browser', { url: next })
  }

  const label = pageLabel(address)

  return (
    <main className="mint-browser-shell">
      <div className="mint-browser-tabs" role="tablist" aria-label="Browser tabs">
        <div className="mint-browser-tab" role="tab" aria-selected="true">
          <span className="mint-browser-tab-icon" aria-hidden="true">●</span>
          <span className="mint-browser-tab-title">{label}</span>
          <button type="button" className="mint-browser-tab-close" onClick={() => void getCurrentWindow().close()} aria-label="Close tab" title="Close browser">×</button>
        </div>
        <button type="button" className="mint-browser-new-tab" onClick={() => void navigate(HOME_URL)} aria-label="New tab" title="New tab">+</button>
      </div>

      <form className="mint-browser-toolbar" onSubmit={(event) => { event.preventDefault(); void navigate() }}>
        <div className="mint-browser-nav-group">
          <button type="button" className="mint-browser-nav-button" onClick={() => void invoke('browser_go_back')} aria-label="Go back" title="Back">‹</button>
          <button type="button" className="mint-browser-nav-button" onClick={() => void invoke('browser_go_forward')} aria-label="Go forward" title="Forward">›</button>
          <button type="button" className="mint-browser-nav-button" onClick={() => void navigate(HOME_URL)} aria-label="Home" title="Home">⌂</button>
        </div>
        <div className={`mint-browser-address-wrap${addressFocused ? ' is-open' : ''}`}>
          <div className="mint-browser-address-input">
          <span className="mint-browser-address-icon" aria-hidden="true">◎</span>
          <input
            value={draft}
            onChange={(event) => {
              const value = event.target.value
              setDraft(value)
              if (value.trim()) void invoke('show_mint_browser_suggestions', { query: value })
              else void invoke('hide_mint_browser_suggestions')
            }}
            onFocus={() => setAddressFocus(true)}
            onBlur={() => setAddressFocus(false)}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                setAddressFocused(false)
                void invoke('hide_mint_browser_suggestions')
                event.currentTarget.blur()
              }
            }}
            aria-label="Web address"
            aria-expanded={addressFocused}
            aria-autocomplete="list"
            spellCheck={false}
          />
          <button type="submit" aria-label="Open address" title="Open address">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h13M13 6l6 6-6 6" /></svg>
          </button>
          </div>
        </div>
        <button type="button" className="mint-browser-nav-button" onClick={() => void invoke('browser_reload')} aria-label="Reload" title="Reload">↻</button>
      </form>
    </main>
  )
}
