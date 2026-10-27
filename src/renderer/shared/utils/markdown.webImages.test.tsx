import { describe, expect, it, vi } from 'vitest'
import { renderToStaticMarkup } from 'react-dom/server'
import { renderFormattedMessage, sanitizeLatex, autoHealMarkdownImages, ChatMediaImage, resolveMediaUrl, resolveApiProxyUrl } from './markdown'
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

  it('auto-heals unclosed markdown image tags at the end of a line', () => {
    const unclosed = '![Machi 馬吉 Portaly](https://img.portaly.cc/12345'
    expect(autoHealMarkdownImages(unclosed)).toBe('![Machi 馬吉 Portaly](https://img.portaly.cc/12345)')

    const multiline = 'Some text\n![Unclosed](https://example.com/pic.png\n\nMore text'
    expect(autoHealMarkdownImages(multiline)).toBe('Some text\n![Unclosed](https://example.com/pic.png)\n\nMore text')

    const alreadyClosed = '![Closed](https://example.com/pic.png)'
    expect(autoHealMarkdownImages(alreadyClosed)).toBe(alreadyClosed)
  })

  it('renders unclosed image tags properly as images through renderFormattedMessage', () => {
    const raw = '![Machi](https://img.portaly.cc/12345'
    const html = renderToStaticMarkup(renderFormattedMessage(raw))
    expect(html).toContain('<img')
    expect(html).toContain('src="https://img.portaly.cc/12345"')
    expect(html).not.toContain('![Machi](')
  })

  it('renders a fallback UI with link when image fails to load', () => {
    const html = renderToStaticMarkup(
      <ChatMediaImage
        src="https://pbs.twimg.com/profile_banners/123/456"
        alt="Machi on X"
        initialError
      />
    )
    expect(html).toContain('chat-media-card--failed')
    expect(html).toContain('Image failed to load')
    expect(html).toContain('Machi on X')
    expect(html).toContain('href="https://pbs.twimg.com/profile_banners/123/456/1500x500"')
    expect(html).toContain('Open link')
  })

  it('normalizes Twitter profile banner URLs missing dimensions in resolveMediaUrl', () => {
    const rawBanner = 'https://pbs.twimg.com/profile_banners/1366430884698546176/1684330404'
    expect(resolveMediaUrl(rawBanner)).toBe('https://pbs.twimg.com/profile_banners/1366430884698546176/1684330404/1500x500')

    const trailingSlash = 'https://pbs.twimg.com/profile_banners/1366430884698546176/1684330404/'
    expect(resolveMediaUrl(trailingSlash)).toBe('https://pbs.twimg.com/profile_banners/1366430884698546176/1684330404/1500x500')

    const alreadySized = 'https://pbs.twimg.com/profile_banners/1366430884698546176/1684330404/1500x500'
    expect(resolveMediaUrl(alreadySized)).toBe(alreadySized)
  })

  it('generates local image proxy URL with resolveApiProxyUrl', () => {
    expect(resolveApiProxyUrl('')).toBe('')

    // Default test environment (origin is empty or localhost)
    const target = 'https://pbs.twimg.com/profile_banners/123/456/1500x500'
    const proxyUrl = resolveApiProxyUrl(target)
    expect(proxyUrl).toContain('/api/proxy-image?url=https%3A%2F%2Fpbs.twimg.com%2Fprofile_banners%2F123%2F456%2F1500x500')

    // Web UI dev environment on port 9000
    vi.stubGlobal('window', { location: { port: '9000', origin: 'http://localhost:9000' } })
    expect(resolveApiProxyUrl(target)).toBe(
      `http://localhost:3000/api/proxy-image?url=${encodeURIComponent(target)}`
    )
    vi.unstubAllGlobals()
  })

  it('renders image with proxy endpoint when stage is proxy', () => {
    const rawBanner = 'https://pbs.twimg.com/profile_banners/1366430884698546176/1684330404'
    const html = renderToStaticMarkup(
      <ChatMediaImage
        src={rawBanner}
        alt="Banner Image"
        initialStage="proxy"
      />
    )
    expect(html).toContain('data-load-stage="proxy"')
    expect(html).toContain('/api/proxy-image?url=https%3A%2F%2Fpbs.twimg.com%2Fprofile_banners%2F1366430884698546176%2F1684330404%2F1500x500')
    expect(html).toContain('alt="Banner Image"')
  })
})
