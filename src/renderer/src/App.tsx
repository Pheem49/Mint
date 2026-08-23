import React, { lazy, Suspense, useEffect, useState } from 'react'
import AuthGate from '../shared/components/AuthGate'
import ChunkErrorBoundary from '../shared/components/ChunkErrorBoundary'
import { listen } from './tauri'

const SettingsWindow = lazy(() => import('./components/SettingsWindow'))
const SpotlightWindow = lazy(() => import('./components/SpotlightWindow'))
const WidgetWindow = lazy(() => import('./components/WidgetWindow'))
const ProactiveGlow = lazy(() => import('./components/ProactiveGlow'))
const ScreenPicker = lazy(() => import('./components/ScreenPicker'))
const MintDashboard = lazy(() => import('./components/MintDashboard'))
function getCurrentRoute(): string {
  if (typeof window === 'undefined') return '/'
  const hash = window.location.hash.replace(/^#/, '')
  const pathname = window.location.pathname
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

    // Fired by the tray "Settings" item (src-tauri/src/lib.rs) so it opens
    // the settings view in this window instead of spawning a separate one.
    const unlistenPromise = listen('open-settings', () => {
      window.location.hash = '#/settings'
    })

    return () => {
      window.removeEventListener('popstate', handleUrlChange)
      window.removeEventListener('hashchange', handleUrlChange)
      unlistenPromise.then((unlisten) => unlisten())
    }
  }, [])

  // Auxiliary overlay windows (spotlight/widget/proactive-glow/screen-picker)
  // belong to an already-running, already-authenticated main session, so
  // they render without their own login gate.
  if (route.startsWith('/spotlight')) {
    return (
      <Suspense fallback={null}>
        <SpotlightWindow />
      </Suspense>
    )
  }
  if (route.startsWith('/widget')) {
    return (
      <Suspense fallback={null}>
        <WidgetWindow />
      </Suspense>
    )
  }
  if (route.startsWith('/proactive-glow')) {
    return (
      <Suspense fallback={null}>
        <ProactiveGlow />
      </Suspense>
    )
  }
  if (route.startsWith('/screen-picker')) {
    return (
      <Suspense fallback={null}>
        <ScreenPicker />
      </Suspense>
    )
  }

  const content = route.startsWith('/settings') ? (
    <>
      <MintDashboard />
      <div className="settings-modal-overlay" onClick={() => { window.location.hash = '#/' }}>
        <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
          <Suspense fallback={null}>
            <SettingsWindow />
          </Suspense>
        </div>
      </div>
    </>
  ) : (
    <MintDashboard />
  )

  return (
    <ChunkErrorBoundary>
      <Suspense fallback={<div className="auth-gate-loading">Loading Mint…</div>}>
        <AuthGate>{content}</AuthGate>
      </Suspense>
    </ChunkErrorBoundary>
  )
}
