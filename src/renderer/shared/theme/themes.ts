/**
 * shared/theme/themes.ts
 * TypeScript constants and types for the Mint GUI Theme System.
 */

export type ColorMode = 'system' | 'dark' | 'light' | 'midnight' | 'studio'
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
  theme: 'dark' | 'light' | 'midnight' | 'studio'
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
    description: 'Black background · Mint green & slate gray elements',
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
    description: 'Soft gray background · Green elements · High-contrast text',
    theme: 'light',
    accentColor: '#10b981',
    systemTextColor: '#0f172a',
    chatTextColor: '#0f172a',
    bgPreview: '#f0f2f5',
    panelPreview: '#ffffff',
    accentPreview: '#10b981',
    textPreview: '#0f172a',
  },
  {
    id: 'preset-studio-dark',
    name: 'Studio Dark',
    description: '#101010 canvas · #1c1c1c surfaces · #323232 active',
    theme: 'studio',
    accentColor: '#10b981',
    systemTextColor: '#e1e1e1',
    chatTextColor: '#e1e1e1',
    bgPreview: '#101010',
    panelPreview: '#161616',
    accentPreview: '#10b981',
    textPreview: '#e1e1e1',
  },
  {
    id: 'preset-dark-mono',
    name: 'Dark Monochrome',
    description: 'Black background · Slate gray surfaces · White text',
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
