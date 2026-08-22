// Registers the PWA service worker (public/sw.js) for the Web UI build only.
// Gated to production: registering it under Vite's dev server would have the
// service worker intercept and cache HMR/module-graph requests, breaking
// live reload in ways that are confusing to debug.
export function registerServiceWorker() {
  if (!import.meta.env.PROD || !('serviceWorker' in navigator)) {
    return
  }
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js').catch((error) => {
      console.warn('[pwa] service worker registration failed', error)
    })
  })
}
