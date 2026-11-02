import { describe, expect, it } from 'vitest'
import {
  extractRenderableStreamPrefix,
  splitProgressiveStreamUnits,
  createProgressiveStreamPacer,
} from './progressiveStream'

describe('progressiveStream (Line-by-Line + Block-Aware)', () => {
  it('releases normal paragraphs and regular code blocks line by line while holding incomplete tables and unclosed UI cards', () => {
    const partialTable = 'Intro paragraph\n| Name | Role |\n| --- | --- |\n| Alice | Admin |\n'
    expect(extractRenderableStreamPrefix(partialTable, false)).toBe('Intro paragraph')
    expect(extractRenderableStreamPrefix(partialTable, true)).toBe(partialTable.trimEnd())

    const completedTable = `${partialTable}Summary after table\n`
    expect(extractRenderableStreamPrefix(completedTable, false)).toBe(completedTable.trimEnd())

    const partialUiCard = 'Before card\n```ui-card\n[{"title": "Fast", "subtitle": "Card"}]\n'
    expect(extractRenderableStreamPrefix(partialUiCard, false)).toBe('Before card')

    const closedUiCard = `${partialUiCard}\`\`\`\nAfter card\n`
    expect(extractRenderableStreamPrefix(closedUiCard, false)).toBe(closedUiCard.trimEnd())

    const bareFence = 'Before code\n```ts\n'
    expect(extractRenderableStreamPrefix(bareFence, false)).toBe('Before code')

    const partialCode = 'Before code\n```ts\nconst x = 1\n'
    expect(extractRenderableStreamPrefix(partialCode, false)).toBe('Before code\n```ts\nconst x = 1')

    const unlabeledUiCard = 'Before card\n```\nui-card [\n  {"title": "Fast"}\n]\n'
    expect(extractRenderableStreamPrefix(unlabeledUiCard, false)).toBe('Before card')
    expect(extractRenderableStreamPrefix(`${unlabeledUiCard}\`\`\`\n`, false)).toBe(
      `${unlabeledUiCard}\`\`\``,
    )
  })

  it('splits complete markdown into line units for text/code and atomic block units for tables and JSON/Mermaid cards', () => {
    const doc = [
      '# Title',
      'First line',
      '| A | B |',
      '| - | - |',
      '| 1 | 2 |',
      '```python',
      'print(42)',
      '```',
      '```ui-grid',
      '[{"title": "Item"}]',
      '```',
      'Done',
    ].join('\n')

    expect(splitProgressiveStreamUnits(doc, true)).toEqual([
      '# Title',
      'First line',
      '| A | B |\n| - | - |\n| 1 | 2 |',
      '```python\nprint(42)',
      '```',
      '```ui-grid\n[{"title": "Item"}]\n```',
      'Done',
    ])
  })

  it('paces deltas and authoritative final text smoothly in order without leaking raw JSON mid-card', () => {
    const revealed: string[] = []
    const pacer = createProgressiveStreamPacer((text) => revealed.push(text))

    pacer.pushDelta('Line 1\n```ui-card\n[{"title":"Box"}]\n')
    expect(pacer.step()).toBe('Line 1')
    expect(pacer.step()).toBeNull()

    pacer.pushDelta('```\nLine 2\n')
    expect(pacer.step()).toBe('Line 1\n```ui-card\n[{"title":"Box"}]\n```')
    expect(pacer.step()).toBe('Line 1\n```ui-card\n[{"title":"Box"}]\n```\nLine 2')
    expect(pacer.isDone()).toBe(false)

    pacer.finish('Line 1\n```ui-card\n[{"title":"Box"}]\n```\nLine 2\nFinal tail')
    expect(pacer.flushAll()).toBe('Line 1\n```ui-card\n[{"title":"Box"}]\n```\nLine 2\nFinal tail')
    expect(pacer.isDone()).toBe(true)
    expect(revealed[revealed.length - 1]).toBe(
      'Line 1\n```ui-card\n[{"title":"Box"}]\n```\nLine 2\nFinal tail',
    )
  })
})
