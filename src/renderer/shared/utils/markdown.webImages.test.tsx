import { describe, expect, it } from 'vitest'
import { renderToStaticMarkup } from 'react-dom/server'
import { renderFormattedMessage } from './markdown'
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
})
