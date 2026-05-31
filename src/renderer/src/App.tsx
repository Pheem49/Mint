import React, { useState, useEffect } from 'react'
import SettingsWindow from './components/SettingsWindow'
import SpotlightWindow from './components/SpotlightWindow'
import WidgetWindow from './components/WidgetWindow'
import ProactiveGlow from './components/ProactiveGlow'
import ScreenPicker from './components/ScreenPicker'
import MintDashboard from './components/MintDashboard'

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

  if (route.startsWith('/settings')) {
    return <SettingsWindow />
  }
  if (route.startsWith('/spotlight')) {
    return <SpotlightWindow />
  }
  if (route.startsWith('/widget')) {
    return <WidgetWindow />
  }
  if (route.startsWith('/proactive-glow')) {
    return <ProactiveGlow />
  }
  if (route.startsWith('/screen-picker')) {
    return <ScreenPicker />
  }

  return <MintDashboard />
}
