import { describe, expect, it } from 'vitest'
import { renderToStaticMarkup } from 'react-dom/server'
import { ChatCodeBlock } from './ChatCodeBlock'

describe('ChatCodeBlock', () => {
  it('hides line numbers for short snippets of 5 lines or fewer', () => {
    const code5Lines = [
      '# ImageMagick snippet',
      'mkdir -p optimized',
      'for img in *.jpg; do',
      '  convert "$img" -resize 2000x "$img"',
      'done',
    ].join('\n')

    const markup = renderToStaticMarkup(<ChatCodeBlock code={code5Lines} language="bash" />)
    expect(markup).not.toContain('chat-code-line-num')
  })

  it('shows line numbers for longer code blocks with more than 5 lines', () => {
    const code6Lines = [
      'def fn():',
      '    line1 = 1',
      '    line2 = 2',
      '    line3 = 3',
      '    line4 = 4',
      '    return line1 + line4',
    ].join('\n')

    const markup = renderToStaticMarkup(<ChatCodeBlock code={code6Lines} language="python" />)
    expect(markup).toContain('chat-code-line-num')
    expect(markup).toContain('chat-code-toggle-lines-btn')
  })
})
