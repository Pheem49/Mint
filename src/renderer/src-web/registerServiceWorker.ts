// Registers the PWA service worker (public/sw.js) for the Web UI build only.
// Gated to production: registering it under Vite's dev server would have the
// service worker intercept and cache HMR/module-graph requests, breaking
// live reload in ways that are confusing to debug.
export function registerServiceWorker() {
  if (!('serviceWorker' in navigator)) {
    return
  }
  if (!import.meta.env.PROD) {
    // In dev mode, unregister any leftover service worker from preview/production runs
    // so it doesn't intercept or corrupt Vite's module graph.
    navigator.serviceWorker.getRegistrations().then((registrations) => {
      for (const registration of registrations) {
        registration.unregister()
      }
    }).catch(() => {})
    return
  }
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js').catch((error) => {
      console.warn('[pwa] service worker registration failed', error)
    })
  })
}
