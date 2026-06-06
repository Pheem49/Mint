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

  if (route.startsWith('/settings')) content = <SettingsWindow />

  return <Suspense fallback={null}>{content}</Suspense>
}
