import React from 'react'
import { ErrorRecovery } from './ui/error-recovery'

interface State {
  hasError: boolean
}

/** Catches failed route chunks and keeps recovery/window controls available. */
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

          <ErrorRecovery onBackToChat={this.handleReset} onRefresh={this.handleReload} />
        </div>
      )
    }
    return this.props.children
  }
}
