import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import WorkspacePreviewActions from './WorkspacePreviewActions'
import { ArtifactPreviewPanel, detectArtifactType } from './ArtifactPreviewPanel'

function renderActions(name: string, kind: 'file' | 'directory' = 'file', pending = false) {
  return renderToStaticMarkup(<WorkspacePreviewActions entry={{ name, path: `nested/${name}`, kind, children: [] }} pending={pending} onPreview={() => {}} onOpenHtml={() => {}} />)
}

describe('workspace preview menu', () => {
  for (const name of ['README.md', 'notes.MARKDOWN', 'index.html', 'page.HTM', 'photo.PNG', 'cat.jpeg', 'animation.gif', 'icon.svg', 'photo.webp', 'icon.ico', 'photo.jpg']) {
    it(`offers an in-app preview for ${name}`, () => { expect(renderActions(name)).toContain('Preview in Mint') })
  }
  it('offers browser launch only for HTML files', () => {
    expect(renderActions('index.html')).toContain('Open in Browser')
    for (const name of ['README.md', 'photo.png']) expect(renderActions(name)).not.toContain('Open in Browser')
  })
  it('keeps directories out of previews and provides a menu alternative to dragging any file', () => {
    expect(renderActions('guide.md', 'directory')).toBe('')
    expect(renderActions('script.rs')).toContain('Preview in Mint')
    expect(renderActions('archive.zip')).toContain('Preview in Mint')
  })
  it('disables preview actions while another operation is pending', () => { expect(renderActions('README.md', 'file', true)).toContain('disabled=""') })
})

it('uses a served HTML URL in the Preview iframe so relative CSS and scripts can load', () => {
  const html = renderToStaticMarkup(<ArtifactPreviewPanel artifact={{ path: 'site/index.html', content: '<h1>Hello</h1>', url: 'http://127.0.0.1:12345/site/index.html' }} onClose={() => {}} />)
  expect(html).toContain('src="http://127.0.0.1:12345/site/index.html"')
  expect(html).not.toContain('srcDoc=')
  expect(html).toContain('allow-same-origin')
})

it('renders images fitted inside Preview without a binary code tab and with format chip', () => {
  expect(detectArtifactType('assets/PHOTO.PNG')).toBe('image')
  const html = renderToStaticMarkup(<ArtifactPreviewPanel artifact={{ path: 'assets/PHOTO.PNG', url: 'http://127.0.0.1:12345/assets/PHOTO.PNG' }} onClose={() => {}} />)
  expect(html).toContain('src="http://127.0.0.1:12345/assets/PHOTO.PNG"')
  expect(html).toContain('object-fit:contain')
  expect(html).not.toMatch(/>Code<\/button>/)
  expect(html).toContain('PNG')

  const webpHtml = renderToStaticMarkup(<ArtifactPreviewPanel artifact={{ path: 'images/caster.webp', url: 'http://127.0.0.1:12345/caster.webp' }} onClose={() => {}} />)
  expect(webpHtml).toContain('WEBP')
})

it('identifies the browser action as Mint Browser instead of a generic popup', () => {
  const html = renderToStaticMarkup(<ArtifactPreviewPanel artifact={{ path: 'site/index.html', url: 'http://127.0.0.1:12345/site/index.html' }} onClose={() => {}} />)
  expect(html).toContain('aria-label="Open in Mint Browser (Mint Auto)"')
})

it('renders pure code files in code mode with language chip and without preview toggle or browser button', () => {
  expect(detectArtifactType('scripts/fetch_api.py')).toBe('code')
  const html = renderToStaticMarkup(
    <ArtifactPreviewPanel
      artifact={{ path: 'scripts/fetch_api.py', content: 'import json\nprint("hello")' }}
      onClose={() => {}}
    />,
  )
  expect(html).not.toMatch(/>Preview<\/button>/)
  expect(html).not.toMatch(/>Code<\/button>/)
  expect(html).toContain('Python')
  expect(html).toContain('import')
  expect(html).toContain('json')
  expect(html).toContain('hello')
  expect(html).not.toContain('aria-label="Open in Mint Browser (Mint Auto)"')
})

it('renders dual-mode toggles for markdown, svg, and html without showing browser button for non-html', () => {
  const mdHtml = renderToStaticMarkup(
    <ArtifactPreviewPanel artifact={{ path: 'docs/README.md', content: '# Documentation' }} onClose={() => {}} />,
  )
  expect(mdHtml).toMatch(/>Preview<\/button>/)
  expect(mdHtml).toMatch(/>Code<\/button>/)
  expect(mdHtml).not.toContain('aria-label="Open in Mint Browser (Mint Auto)"')

  const svgHtml = renderToStaticMarkup(
    <ArtifactPreviewPanel artifact={{ path: 'icons/logo.svg', content: '<svg></svg>' }} onClose={() => {}} />,
  )
  expect(svgHtml).toMatch(/>Preview<\/button>/)
  expect(svgHtml).toMatch(/>Code<\/button>/)
  expect(svgHtml).not.toContain('aria-label="Open in Mint Browser (Mint Auto)"')

  const webHtml = renderToStaticMarkup(
    <ArtifactPreviewPanel artifact={{ path: 'index.html', content: '<h1>Hi</h1>' }} onClose={() => {}} />,
  )
  expect(webHtml).toMatch(/>Preview<\/button>/)
  expect(webHtml).toMatch(/>Code<\/button>/)
  expect(webHtml).toContain('aria-label="Open in Mint Browser (Mint Auto)"')
})
