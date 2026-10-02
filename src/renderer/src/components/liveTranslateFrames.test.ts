import { describe, expect, it } from 'vitest'
import { hasMeaningfulFrameChange } from './liveTranslateFrames'

describe('live translation change detection', () => {
  const frame = () => new Uint8ClampedArray(160 * 90).fill(110)

  it('translates the first frame and skips identical frames', () => {
    const first = frame()
    expect(hasMeaningfulFrameChange(null, first)).toBe(true)
    expect(hasMeaningfulFrameChange(first, frame())).toBe(false)
  })

  it('ignores a blinking caret but reacts to a changed line', () => {
    const first = frame()
    const caret = frame()
    caret.fill(220, 500, 508)
    expect(hasMeaningfulFrameChange(first, caret)).toBe(false)

    const changedLine = frame()
    changedLine.fill(220, 500, 530)
    expect(hasMeaningfulFrameChange(first, changedLine)).toBe(true)
  })
})
