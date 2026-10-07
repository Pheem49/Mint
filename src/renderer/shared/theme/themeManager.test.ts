import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { applyTheme } from './themeManager'

describe('theme surface settings', () => {
  let attributes: Map<string, string>
  let properties: Map<string, string>

  beforeEach(() => {
    attributes = new Map()
    properties = new Map()
    // The Node test environment supplies only the DOM boundary used by applyTheme.
    vi.stubGlobal('document', {
      documentElement: {
        setAttribute: (name: string, value: string) => attributes.set(name, value),
        style: {
          setProperty: (name: string, value: string) => properties.set(name, value),
          removeProperty: (name: string) => properties.delete(name),
        },
      },
      body: { style: {} },
    })
  })

  afterEach(() => vi.unstubAllGlobals())

  it.each([undefined, 'glass', 'opaque'])('uses opaque surfaces when blur is disabled and surfaceStyle is %s', (surfaceStyle) => {
    applyTheme({ theme: 'dark', surfaceStyle, glassBlur: 'none' })
    expect(attributes.get('data-surface')).toBe('opaque')
    expect(properties.get('--glass-blur')).toBe('none')
  })

  it('preserves enabled glass surfaces and the selected blur', () => {
    applyTheme({ theme: 'dark', surfaceStyle: 'glass', glassBlur: 'blur(24px)' })
    expect(attributes.get('data-surface')).toBe('glass')
    expect(properties.get('--glass-blur')).toBe('blur(24px)')
  })

  it('keeps explicit opaque surfaces solid despite a saved blur value', () => {
    applyTheme({ theme: 'light', surfaceStyle: 'opaque', glassBlur: 'blur(16px)' })
    expect(attributes.get('data-surface')).toBe('opaque')
    expect(properties.get('--glass-blur')).toBe('none')
  })
})
