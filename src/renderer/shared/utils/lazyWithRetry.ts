import React, { lazy } from 'react'

/**
 * Wraps dynamic `import()` with retry logic to gracefully handle transient
 * chunk loading failures (e.g. Vite dev-server module re-optimization,
 * temporary network drops, or deployment asset transitions).
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
          .then(resolve)
          .catch((error) => {
            if (remaining <= 0) {
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
