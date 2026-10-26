import { describe, expect, it, vi } from 'vitest'
import { renderToStaticMarkup } from 'react-dom/server'
import { renderFormattedMessage, sanitizeLatex } from './markdown'
import { extractToolCardPayloads } from '../components/cards/ToolResultCards'
import { subscribeThemeChanges } from '../components/cards/MermaidCard'
import type { WebSearchSource } from './agentActivity'

const source: WebSearchSource = {
  title: 'WindowPet',
  url: 'https://github.com/SeakMengs/WindowPet',
  imageUrl: 'https://opengraph.githubassets.com/example/SeakMengs/WindowPet',
  snippet: '',
  domain: 'github.com',
  faviconUrl: '',
}

describe('web search image links', () => {
  it('opens the paired source page from the thumbnail and the image file from a separate link', () => {
    const html = renderToStaticMarkup(renderFormattedMessage(`![WindowPet](${source.imageUrl})`, [source]))

    expect(html).toContain(`href="${source.url}"`)
    expect(html).toContain(`href="${source.imageUrl}"`)
    expect(html).toContain('View full image')
  })

  it('does not invent a source page for an unrelated image', () => {
    const otherImage = 'https://example.com/other.png'
    const html = renderToStaticMarkup(renderFormattedMessage(`![Other](${otherImage})`, [source]))

    expect(html).toContain(`href="${otherImage}"`)
    expect(html).not.toContain(`href="${source.url}"`)
    expect(html).not.toContain('View full image')
  })

  it('uses an explicitly linked page without nesting image links', () => {
    const html = renderToStaticMarkup(renderFormattedMessage(`[![WindowPet](${source.imageUrl})](${source.url})`))

    expect(html).toContain(`href="${source.url}"`)
    expect(html).toContain(`href="${source.imageUrl}"`)
    expect(html).not.toMatch(/<a[^>]*>\s*<a/)
  })

  it('fast-paths sanitizeLatex on plain prose and converts LaTeX symbols when present', () => {
    const replaceSpy = vi.spyOn(String.prototype, 'replace')
    const plain = 'Hello world! This is a normal paragraph without math.'
    expect(sanitizeLatex(plain)).toBe(plain)
    expect(replaceSpy).not.toHaveBeenCalled()
    replaceSpy.mockRestore()

    expect(sanitizeLatex('A $\\rightarrow$ B and $\\leq$ C')).toBe('A → B and ≤ C')
  })

  it('extracts tool result card payloads in a single pass and respects inline fence suppression', () => {
    const progress: any[] = [
      { type: 'ToolEnd', data: { action: 'weather', result: '```weather_json\n{"city":"Bangkok"}\n```' } },
      { type: 'ToolEnd', data: { action: 'calculation', result: '```calculation_json\n{"result":42}\n```' } },
    ]
    expect(extractToolCardPayloads(progress, 'Here is the result')).toEqual({
      weatherData: { city: 'Bangkok' },
      stockData: null,
      calcData: { result: 42 },
      imageSearchData: null,
      imageGenData: null,
    })
    expect(extractToolCardPayloads(progress, '```weather_json\n{"city":"Bangkok"}\n```').weatherData).toBeNull()
  })

  it('multiplexes Mermaid theme change listeners through a single MutationObserver', () => {
    let observeCount = 0
    let disconnectCount = 0
    class FakeMutationObserver {
      observe() { observeCount++ }
      disconnect() { disconnectCount++ }
    }
    vi.stubGlobal('MutationObserver', FakeMutationObserver)
    vi.stubGlobal('document', { documentElement: {} })

    const unsub1 = subscribeThemeChanges(() => {})
    const unsub2 = subscribeThemeChanges(() => {})
    expect(observeCount).toBe(1)
    unsub1()
    expect(disconnectCount).toBe(0)
    unsub2()
    expect(disconnectCount).toBe(1)
    vi.unstubAllGlobals()
  })
})
