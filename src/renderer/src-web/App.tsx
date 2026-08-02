import React, { lazy, Suspense, useEffect, useState } from 'react'
import AuthGate from '../shared/components/AuthGate'
import ChunkErrorBoundary from '../shared/components/ChunkErrorBoundary'

const SettingsWindow = lazy(() => import('./components/SettingsWindow'))
const MintDashboard = lazy(() => import('./components/MintDashboard'))

function getCurrentRoute(): string {
  if (typeof window === 'undefined') return '/'
  const hash = window.location.hash.replace(/^#/, '')
  const pathname = window.location.pathname
  if (hash.startsWith('/settings') || pathname.startsWith('/settings')) {
    return '/settings'
  }
  return hash || pathname || '/'
}

export default function App() {
  const [route, setRoute] = useState(getCurrentRoute)

  useEffect(() => {
    const handleUrlChange = () => {
      setRoute(getCurrentRoute())
    }
    window.addEventListener('popstate', handleUrlChange)
    window.addEventListener('hashchange', handleUrlChange)
    return () => {
      window.removeEventListener('popstate', handleUrlChange)
      window.removeEventListener('hashchange', handleUrlChange)
    }
  }, [])

  let content = <MintDashboard />

  // For web, show settings as a centered modal overlay instead of a full-page route
  if (route.startsWith('/settings')) {
    content = (
      <>
        <MintDashboard />
        <div
          className="settings-modal-overlay"
          onClick={() => window.settingsApi?.closeSettings?.()}
        >
          <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
            <SettingsWindow />
          </div>
        </div>
      </>
    )
  }

  return (
    <ChunkErrorBoundary>
      <Suspense fallback={<div className="auth-gate-loading">Loading Mint…</div>}>
        <AuthGate>{content}</AuthGate>
      </Suspense>
    </ChunkErrorBoundary>
  )
}
