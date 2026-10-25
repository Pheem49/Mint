import { describe, expect, it } from 'vitest'
import { collectSessionSources } from './sessionSources'
import type { AgentProgress } from '../types'
import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import SessionSources from '../components/SessionSources'

const search = (url: string, title = 'Reference'): AgentProgress => ({ type: 'ToolEnd', data: { action: 'web_search', input: { query: 'example' }, result: `1. ${title}\nURL: ${url}\nA useful reference` } })
describe('session sources', () => {
  it('includes sent images and files alongside historical and live searches, deduplicating pages', () => {
    const result = collectSessionSources({ attachments: [
      { id: 1, interactionId: 3, name: 'photo.png', kind: 'image', mimeType: 'image/png', thumbnailDataUri: 'data:image/png;base64,aGVsbG8=' },
      { id: 2, interactionId: 7, name: 'notes.pdf', kind: 'file', mimeType: 'application/pdf', thumbnailDataUri: null },
    ], webSearchProgress: [search('https://example.com/docs')] }, [search('https://example.com/docs'), search('https://other.example/guide')])
    expect(result.map(source => source.name)).toEqual(['photo.png', 'notes.pdf', 'Reference', 'Reference'])
    expect(result[0]).toMatchObject({ kind: 'image', interactionId: 3, thumbnailDataUri: 'data:image/png;base64,aGVsbG8=' })
    expect(result[1]).toMatchObject({ kind: 'file', detail: 'PDF' })
    expect(result[2]).toMatchObject({ kind: 'web', url: 'https://example.com/docs', detail: 'example.com' })
  })
  it('does not turn non-web URLs or failed searches into source links', () => {
    expect(collectSessionSources({ attachments: [], webSearchProgress: [search('javascript:alert(1)'), search('file:///etc/passwd')] })).toEqual([])
    expect(collectSessionSources({ attachments: [], webSearchProgress: [search('https://example.com')] })).toHaveLength(1)
  })
  it('shows website favicons from the existing web-search source metadata', () => {
    const sources = collectSessionSources({ attachments: [], webSearchProgress: [search('https://open.spotify.com/track/example?secret=private', 'Song')] })
    expect(sources[0]).toMatchObject({ faviconUrl: 'https://www.google.com/s2/favicons?sz=32&domain=open.spotify.com' })
    const html = renderToStaticMarkup(createElement(SessionSources, { sources, loading: false, error: '', onRetry: () => {}, onOpenLibrary: () => {} }))
    expect(html).toContain('src="https://www.google.com/s2/favicons?sz=32&amp;domain=open.spotify.com"')
    expect(html).toContain('referrerPolicy="no-referrer"')
  })
  it('maps libraryItemId onto session sources and exposes tool preview for images', () => {
    const sources = collectSessionSources({
      attachments: [{
        id: 10,
        interactionId: 1,
        name: 'diagram.png',
        kind: 'image',
        mimeType: 'image/png',
        thumbnailDataUri: 'data:image/png;base64,abc',
        libraryItemId: 'lib-123',
      }],
      webSearchProgress: [],
    })
    expect(sources[0]).toMatchObject({
      id: 'attachment:10',
      kind: 'image',
      name: 'diagram.png',
      libraryItemId: 'lib-123',
      thumbnailDataUri: 'data:image/png;base64,abc',
    })
    const htmlWithPreview = renderToStaticMarkup(createElement(SessionSources, {
      sources,
      loading: false,
      error: '',
      onRetry: () => {},
      onOpenLibrary: () => {},
      onOpenArtifact: () => {},
    }))
    expect(htmlWithPreview).toContain('Open image preview in Tools')

    const htmlWithoutPreview = renderToStaticMarkup(createElement(SessionSources, {
      sources,
      loading: false,
      error: '',
      onRetry: () => {},
      onOpenLibrary: () => {},
    }))
    expect(htmlWithoutPreview).not.toContain('Open image preview in Tools')
    expect(htmlWithoutPreview).toContain('href="/library?item=lib-123"')
  })
})
