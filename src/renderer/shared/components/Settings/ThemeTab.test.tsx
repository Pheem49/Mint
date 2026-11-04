import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import ThemeTab from './ThemeTab'
import { DEFAULT_CONFIG } from '../../constants/config'
import { TEXT_COLOR_FLAVORS } from '../../theme/themes'

describe('ThemeTab Text Color Picker', () => {
  it('renders the Text color section with curated swatches and custom hex toggle', () => {
    const html = renderToStaticMarkup(
      <ThemeTab
        config={{ ...DEFAULT_CONFIG, textColor: '#f8fafc' }}
        updateField={() => {}}
      />
    )
    expect(html).toContain('Text color')
    expect(html).toContain('Custom Hex...')
    expect(html).toContain('aria-label="Text colors"')

    for (const flavor of TEXT_COLOR_FLAVORS) {
      expect(html).toContain(flavor.name)
    }
  })

  it('marks the active flavor based on config.textColor', () => {
    const html = renderToStaticMarkup(
      <ThemeTab
        config={{ ...DEFAULT_CONFIG, textColor: '#fde047' }}
        updateField={() => {}}
      />
    )
    // Retro Amber flavor (#fde047) should have aria-checked="true"
    expect(html).toMatch(/role="radio"[^>]*aria-checked="true"[^>]*title="Warm CRT phosphor amber gold"/)
  })

  it('marks Pure White as active when default #f8fafc is set', () => {
    const html = renderToStaticMarkup(
      <ThemeTab
        config={{ ...DEFAULT_CONFIG, textColor: '#f8fafc' }}
        updateField={() => {}}
      />
    )
    expect(html).toMatch(/role="radio"[^>]*aria-checked="true"[^>]*title="Crisp classic bright white"/)
  })
})
