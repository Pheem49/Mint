import type { AgentProgress } from '../types'

/** view_image returns an inline image, optionally followed by post-tool hook notes. */
export function viewedImageDataUri(result: string): string | null {
  const firstLine = result.trim().split('\n', 1)[0].trim()
  return /^data:image\/(?:png|jpeg|gif|webp|svg\+xml);base64,[A-Za-z0-9+/]+={0,2}$/.test(firstLine)
    ? firstLine : null
}

export function viewedImagesFrom(progress: AgentProgress[]) {
  const images = new Map<string, { id: string; src: string; path: string; name: string }>()
  let anonymousCount = 0
  progress.forEach(event => {
    if (event.type !== 'ToolEnd' || event.data.action !== 'view_image'
      || (event.data.status && event.data.status !== 'success')) return
    const src = viewedImageDataUri(event.data.result)
    if (!src) return
    const path = typeof event.data.input.path === 'string' ? event.data.input.path : ''
    const name = path.replaceAll('\\', '/').split('/').at(-1) || 'Image'
    // Legacy/parallel results may omit callId. Count only retained images so
    // trimming unrelated progress cannot remount an image and close its dialog.
    const id = event.data.callId || `view-image-${anonymousCount++}`
    images.set(id, { id, src, path, name })
  })
  return [...images.values()]
}
