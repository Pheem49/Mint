import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import ChatPanel from './ChatPanel'
import { chatPanelFixture } from '../../../tests/chatPanelFixture'
import { installRendererPlatform } from '../../platform'
import * as webAdapter from '../../../src-web/tauri'

describe('Shared Tools entry point', () => {
  it('keeps Sources out of the chat header and offers Tools instead', () => {
    installRendererPlatform(webAdapter)
    const html = renderToStaticMarkup(createElement(ChatPanel, { ...chatPanelFixture(), tools: { modsPanelTarget: null, onOpenMods: () => {}, open: false, backgroundTaskCount: 0, onToggle: () => {} } } as any))
    expect(html).not.toContain('Open session sources')
    expect(html).toContain('Open tools panel')
  })
  it('does not duplicate Desktop’s titlebar Tools button', () => {
    installRendererPlatform(webAdapter)
    const html = renderToStaticMarkup(createElement(ChatPanel, chatPanelFixture() as any))
    expect(html).not.toContain('Open session sources')
  })
})
