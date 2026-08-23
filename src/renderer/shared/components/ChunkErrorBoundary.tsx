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

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 16,
            height: '100vh',
            width: '100vw',
            background: '#ffffff',
            color: '#3f3f46',
            fontFamily: 'Inter, ui-sans-serif, system-ui, sans-serif',
            fontSize: '0.95rem',
          }}
        >
          <span>Mint couldn&apos;t finish loading. This usually means the app was updated.</span>
          <button
            type="button"
            onClick={() => window.location.reload()}
            style={{
              padding: '10px 24px',
              borderRadius: 10,
              border: 'none',
              background: '#10b981',
              color: '#ffffff',
              fontWeight: 650,
              fontSize: '0.9rem',
              cursor: 'pointer',
            }}
          >
            Reload
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
