import React from 'react'

interface State {
  hasError: boolean
}

/**
 * Catches errors from lazy-loaded route chunks (e.g. a stale cached page
 * trying to fetch a hashed JS file that no longer exists after a rebuild)
 * and offers a reload instead of leaving the app permanently blank.
 *
 * Deliberately styled with plain inline styles instead of the app's themed
 * CSS: this screen exists for the case where something in the app's own
 * loading chain already failed, so it shouldn't gamble on `--bg-color` /
 * `--accent` etc. having made it in — a fixed white background reads
 * correctly no matter what broke.
 */
type Props = { children: React.ReactNode }

export default class ChunkErrorBoundary extends React.Component<Props, State> {
  state: State = { hasError: false }
  declare props: Props

  static getDerivedStateFromError(): State {
    return { hasError: true }
  }

  componentDidCatch(error: unknown) {
    console.error('Mint failed to load:', error)
  }

  private handleReload = async () => {
    try {
      if (typeof window !== 'undefined') {
        window.sessionStorage.removeItem('mint_chunk_reload_attempted')
        window.sessionStorage.removeItem('mint_vite_preload_reload')
        if ('caches' in window) {
          const keys = await caches.keys().catch(() => [])
          await Promise.all(keys.map((k) => caches.delete(k))).catch(() => {})
        }
        if ('serviceWorker' in navigator) {
          const regs = await navigator.serviceWorker.getRegistrations().catch(() => [])
          await Promise.all(regs.map((r) => r.unregister())).catch(() => {})
        }
      }
    } finally {
      window.location.reload()
    }
  }

  private handleCloseWindow = () => {
    try {
      const api = typeof window !== 'undefined' ? (window as any).api : null
      if (api?.closeWindow) {
        api.closeWindow()
        return
      }
    } catch {}
    if (typeof window !== 'undefined') window.close()
  }

  private handleMinimize = () => {
    try {
      const api = typeof window !== 'undefined' ? (window as any).api : null
      api?.minimizeWindow?.()
    } catch {}
  }

  private handleToggleMaximize = () => {
    try {
      const api = typeof window !== 'undefined' ? (window as any).api : null
      api?.toggleMaximize?.()
    } catch {}
  }

  private handleReset = () => {
    this.setState({ hasError: false })
    if (typeof window !== 'undefined') {
      window.location.hash = '#/'
    }
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            height: '100vh',
            width: '100vw',
            background: '#09090b',
            color: '#e4e4e7',
            fontFamily: 'Outfit, Inter, ui-sans-serif, system-ui, sans-serif',
            boxSizing: 'border-box',
            overflow: 'hidden',
          }}
        >
          {/* Custom titlebar so desktop user can drag window, minimize, or close */}
          <div
            data-tauri-drag-region
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              height: 38,
              padding: '0 12px',
              background: '#141416',
              borderBottom: '1px solid rgba(255, 255, 255, 0.08)',
              userSelect: 'none',
              WebkitAppRegion: 'drag',
              flexShrink: 0,
            } as React.CSSProperties}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: '0.8rem', color: '#9ca3af' }}>
              <span style={{ color: '#10b981', fontWeight: 700 }}>✦</span> Mint Agent
            </div>
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 2,
                WebkitAppRegion: 'no-drag',
              } as React.CSSProperties}
            >
              <button
                type="button"
                aria-label="Minimize window"
                title="Minimize"
                onClick={this.handleMinimize}
                style={{
                  width: 34,
                  height: 28,
                  background: 'transparent',
                  border: 'none',
                  color: '#9ca3af',
                  cursor: 'pointer',
                  borderRadius: 4,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M2 6h8"/></svg>
              </button>
              <button
                type="button"
                aria-label="Maximize window"
                title="Maximize"
                onClick={this.handleToggleMaximize}
                style={{
                  width: 34,
                  height: 28,
                  background: 'transparent',
                  border: 'none',
                  color: '#9ca3af',
                  cursor: 'pointer',
                  borderRadius: 4,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="2.5" y="2.5" width="7" height="7" rx="0.8"/></svg>
              </button>
              <button
                type="button"
                aria-label="Close window"
                title="Close"
                onClick={this.handleCloseWindow}
                style={{
                  width: 34,
                  height: 28,
                  background: 'transparent',
                  border: 'none',
                  color: '#9ca3af',
                  cursor: 'pointer',
                  borderRadius: 4,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="m3 3 6 6m0-6L3 9"/></svg>
              </button>
            </div>
          </div>

          <div
            style={{
              flex: 1,
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              padding: 24,
              boxSizing: 'border-box',
            }}
          >
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                maxWidth: 420,
                textAlign: 'center',
                padding: '32px 28px',
                borderRadius: 20,
                background: '#18181b',
                border: '1px solid rgba(255, 255, 255, 0.08)',
                boxShadow: '0 20px 40px rgba(0, 0, 0, 0.6)',
                gap: 16,
              }}
            >
              <div
                style={{
                  width: 48,
                  height: 48,
                  borderRadius: 14,
                  background: 'rgba(16, 185, 129, 0.12)',
                  border: '1px solid rgba(16, 185, 129, 0.25)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: '#10b981',
                  fontSize: '1.4rem',
                  fontWeight: 700,
                }}
              >
                ✦
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                <h2 style={{ margin: 0, fontSize: '1.15rem', fontWeight: 650, color: '#f4f4f5' }}>
                  Mint needs to refresh
                </h2>
                <span style={{ fontSize: '0.88rem', color: '#a1a1aa', lineHeight: 1.5 }}>
                  An asset failed to load or a new version was deployed. Refreshing will load the latest version.
                </span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 8 }}>
                <button
                  type="button"
                  onClick={this.handleReset}
                  style={{
                    padding: '10px 18px',
                    borderRadius: 12,
                    border: '1px solid rgba(255, 255, 255, 0.12)',
                    background: 'rgba(255, 255, 255, 0.06)',
                    color: '#e4e4e7',
                    fontWeight: 600,
                    fontSize: '0.88rem',
                    cursor: 'pointer',
                    transition: 'background 0.15s ease',
                  }}
                >
                  Back to Chat
                </button>
                <button
                  type="button"
                  onClick={this.handleReload}
                  style={{
                    padding: '10px 22px',
                    borderRadius: 12,
                    border: 'none',
                    background: '#10b981',
                    color: '#06120c',
                    fontWeight: 650,
                    fontSize: '0.88rem',
                    cursor: 'pointer',
                    boxShadow: '0 4px 14px rgba(16, 185, 129, 0.25)',
                    transition: 'background 0.15s ease, transform 0.1s ease',
                  }}
                >
                  Refresh &amp; Update
                </button>
              </div>
            </div>
          </div>
        </div>
      )
    }
    return this.props.children
  }
}
