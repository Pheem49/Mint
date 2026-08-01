import React, { lazy, Suspense, useEffect, useState } from 'react'

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
    return () => {
      window.removeEventListener('popstate', handleUrlChange)
      window.removeEventListener('hashchange', handleUrlChange)
    }
  }, [])

  let content = <MintDashboard />

  if (route.startsWith('/settings')) content = <SettingsWindow />
  if (route.startsWith('/spotlight')) content = <SpotlightWindow />
  if (route.startsWith('/widget')) content = <WidgetWindow />
  if (route.startsWith('/proactive-glow')) content = <ProactiveGlow />
  if (route.startsWith('/screen-picker')) content = <ScreenPicker />


  return <Suspense fallback={null}>{content}</Suspense>
}
