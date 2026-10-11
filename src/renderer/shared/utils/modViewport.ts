import type { ModViewport } from '../mods'
export function modViewport(width: number, height: number, surface: 'web' | 'desktop', open: boolean): ModViewport | null {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 24 || height < 1) return null
  const widthPx = Math.min(100000, Math.floor(width - 24))
  const heightPx = Math.min(100000, Math.floor(height))
  return { surface, widthPx, heightPx, columns: Math.min(10000, Math.max(1, Math.floor(widthPx / 8))), rows: Math.min(10000, Math.max(1, Math.floor(heightPx / 20))), sidePanelOpen: open }
}
