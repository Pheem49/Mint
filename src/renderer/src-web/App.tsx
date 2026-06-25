import React, { lazy, Suspense, useEffect, useState } from 'react'

const SettingsWindow = lazy(() => import('./components/SettingsWindow'))
const MintDashboard = lazy(() => import('./components/MintDashboard'))

export default function App() {
  const [hash, setHash] = useState(window.location.hash || '#/')

  useEffect(() => {
    const handleHashChange = () => {
      setHash(window.location.hash || '#/')
    }
    window.addEventListener('hashchange', handleHashChange)
    return () => window.removeEventListener('hashchange', handleHashChange)
  }, [])

  // Basic route parsing
  const route = hash.replace(/^#/, '')

  let content = <MintDashboard />

  // For web, show settings as a centered modal overlay instead of a full-page route
  if (route.startsWith('/settings')) {
    content = (
      <>
        <MintDashboard />
        <div
          className="settings-modal-overlay"
          onClick={() => window.api?.closeSettings?.()}
        >
          <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
            <SettingsWindow />
          </div>
        </div>
      </>
    )
  }

  return <Suspense fallback={null}>{content}</Suspense>
}
