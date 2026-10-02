/**
 * shared/utils/ui.ts
 * Shared UI and theme helper functions.
 * Used by both Desktop and Web components.
 */

export function numericSetting(value: unknown, fallback: number): number {
  const numeric = Number(value)
  return Number.isFinite(numeric) ? numeric : fallback
}

/** Gap between two messages large enough to draw a divider between them —
 * long enough that they read as separate sitting-down-to-chat sessions
 * rather than back-and-forth replies in the same one. */
const SESSION_GAP_MS = 30 * 60 * 1000

export function parseUtcDate(value: unknown): Date {
  if (!value) return new Date()
  if (value instanceof Date) return value
  if (typeof value !== 'string') return new Date(value as any)
  let s = value.trim()
  if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/.test(s)) {
    s = s.replace(' ', 'T') + 'Z'
  } else if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?$/.test(s)) {
    s = s + 'Z'
  }
  const date = new Date(s)
  return Number.isNaN(date.getTime()) ? new Date() : date
}

export function shouldShowSessionDivider(prevCreatedAt: unknown, createdAt: unknown): boolean {
  const prev = parseUtcDate(prevCreatedAt).getTime()
  const curr = parseUtcDate(createdAt).getTime()
  if (!Number.isFinite(prev) || !Number.isFinite(curr)) return false
  return Math.abs(curr - prev) > SESSION_GAP_MS
}

export function formatSessionDividerLabel(createdAt: unknown): string {
  const date = parseUtcDate(createdAt)
  if (Number.isNaN(date.getTime())) return ''
  const now = new Date()
  const time = date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  if (date.toDateString() === now.toDateString()) return time
  const yesterday = new Date(now)
  yesterday.setDate(now.getDate() - 1)
  if (date.toDateString() === yesterday.toDateString()) return `Yesterday, ${time}`
  return `${date.toLocaleDateString([], { month: 'short', day: 'numeric' })}, ${time}`
}

export function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}

export function createObjectUrlPreview(file: File): { objectUrl: string; revoke: () => void } {
  const objectUrl = URL.createObjectURL(file)
  return {
    objectUrl,
    revoke: () => {
      try {
        URL.revokeObjectURL(objectUrl)
      } catch (e) {
        // ignore
      }
    },
  }
}

export function readImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error ?? new Error('Unable to read image'))
    reader.readAsDataURL(file)
  })
}

export function readDocument(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error ?? new Error('Unable to read document'))
    reader.readAsDataURL(file)
  })
}

export async function createTrimmedImagePreview(dataUri: string): Promise<string> {
  const image = await new Promise<HTMLImageElement>((resolve, reject) => {
    const nextImage = new Image()
    nextImage.onload = () => resolve(nextImage)
    nextImage.onerror = () => reject(new Error('Unable to prepare image preview'))
    nextImage.src = dataUri
  })

  const canvas = document.createElement('canvas')
  canvas.width = image.naturalWidth || image.width
  canvas.height = image.naturalHeight || image.height
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context || canvas.width === 0 || canvas.height === 0) return dataUri

  context.drawImage(image, 0, 0)
  const pixels = context.getImageData(0, 0, canvas.width, canvas.height)
  let minX = canvas.width
  let minY = canvas.height
  let maxX = -1
  let maxY = -1

  for (let y = 0; y < canvas.height; y += 1) {
    for (let x = 0; x < canvas.width; x += 1) {
      const alpha = pixels.data[(y * canvas.width + x) * 4 + 3]
      if (alpha > 12) {
        minX = Math.min(minX, x)
        minY = Math.min(minY, y)
        maxX = Math.max(maxX, x)
        maxY = Math.max(maxY, y)
      }
    }
  }

  if (maxX < minX || maxY < minY) return dataUri

  const padding = 8
  const sx = Math.max(0, minX - padding)
  const sy = Math.max(0, minY - padding)
  const sw = Math.min(canvas.width - sx, maxX - minX + 1 + padding * 2)
  const sh = Math.min(canvas.height - sy, maxY - minY + 1 + padding * 2)

  if (sw >= canvas.width * 0.92 && sh >= canvas.height * 0.92) return dataUri

  const previewCanvas = document.createElement('canvas')
  previewCanvas.width = sw
  previewCanvas.height = sh
  const previewContext = previewCanvas.getContext('2d')
  if (!previewContext) return dataUri
  previewContext.drawImage(canvas, sx, sy, sw, sh, 0, 0, sw, sh)
  return previewCanvas.toDataURL('image/png')
}

import { applyTheme, hexToRgb, lightenColor, getContrastText } from '../theme/themeManager'
export { hexToRgb, lightenColor, getContrastText }

export const applyThemeStyles = (cfg: any): void => {
  applyTheme(cfg)
}


