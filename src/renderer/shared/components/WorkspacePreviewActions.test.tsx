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
  it('does not offer previews for directories or unsupported files', () => {
    expect(renderActions('guide.md', 'directory')).toBe('')
    expect(renderActions('script.rs')).toBe('')
  })
  it('disables preview actions while another operation is pending', () => { expect(renderActions('README.md', 'file', true)).toContain('disabled=""') })
})

it('uses a served HTML URL in the Preview iframe so relative CSS and scripts can load', () => {
  const html = renderToStaticMarkup(<ArtifactPreviewPanel artifact={{ path: 'site/index.html', content: '<h1>Hello</h1>', url: 'http://127.0.0.1:12345/site/index.html' }} onClose={() => {}} />)
  expect(html).toContain('src="http://127.0.0.1:12345/site/index.html"')
  expect(html).not.toContain('srcDoc=')
  expect(html).toContain('allow-same-origin')
})

it('renders images fitted inside Preview without a binary code tab', () => {
  expect(detectArtifactType('assets/PHOTO.PNG')).toBe('image')
  const html = renderToStaticMarkup(<ArtifactPreviewPanel artifact={{ path: 'assets/PHOTO.PNG', url: 'http://127.0.0.1:12345/assets/PHOTO.PNG' }} onClose={() => {}} />)
  expect(html).toContain('src="http://127.0.0.1:12345/assets/PHOTO.PNG"')
  expect(html).toContain('object-fit:contain')
  expect(html).not.toMatch(/>Code<\/button>/)
})

it('identifies the browser action as Mint Browser instead of a generic popup', () => {
  const html = renderToStaticMarkup(<ArtifactPreviewPanel artifact={{ path: 'site/index.html', url: 'http://127.0.0.1:12345/site/index.html' }} onClose={() => {}} />)
  expect(html).toContain('aria-label="Open in Mint Browser (Mint Auto)"')
})
