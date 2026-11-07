import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import ChatModeSelector from './ChatModeSelector'

describe('ChatModeSelector', () => {
  it('renders Agent mode trigger when agentMode is true and planMode is false', () => {
    const html = renderToStaticMarkup(
      createElement(ChatModeSelector, {
        agentMode: true,
        planMode: false,
        onSetAgentMode: () => {},
        onSetPlanMode: () => {},
      })
    )
    expect(html).toContain('id="chat-mode-btn"')
    expect(html).toContain('is-agent')
    expect(html).toContain('Agent')
  })

  it('renders Plan mode trigger when planMode is true', () => {
    const html = renderToStaticMarkup(
      createElement(ChatModeSelector, {
        agentMode: true,
        planMode: true,
        onSetAgentMode: () => {},
        onSetPlanMode: () => {},
      })
    )
    expect(html).toContain('is-plan')
    expect(html).toContain('Plan')
  })

  it('renders Chat mode trigger when agentMode is false', () => {
    const html = renderToStaticMarkup(
      createElement(ChatModeSelector, {
        agentMode: false,
        planMode: false,
        onSetAgentMode: () => {},
      })
    )
    expect(html).toContain('is-chat')
    expect(html).toContain('Chat')
  })
})
