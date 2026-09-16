import React, { lazy } from 'react'

const CHUNK_RELOAD_KEY = 'mint_chunk_reload_attempted'

function isChunkLoadError(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err || '')
  const name = err instanceof Error ? err.name : ''
  return (
    name === 'ChunkLoadError' ||
    /Failed to fetch dynamically imported module/i.test(msg) ||
    /Loading chunk .* failed/i.test(msg) ||
    /error loading dynamically imported module/i.test(msg) ||
    /Failed to load module script/i.test(msg)
  )
}

/**
 * Wraps dynamic `import()` with retry logic and automatic reload recovery
 * to gracefully handle transient chunk loading failures (e.g. Vite dev-server
 * rebuilds, stale cached hashed chunks, or deployment asset transitions).
 */
export function lazyWithRetry<T extends React.ComponentType<any>>(
  factory: () => Promise<{ default: T }>,
  retries = 2,
  interval = 400
): React.LazyExoticComponent<T> {
  return lazy(() =>
    new Promise<{ default: T }>((resolve, reject) => {
      const attempt = (remaining: number) => {
        factory()
          .then((comp) => {
            if (typeof window !== 'undefined') {
              window.sessionStorage.removeItem(CHUNK_RELOAD_KEY)
            }
            resolve(comp)
          })
          .catch((error) => {
            if (remaining <= 0) {
              if (isChunkLoadError(error) && typeof window !== 'undefined') {
                const alreadyReloaded = window.sessionStorage.getItem(CHUNK_RELOAD_KEY)
                if (!alreadyReloaded) {
                  window.sessionStorage.setItem(CHUNK_RELOAD_KEY, 'true')
                  window.location.reload()
                  return
                }
              }
              reject(error)
              return
            }
            setTimeout(() => {
              attempt(remaining - 1)
            }, interval)
          })
      }
      attempt(retries)
    })
  )
}

