/**
 * shared/theme/themeManager.ts
 * Centralized theme controller and runtime state manager.
 */

import { ACCENT_FLAVORS } from './themes'

export const hexToRgb = (hex: string): { r: number; g: number; b: number } => {
  const clean = hex.replace('#', '')
  if (clean.length === 3) {
    const r = parseInt(clean[0] + clean[0], 16)
    const g = parseInt(clean[1] + clean[1], 16)
    const b = parseInt(clean[2] + clean[2], 16)
    return { r, g, b }
  }
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
  return result
    ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16),
      }
    : { r: 16, g: 185, b: 129 }
}

export const lightenColor = (hex: string, amount: number): string => {
  const clean = hex.replace('#', '')
  if (clean.length !== 6) return hex
  const num = parseInt(clean, 16)
  const r = Math.min(255, (num >> 16) + amount)
  const g = Math.min(255, ((num >> 8) & 0x00ff) + amount)
  const b = Math.min(255, (num & 0x0000ff) + amount)
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`
}

export const getContrastText = (hex: string): string => {
  const rgb = hexToRgb(hex)
  // Perceived relative luminance formula (W3C standard)
  const luminance = 0.299 * rgb.r + 0.587 * rgb.g + 0.114 * rgb.b
  return luminance > 160 ? '#09090b' : '#ffffff'
}

let systemThemeMediaQuery: MediaQueryList | null = null
let systemThemeListener: ((e: MediaQueryListEvent) => void) | null = null

export const applyTheme = (cfg: any): void => {
  if (typeof document === 'undefined') return

  const rawTheme = cfg.theme || 'dark'
  const surfaceStyle = cfg.surfaceStyle || (cfg.glassBlur === 'none' ? 'opaque' : 'glass')
  const accentColor = cfg.accentColor || '#10b981'
  const rgb = hexToRgb(accentColor)
  const contrastText = getContrastText(accentColor)

  // Resolve effective theme for 'system'
  let effectiveTheme = rawTheme
  if (rawTheme === 'system') {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    effectiveTheme = prefersDark ? 'dark' : 'light'

    // Attach reactive OS preference listener once
    if (!systemThemeListener) {
      systemThemeMediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
      systemThemeListener = (e) => {
        document.documentElement.setAttribute('data-theme', e.matches ? 'dark' : 'light')
      }
      systemThemeMediaQuery.addEventListener('change', systemThemeListener)
    }
  } else if (systemThemeListener && systemThemeMediaQuery) {
    systemThemeMediaQuery.removeEventListener('change', systemThemeListener)
    systemThemeListener = null
    systemThemeMediaQuery = null
  }

  // Set DOM data attributes
  document.documentElement.setAttribute('data-theme', effectiveTheme)
  document.documentElement.setAttribute('data-surface', surfaceStyle)

  // Set Accent variables
  document.documentElement.style.setProperty('--accent-default', accentColor)
  document.documentElement.style.setProperty('--accent', accentColor)
  document.documentElement.style.setProperty('--accent-hover', lightenColor(accentColor, 20))
  document.documentElement.style.setProperty('--accent-glow', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.35)`)
  document.documentElement.style.setProperty('--accent-subtle', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.14)`)
  document.documentElement.style.setProperty('--accent-border', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`)
  document.documentElement.style.setProperty('--text-on-accent', contrastText)
  document.documentElement.style.setProperty('--accent-contrast', contrastText)

  // Surface and Blur
  if (surfaceStyle === 'opaque') {
    document.documentElement.style.setProperty('--glass-blur', 'none')
  } else {
    document.documentElement.style.setProperty('--glass-blur', cfg.glassBlur || 'blur(16px)')
  }

  // Typography
  if (cfg.fontFamily) {
    document.body.style.fontFamily = cfg.fontFamily
  }
  if (cfg.fontSize) {
    document.documentElement.style.fontSize = cfg.fontSize
  }

  // Handle custom theme overrides if active
  if (rawTheme === 'custom') {
    if (cfg.customBgStart && cfg.customBgEnd) {
      document.documentElement.style.setProperty('--bg-canvas', cfg.customBgStart)
      document.documentElement.style.setProperty('--bg-color', cfg.customBgStart)
      document.documentElement.style.setProperty(
        '--bg-gradient',
        `linear-gradient(135deg, ${cfg.customBgStart} 0%, ${cfg.customBgEnd} 100%)`
      )
    }
    if (cfg.customPanelBg) {
      const pRgb = hexToRgb(cfg.customPanelBg)
      document.documentElement.style.setProperty('--bg-surface', `rgba(${pRgb.r}, ${pRgb.g}, ${pRgb.b}, 0.75)`)
      document.documentElement.style.setProperty('--panel-bg', `rgba(${pRgb.r}, ${pRgb.g}, ${pRgb.b}, 0.75)`)
      document.documentElement.style.setProperty('--surface-bg', `rgba(${pRgb.r}, ${pRgb.g}, ${pRgb.b}, 0.62)`)
    }
    return
  }

  // Clean up inline background and text overrides for standard themes
  ;[
    '--bg-canvas',
    '--bg-color',
    '--bg-gradient',
    '--panel-bg',
    '--panel-raised',
    '--panel-soft',
    '--chrome-bg',
    '--surface-bg',
    '--surface-strong',
    '--input-bg',
    '--text-main',
    '--text-chat',
    '--text-soft',
  ].forEach((prop) => document.documentElement.style.removeProperty(prop))
}
