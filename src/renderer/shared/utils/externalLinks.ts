/**
 * Shared utility for detecting and intercepting external link navigation.
 */

/**
 * Determines whether a given href/origin represents an external destination.
 */
export function isExternalHref(href: string | null | undefined, anchorOrigin?: string): boolean {
  if (!href) return false
  const trimmed = href.trim()
  if (trimmed.startsWith('//') || /^(https?:)?\/\//i.test(trimmed)) {
    return true
  }

  if (
    trimmed.startsWith('#') ||
    trimmed.startsWith('/') ||
    trimmed.startsWith('./') ||
    trimmed.startsWith('../') ||
    trimmed.startsWith('javascript:') ||
    trimmed.startsWith('data:') ||
    trimmed.startsWith('blob:')
  ) {
    return false
  }

  if (typeof window !== 'undefined' && anchorOrigin && window.location?.origin) {
    return anchorOrigin !== window.location.origin
  }

  return false
}

export type OpenExternalFn = (url: string) => Promise<any>

/**
 * Factory that creates a link click interceptor function for a given `openExternal` handler.
 */
export function createExternalLinkInterceptor(openExternal: OpenExternalFn) {
  return function installExternalLinkInterceptor(
    targetDocument: Document | null = typeof document !== 'undefined' ? document : null
  ): () => void {
    if (!targetDocument) return () => {}

    const handleClick = (event: MouseEvent) => {
      // Only handle primary click (left) or auxiliary click (middle)
      if (event.defaultPrevented || (event.button !== 0 && event.button !== 1)) {
        return
      }

      const target = event.target as HTMLElement | null
      if (!target || typeof target.closest !== 'function') return

      const anchor = target.closest('a') as HTMLAnchorElement | null
      if (!anchor || anchor.hasAttribute('download')) return

      const rawHref = anchor.getAttribute('href') || ''
      let anchorOrigin = ''
      try {
        anchorOrigin = anchor.origin || ''
      } catch {
        anchorOrigin = ''
      }

      const isExternal = isExternalHref(rawHref, anchorOrigin)
      const isBlank = anchor.target === '_blank'

      // Only intercept if classified as external, or marked with target="_blank" without being relative/hash/javascript
      if (!isExternal && (!isBlank || rawHref.startsWith('/') || rawHref.startsWith('#') || rawHref.startsWith('javascript:') || rawHref.startsWith('./') || rawHref.startsWith('../'))) {
        return
      }

      let resolvedUrl = ''
      try {
        resolvedUrl = anchor.href || rawHref
      } catch {
        resolvedUrl = rawHref
      }

      event.preventDefault()
      openExternal(resolvedUrl).catch((err) => {
        console.error('Failed to open external link:', err)
      })
    }

    targetDocument.addEventListener('click', handleClick, true)
    return () => {
      targetDocument.removeEventListener('click', handleClick, true)
    }
  }
}
