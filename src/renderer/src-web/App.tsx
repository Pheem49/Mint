import React, { Suspense, useEffect, useState } from 'react'
import AuthGate from '../shared/components/AuthGate'
import ChunkErrorBoundary from '../shared/components/ChunkErrorBoundary'
import ModalErrorBoundary from '../shared/components/ModalErrorBoundary'
import { lazyWithRetry } from '../shared/utils/lazyWithRetry'

const SettingsWindow = lazyWithRetry(() => import('./components/SettingsWindow'))
const MintDashboard = lazyWithRetry(() => import('./components/MintDashboard'))

function getCurrentRoute(): string {
  if (typeof window === 'undefined') return '/'
  const hash = window.location.hash.replace(/^#![/]*/, '/').replace(/^#[/]*/, '/')
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

  useEffect(() => {
    // Preload SettingsWindow chunk in background so opening settings is instantaneous
    const timer = setTimeout(() => {
      import('./components/SettingsWindow').catch(() => {})
    }, 1200)
    return () => clearTimeout(timer)
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
            <ModalErrorBoundary onClose={() => window.settingsApi?.closeSettings?.()}>
              <Suspense fallback={<div className="auth-gate-loading" style={{ minHeight: 240 }}>Loading Settings…</div>}>
                <SettingsWindow />
              </Suspense>
            </ModalErrorBoundary>
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
