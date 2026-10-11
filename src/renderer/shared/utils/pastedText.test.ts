import { describe, expect, it } from 'vitest'
import { insertPastedText, pastedTextLabel } from './pastedText'

describe('long clipboard text on Desktop and Web', () => {
  it('collapses seven lines while retaining the complete submitted value', () => {
    const result = insertPastedText('before selection after', 7, 16, 'a\rb\rc\rd\re\rf\rg')
    expect(result.collapsed).toBe(true)
    expect(result.value).toBe('before a\nb\nc\nd\ne\nf\ng after')
    expect(result.text).toBe('a\nb\nc\nd\ne\nf\ng')
    expect(result.caret).toBe(20)
  })
  it('keeps six lines inline and normalizes mixed clipboard line endings', () => {
    const result = insertPastedText('', 0, 0, 'a\r\nb\rc\nd\ne\nf\r\n')
    expect(result.collapsed).toBe(false)
    expect(result.value).toBe('a\nb\nc\nd\ne\nf\n')
  })
  it('counts Unicode characters rather than UTF-16 units in the badge', () => {
    expect(pastedTextLabel('🙂\na\nb\nc\nd\ne\nf')).toBe('[Pasted Content 13 chars]')
  })
})
