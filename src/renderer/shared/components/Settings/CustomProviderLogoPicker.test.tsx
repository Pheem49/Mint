import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import CustomProviderLogoPicker, { svgStringToDataUrl, emojiToDataUrl } from './CustomProviderLogoPicker'

describe('CustomProviderLogoPicker helpers', () => {
  it('converts raw SVG string to valid encoded data URL and injects xmlns if missing', () => {
    const rawSvg = '<svg viewBox="0 0 24 24"><path d="M0 0h24v24H0z"/></svg>'
    const dataUrl = svgStringToDataUrl(rawSvg)
    expect(dataUrl.startsWith('data:image/svg+xml;utf8,')).toBe(true)
    expect(decodeURIComponent(dataUrl)).toContain('xmlns="http://www.w3.org/2000/svg"')
  })

  it('converts emoji to a vector SVG data URL', () => {
    const emojiDataUrl = emojiToDataUrl('🤖')
    expect(emojiDataUrl.startsWith('data:image/svg+xml;utf8,')).toBe(true)
    expect(decodeURIComponent(emojiDataUrl)).toContain('🤖')
  })
})

describe('CustomProviderLogoPicker rendering', () => {
  it('renders collapsed state by default with toggle button', () => {
    const html = renderToStaticMarkup(
      <CustomProviderLogoPicker
        onChange={vi.fn()}
        providerName="DeepSeek Test"
      />
    )
    expect(html).toContain('Choose logo')
    expect(html).toContain('is-collapsed')
    expect(html).not.toContain('AI Presets')
  })

  it('renders expanded state with AI Presets tab and popular AI models', () => {
    const html = renderToStaticMarkup(
      <CustomProviderLogoPicker
        onChange={vi.fn()}
        providerName="DeepSeek Test"
        defaultExpanded={true}
      />
    )
    expect(html).toContain('AI Presets')
    expect(html).toContain('Upload / Crop')
    expect(html).toContain('URL / Emoji')
    expect(html).toContain('DeepSeek')
    expect(html).toContain('Ollama')
    expect(html).toContain('OpenRouter')
    expect(html).toContain('Groq')
    expect(html).toContain('Mistral AI')
  })

  it('renders active logo image, change logo button, and remove button when value is provided', () => {
    const sampleLogo = 'data:image/svg+xml;utf8,%3Csvg%3E%3C/svg%3E'
    const html = renderToStaticMarkup(
      <CustomProviderLogoPicker
        value={sampleLogo}
        onChange={vi.fn()}
        providerName="Custom Provider"
      />
    )
    expect(html).toContain('Custom logo active')
    expect(html).toContain('Change logo')
    expect(html).toContain('Remove')
  })
})
