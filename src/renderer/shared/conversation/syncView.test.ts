import { describe, expect, it } from 'vitest'
import { visibleInteractionsDuringRun } from './syncView'

describe('shared conversation view reconciliation', () => {
  it('hides only the initiating turn, never a matching remote prompt', () => {
    const turns = [
      { id: 10, userText: 'same prompt' },
      { id: 11, userText: 'same prompt' },
    ]
    expect(visibleInteractionsDuringRun(turns, true, null)).toBe(turns)
    expect(visibleInteractionsDuringRun(turns, true, 11)).toEqual([turns[0]])
  })
})
