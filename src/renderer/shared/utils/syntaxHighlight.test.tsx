import { describe, expect, it } from 'vitest'
import { renderToStaticMarkup } from 'react-dom/server'
import { renderHighlightedCode } from './syntaxHighlight'

describe('highlight reuse', () => {
  it('reuses unchanged code across rerenders and message remounts', () => {
    const first = renderHighlightedCode('const answer = 42', 'typescript', true)
    expect(renderHighlightedCode('const answer = 42', 'typescript', true)).toBe(first)
  })
  it('does not reuse stale text, language, or line-number settings', () => {
    const first = renderHighlightedCode('const answer = 42', 'typescript', true)
    expect(renderHighlightedCode('const answer = 43', 'typescript', true)).not.toBe(first)
    expect(renderHighlightedCode('const answer = 42', 'plaintext', true)).not.toBe(first)
    const withoutNumbers = renderHighlightedCode('const answer = 42', 'typescript', false)
    expect(renderToStaticMarkup(withoutNumbers)).not.toContain('chat-code-line-num')
  })
  it('evicts old entries instead of retaining every visited code block', () => {
    const first = renderHighlightedCode('const evicted = 0', 'typescript')
    for (let i = 0; i < 100; i++) renderHighlightedCode(`const other = ${i}`, 'typescript')
    expect(renderHighlightedCode('const evicted = 0', 'typescript')).not.toBe(first)
  })
})
