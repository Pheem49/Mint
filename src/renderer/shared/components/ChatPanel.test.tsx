import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import ChatPanel from './ChatPanel'
import { chatPanelFixture } from '../../tests/chatPanelFixture'
import { installRendererPlatform } from '../platform'
import * as webAdapter from '../../src-web/tauri'

describe('Web Sources entry point', () => {
  it('shows Sources beside Tools in the real web chat header', () => {
    installRendererPlatform(webAdapter)
    const html = renderToStaticMarkup(createElement(ChatPanel, { ...chatPanelFixture(), showSessionSources: true } as any))
    expect(html).toContain('aria-label="Open session sources"')
    expect(html.indexOf('Open session sources')).toBeLessThan(html.indexOf('Open tools panel'))
  })
  it('leaves Desktop Sources in its existing titlebar', () => {
    installRendererPlatform(webAdapter)
    const html = renderToStaticMarkup(createElement(ChatPanel, { ...chatPanelFixture(), showSessionSources: false } as any))
    expect(html).not.toContain('Open session sources')
  })
})
