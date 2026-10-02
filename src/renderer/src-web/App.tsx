import React, { Suspense, useEffect, useState } from 'react'
import AuthGate from '../shared/components/AuthGate'
import ChunkErrorBoundary from '../shared/components/ChunkErrorBoundary'
import ModalErrorBoundary from '../shared/components/ModalErrorBoundary'
import PullToRefresh from './components/PullToRefresh'
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
    const viewport = window.visualViewport
    let fullHeight = viewport?.height ?? window.innerHeight
    let focusTimer: ReturnType<typeof setTimeout> | undefined
    const updateHeight = () => {
      const height = viewport?.height ?? window.innerHeight
      if (height > 0) {
        const focused = document.activeElement?.matches('input, textarea, [contenteditable="true"]') ?? false
        if (!focused) fullHeight = Math.max(fullHeight, height)
        document.documentElement.style.setProperty('--mint-viewport-height', `${height}px`)
        document.documentElement.style.setProperty('--mint-viewport-top', `${viewport?.offsetTop ?? 0}px`)
        document.documentElement.classList.toggle('mint-keyboard-open', focused && fullHeight - height > 100)
      }
    }
    const updateAfterFocus = () => {
      updateHeight()
      if (focusTimer) clearTimeout(focusTimer)
      focusTimer = setTimeout(updateHeight, 250)
    }
    updateHeight()
    viewport?.addEventListener('resize', updateHeight)
    viewport?.addEventListener('scroll', updateHeight)
    window.addEventListener('resize', updateHeight)
    document.addEventListener('focusin', updateAfterFocus)
    document.addEventListener('focusout', updateAfterFocus)
    return () => {
      viewport?.removeEventListener('resize', updateHeight)
      viewport?.removeEventListener('scroll', updateHeight)
      window.removeEventListener('resize', updateHeight)
      document.removeEventListener('focusin', updateAfterFocus)
      document.removeEventListener('focusout', updateAfterFocus)
      if (focusTimer) clearTimeout(focusTimer)
      document.documentElement.style.removeProperty('--mint-viewport-height')
      document.documentElement.style.removeProperty('--mint-viewport-top')
      document.documentElement.classList.remove('mint-keyboard-open')
    }
  }, [])

  useEffect(() => {
    // Preload SettingsWindow chunk in background so opening settings is instantaneous
    const timer = setTimeout(() => {
      import('./components/SettingsWindow').catch(() => {})
    }, 1200)
    return () => clearTimeout(timer)
  }, [])

  useEffect(() => {
    const handlePreloadError = (event: Event) => {
      // Prevent crash when an outdated chunk hash is requested after a rebuild
      event.preventDefault()
      const key = 'mint_vite_preload_reload'
      if (typeof window !== 'undefined' && !window.sessionStorage.getItem(key)) {
        window.sessionStorage.setItem(key, 'true')
        window.location.reload()
      }
    }
    window.addEventListener('vite:preloadError', handlePreloadError)
    return () => window.removeEventListener('vite:preloadError', handlePreloadError)
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
      <PullToRefresh disabled={route.startsWith('/settings')} />
      <Suspense fallback={<div className="auth-gate-loading">Loading Mint…</div>}>
        <AuthGate>{content}</AuthGate>
      </Suspense>
    </ChunkErrorBoundary>
  )
}
