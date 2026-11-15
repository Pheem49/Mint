import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import SourcesBlock, { groupSourcesByDomain } from './SourcesBlock'
import type { WebSearchSource } from '../../utils/agentActivity'

describe('SourcesBlock', () => {
  const mockSources: WebSearchSource[] = [
    {
      title: 'Nahida Cosplay Album 1',
      url: 'https://mitaku.net/album-1',
      domain: 'mitaku.net',
      faviconUrl: 'https://mitaku.net/favicon.ico',
    },
    {
      title: 'Nahida Cosplay Album 2',
      url: 'https://mitaku.net/album-2',
      domain: 'mitaku.net',
      faviconUrl: 'https://mitaku.net/favicon.ico',
    },
    {
      title: 'Mococo Cosplay',
      url: 'https://e-hentai.org/g/12345',
      domain: 'e-hentai.org',
      faviconUrl: 'https://e-hentai.org/favicon.ico',
    },
  ]

  it('groups sources by domain correctly', () => {
    const groups = groupSourcesByDomain(mockSources)
    expect(groups.length).toBe(2)
    expect(groups[0].domain).toBe('mitaku.net')
    expect(groups[0].displayDomain).toBe('mitaku')
    expect(groups[0].items.length).toBe(2)
    expect(groups[1].domain).toBe('e-hentai.org')
    expect(groups[1].displayDomain).toBe('e-hentai')
    expect(groups[1].items.length).toBe(1)
  })

  it('renders domain group cards with extra count badge', () => {
    const html = renderToStaticMarkup(<SourcesBlock sources={mockSources} />)
    expect(html).toContain('Sources')
    expect(html).toContain('mitaku')
    expect(html).toContain('+1')
    expect(html).toContain('e-hentai')
    expect(html).toContain('extra-badge-btn')
  })

  it('returns null when sources list is empty', () => {
    const html = renderToStaticMarkup(<SourcesBlock sources={[]} />)
    expect(html).toBe('')
  })
})
