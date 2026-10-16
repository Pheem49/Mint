import type { SourceAttachment } from '../types'
import type { PromptOptions } from '../conversation/lifecycle'

async function thumbnail(dataUri: string): Promise<string | null> {
  try {
    const image = new Image()
    image.src = dataUri
    await image.decode()
    const ratio = Math.min(1, 160 / Math.max(image.naturalWidth, image.naturalHeight))
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(image.naturalWidth * ratio))
    canvas.height = Math.max(1, Math.round(image.naturalHeight * ratio))
    const context = canvas.getContext('2d')
    if (!context) return null
    context.drawImage(image, 0, 0, canvas.width, canvas.height)
    return canvas.toDataURL('image/webp', .75)
  } catch { return null }
}

export async function sourceAttachments(options: PromptOptions): Promise<SourceAttachment[]> {
  const mime = (uri: string) => /^data:([^;,]+)/.exec(uri)?.[1] || 'application/octet-stream'
  const images = await Promise.all((options.imageAttachments || []).map(async image => ({
    name: image.name, kind: 'image' as const, mimeType: mime(image.dataUri), thumbnailDataUri: await thumbnail(image.dataUri),
  })))
  const videos = (options.videoAttachments || []).map(video => ({ name: video.name, kind: 'video' as const, mimeType: mime(video.dataUri), thumbnailDataUri: null }))
  const document = options.documentAttachment
  return [...images, ...videos, ...(document ? [{ name: document.filename, kind: 'file' as const, mimeType: mime(document.dataUri), thumbnailDataUri: null }] : [])]
}
