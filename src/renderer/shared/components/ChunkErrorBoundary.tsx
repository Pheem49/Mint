import React from 'react'
import '../css/auth-gate.css'

interface State {
  hasError: boolean
}

/**
 * Catches errors from lazy-loaded route chunks (e.g. a stale cached page
 * trying to fetch a hashed JS file that no longer exists after a rebuild)
 * and offers a reload instead of leaving the app permanently blank.
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
        <div className="auth-gate-loading" style={{ flexDirection: 'column', gap: 12 }}>
          <span>Mint couldn&apos;t finish loading. This usually means the app was updated.</span>
          <button
            type="button"
            className="auth-gate-submit"
            style={{ maxWidth: 160 }}
            onClick={() => window.location.reload()}
          >
            Reload
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
