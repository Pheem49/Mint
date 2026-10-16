import { describe, expect, it } from 'vitest'
import { createFrameChangeTracker } from './liveTranslateFrames'

describe('live translation change detection', () => {
  const frame = () => new Uint8ClampedArray(160 * 90).fill(110)

  it('translates the first frame and skips identical frames', () => {
    const changed = createFrameChangeTracker()
    const first = frame()
    expect(changed(first)).toBe(true)
    expect(changed(frame())).toBe(false)
  })

  it('ignores a blinking caret but reacts to a changed line', () => {
    const changed = createFrameChangeTracker()
    const first = frame()
    expect(changed(first)).toBe(true)
    const caret = frame()
    caret.fill(220, 500, 508)
    expect(changed(caret)).toBe(false)

    const changedLine = frame()
    changedLine.fill(220, 500, 530)
    expect(changed(changedLine)).toBe(true)
  })

  it('ignores a moving game background while detecting a new subtitle', () => {
    const changed = createFrameChangeTracker()
    const first = frame()
    expect(changed(first)).toBe(true)

    const moving = frame()
    moving.fill(180, 0, 4000)
    expect(changed(moving)).toBe(true)

    const movingAgain = frame()
    expect(changed(movingAgain)).toBe(false)

    const newSubtitle = frame()
    newSubtitle.fill(220, 5000, 5080)
    newSubtitle.fill(180, 0, 4000)
    expect(changed(newSubtitle)).toBe(true)
  })
})
