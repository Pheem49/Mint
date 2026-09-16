import React from 'react'

interface Props {
  children: React.ReactNode
  onClose?: () => void
  title?: string
}

interface State {
  hasError: boolean
  error?: unknown
}

function isChunkLoadError(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err || '')
  const name = err instanceof Error ? err.name : ''
  return (
    name === 'ChunkLoadError' ||
    /Failed to fetch dynamically imported module/i.test(msg) ||
    /Loading chunk .* failed/i.test(msg) ||
    /error loading dynamically imported module/i.test(msg) ||
    /Failed to load module script/i.test(msg)
  )
}

export default class ModalErrorBoundary extends React.Component<Props, State> {
  state: State = { hasError: false }

  static getDerivedStateFromError(error: unknown): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: unknown) {
    console.error('Modal component failed to render:', error)
  }

  handleRetry = () => {
    if (isChunkLoadError(this.state.error)) {
      window.location.reload()
    } else {
      this.setState({ hasError: false, error: undefined })
    }
  }

  render() {
    if (this.state.hasError) {
      const isChunk = isChunkLoadError(this.state.error)
      const errorMsg = this.state.error instanceof Error ? this.state.error.message : String(this.state.error || '')

      return (
        <div
          style={{
            padding: '32px 24px',
            textAlign: 'center',
            color: 'var(--text-main, #e4e4e7)',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 16,
            minHeight: 240,
            width: '100%',
          }}
        >
          <div style={{ fontSize: '1.05rem', fontWeight: 600 }}>
            {isChunk ? 'App update detected' : (this.props.title || 'Could not load settings')}
          </div>
          <div style={{ fontSize: '0.875rem', color: 'var(--text-muted, #a1a1aa)', maxWidth: 420 }}>
            {isChunk
              ? 'A newer version of the application was built or loaded. Please reload the page to apply the update.'
              : 'A component failed to render. You can try refreshing the view or closing the modal.'}
          </div>
          {errorMsg && !isChunk && (
            <div
              style={{
                fontSize: '0.75rem',
                color: '#ef4444',
                background: 'rgba(239, 68, 68, 0.1)',
                padding: '6px 12px',
                borderRadius: 6,
                maxWidth: 460,
                wordBreak: 'break-word',
              }}
            >
              {errorMsg}
            </div>
          )}
          <div style={{ display: 'flex', gap: 12, marginTop: 8 }}>
            <button
              type="button"
              onClick={this.handleRetry}
              style={{
                padding: '8px 20px',
                borderRadius: 8,
                border: 'none',
                background: '#10b981',
                color: '#ffffff',
                fontWeight: 600,
                cursor: 'pointer',
                fontSize: '0.9rem',
              }}
            >
              {isChunk ? 'Reload Page' : 'Retry'}
            </button>
            <button
              type="button"
              onClick={() => this.props.onClose?.()}
              style={{
                padding: '8px 20px',
                borderRadius: 8,
                border: '1px solid rgba(255, 255, 255, 0.15)',
                background: 'transparent',
                color: 'var(--text-main, #e4e4e7)',
                cursor: 'pointer',
                fontSize: '0.9rem',
              }}
            >
              Close
            </button>
          </div>
        </div>
      )
    }
    return this.props.children
  }
}

