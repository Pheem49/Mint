import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import SlashSuggestions from './SlashSuggestions'

const render = (message: string, suppressed = false) => renderToStaticMarkup(
  <SlashSuggestions message={message} onComplete={() => {}} anchorRef={{ current: null }} suppressed={suppressed} />,
)

describe('composer command suggestions', () => {
  it('finds a command by its description while retaining its executable token', () => {
    const html = render('/statistics')
    expect(html).toContain('/stats')
    expect(html).toContain('role="option"')
    expect(html).not.toContain('/clear')
  })
  it('shows token-prefix matches before description-only matches', () => {
    const html = render('/st')
    expect(html.indexOf('>/stats<')).toBeLessThan(html.indexOf('>/mods<'))
  })
  it('keeps a command argument as a usage hint instead of unrelated search results', () => {
    const html = render('/remember a durable fact')
    expect(html).toContain('/remember')
    expect(html).not.toContain('role="option"')
  })
  it('keeps CLI-only actions out of the graphical menu and hides it during history browsing', () => {
    const html = render('/')
    expect(html).toContain('/models')
    expect(html).not.toContain('/exit')
    expect(html).not.toContain('/theme')
    expect(render('/', true)).toBe('')
  })
})
