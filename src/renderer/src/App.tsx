import React, { useEffect, useState } from 'react'
import ChatWindow from './components/ChatWindow'
import SettingsWindow from './components/SettingsWindow'
import SpotlightWindow from './components/SpotlightWindow'
import WidgetWindow from './components/WidgetWindow'
import ScreenPickerWindow from './components/ScreenPickerWindow'
import ProactiveGlowWindow from './components/ProactiveGlowWindow'

export default function App() {
  const [hash, setHash] = useState(window.location.hash)

  useEffect(() => {
    const handleHashChange = () => {
      setHash(window.location.hash)
    }
    window.addEventListener('hashchange', handleHashChange)
    return () => window.removeEventListener('hashchange', handleHashChange)
  }, [])

  if (hash === '#/settings') {
    return <SettingsWindow />
  }
  if (hash === '#/spotlight') {
    return <SpotlightWindow />
  }
  if (hash === '#/widget') {
    return <WidgetWindow />
  }
  if (hash === '#/screen-picker') {
    return <ScreenPickerWindow />
  }
  if (hash === '#/proactive-glow') {
    return <ProactiveGlowWindow />
  }

  return <ChatWindow />
}
