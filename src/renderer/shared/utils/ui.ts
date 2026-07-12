/**
 * shared/utils/ui.ts
 * Shared UI and theme helper functions.
 * Used by both Desktop and Web components.
 */

export function numericSetting(value: unknown, fallback: number): number {
  const numeric = Number(value)
  return Number.isFinite(numeric) ? numeric : fallback
}

export function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
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

export const lightenColor = (hex: string, amount: number): string => {
  const clean = hex.replace('#', '')
  if (clean.length !== 6) return hex
  const num = parseInt(clean, 16)
  const r = Math.min(255, (num >> 16) + amount)
  const g = Math.min(255, ((num >> 8) & 0x00FF) + amount)
  const b = Math.min(255, (num & 0x0000FF) + amount)
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`
}

export const hexToRgb = (hex: string): { r: number; g: number; b: number } => {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
  return result ? {
    r: parseInt(result[1], 16),
    g: parseInt(result[2], 16),
    b: parseInt(result[3], 16),
  } : { r: 15, g: 23, b: 42 }
}

export const applyThemeStyles = (cfg: any): void => {
  const theme = cfg.theme || 'dark'
  const accentColor = cfg.accentColor || '#4f83e6'
  const systemTextColor = cfg.systemTextColor || '#f8fafc'

  document.documentElement.setAttribute('data-theme', theme)
  document.documentElement.style.setProperty('--accent', accentColor)
  document.documentElement.style.setProperty('--accent-hover', lightenColor(accentColor, 20))
  document.documentElement.style.setProperty('--text-main', systemTextColor)
  document.documentElement.style.setProperty('--glass-blur', cfg.glassBlur || 'blur(16px)')
  document.body.style.fontFamily = cfg.fontFamily || "'Outfit', sans-serif"
  document.documentElement.style.fontSize = cfg.fontSize || '18px'

  if (theme === 'custom') {
    if (cfg.customBgStart && cfg.customBgEnd) {
      document.documentElement.style.setProperty('--bg-color', cfg.customBgStart)
      document.documentElement.style.setProperty('--bg-gradient', `linear-gradient(135deg, ${cfg.customBgStart} 0%, ${cfg.customBgEnd} 100%)`)
    }
    if (cfg.customPanelBg) {
      const rgb = hexToRgb(cfg.customPanelBg)
      document.documentElement.style.setProperty('--panel-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.75)`)
      document.documentElement.style.setProperty('--panel-raised', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.82)`)
      document.documentElement.style.setProperty('--panel-soft', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.46)`)
      document.documentElement.style.setProperty('--chrome-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.88)`)
      document.documentElement.style.setProperty('--surface-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.62)`)
      document.documentElement.style.setProperty('--surface-strong', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.86)`)
      document.documentElement.style.setProperty('--input-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.72)`)
    }
    return
  }

  ;[
    '--bg-color',
    '--bg-gradient',
    '--panel-bg',
    '--panel-raised',
    '--panel-soft',
    '--chrome-bg',
    '--surface-bg',
    '--surface-strong',
    '--input-bg',
  ].forEach((prop) => document.documentElement.style.removeProperty(prop))
}
