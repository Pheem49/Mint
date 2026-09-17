/**
 * shared/theme/themes.ts
 * TypeScript constants and types for the Mint GUI Theme System.
 */

export type ColorMode = 'system' | 'dark' | 'light' | 'midnight'
export type SurfaceStyle = 'opaque' | 'glass'

export interface AccentFlavor {
  id: string
  name: string
  color: string
  contrastText: string
  description: string
}

export const ACCENT_FLAVORS: AccentFlavor[] = [
  {
    id: 'mint',
    name: 'Mint',
    color: '#10b981',
    contrastText: '#ffffff',
    description: 'Emerald brand signature',
  },
  {
    id: 'cyan',
    name: 'Cyan',
    color: '#06b6d4',
    contrastText: '#ffffff',
    description: 'Electric tech cyan',
  },
  {
    id: 'indigo',
    name: 'Indigo',
    color: '#6366f1',
    contrastText: '#ffffff',
    description: 'Refined violet blue',
  },
  {
    id: 'amber',
    name: 'Amber',
    color: '#f59e0b',
    contrastText: '#0a0a0b',
    description: 'Warm solar gold',
  },
  {
    id: 'rose',
    name: 'Rose',
    color: '#f43f5e',
    contrastText: '#ffffff',
    description: 'Vivid energized berry',
  },
  {
    id: 'monochrome',
    name: 'Monochrome',
    color: '#ffffff',
    contrastText: '#09090b',
    description: 'Minimalist black & white',
  },
]

export interface ThemePreset {
  id: string
  name: string
  description: string
  theme: 'dark' | 'light' | 'midnight'
  accentColor: string
  systemTextColor: string
  chatTextColor: string
  bgPreview: string
  panelPreview: string
  accentPreview: string
  textPreview: string
}

export const THEME_PRESETS: ThemePreset[] = [
  {
    id: 'preset-dark-mint',
    name: 'Dark Mint',
    description: 'Black background · Green elements · White text',
    theme: 'dark',
    accentColor: '#10b981',
    systemTextColor: '#f8fafc',
    chatTextColor: '#f8fafc',
    bgPreview: '#0a0a0c',
    panelPreview: '#18181b',
    accentPreview: '#10b981',
    textPreview: '#f8fafc',
  },
  {
    id: 'preset-light-mint',
    name: 'Light Mint',
    description: 'White background · Green elements · Dark text',
    theme: 'light',
    accentColor: '#10b981',
    systemTextColor: '#000000',
    chatTextColor: '#000000',
    bgPreview: '#f8fafc',
    panelPreview: '#ffffff',
    accentPreview: '#10b981',
    textPreview: '#000000',
  },
  {
    id: 'preset-dark-mono',
    name: 'Dark Monochrome',
    description: 'Black background · White elements · White text',
    theme: 'dark',
    accentColor: '#ffffff',
    systemTextColor: '#f8fafc',
    chatTextColor: '#f8fafc',
    bgPreview: '#0a0a0c',
    panelPreview: '#18181b',
    accentPreview: '#ffffff',
    textPreview: '#f8fafc',
  },
]
