import React, { lazy, Suspense, useEffect, useState } from 'react'

const SettingsWindow = lazy(() => import('./components/SettingsWindow'))
const SpotlightWindow = lazy(() => import('./components/SpotlightWindow'))
const WidgetWindow = lazy(() => import('./components/WidgetWindow'))
const ProactiveGlow = lazy(() => import('./components/ProactiveGlow'))
const ScreenPicker = lazy(() => import('./components/ScreenPicker'))
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
  if (route.startsWith('/spotlight')) content = <SpotlightWindow />
  if (route.startsWith('/widget')) content = <WidgetWindow />
  if (route.startsWith('/proactive-glow')) content = <ProactiveGlow />
  if (route.startsWith('/screen-picker')) content = <ScreenPicker />

  return <Suspense fallback={null}>{content}</Suspense>
}
