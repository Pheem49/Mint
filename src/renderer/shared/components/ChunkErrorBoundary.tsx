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

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 20,
            height: '100vh',
            width: '100vw',
            background: '#09090b',
            color: '#e4e4e7',
            fontFamily: 'Outfit, Inter, ui-sans-serif, system-ui, sans-serif',
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
            <button
              type="button"
              onClick={this.handleReload}
              style={{
                marginTop: 6,
                padding: '10px 24px',
                borderRadius: 12,
                border: 'none',
                background: '#10b981',
                color: '#06120c',
                fontWeight: 650,
                fontSize: '0.9rem',
                cursor: 'pointer',
                boxShadow: '0 4px 14px rgba(16, 185, 129, 0.25)',
                transition: 'background 0.15s ease, transform 0.1s ease',
              }}
            >
              Refresh &amp; Update
            </button>
          </div>
        </div>
      )
    }
    return this.props.children
  }
}
