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

export default class ModalErrorBoundary extends React.Component<Props, State> {
  state: State = { hasError: false }

  static getDerivedStateFromError(error: unknown): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: unknown) {
    console.error('Modal component failed to render:', error)
  }

  render() {
    if (this.state.hasError) {
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
            {this.props.title || 'Could not load settings'}
          </div>
          <div style={{ fontSize: '0.875rem', color: 'var(--text-muted, #a1a1aa)', maxWidth: 400 }}>
            A component failed to render. You can try refreshing the view or closing the modal.
          </div>
          <div style={{ display: 'flex', gap: 12, marginTop: 8 }}>
            <button
              type="button"
              onClick={() => this.setState({ hasError: false })}
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
              Retry
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
