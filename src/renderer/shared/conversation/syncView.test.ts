import { describe, expect, it } from 'vitest'
import { attachActivityToTurn, matchesActiveRun, matchesActiveSession, mergeVisibleHistory, visibleInteractionsDuringRun } from './syncView'

describe('shared conversation view reconciliation', () => {
  it('ignores history arriving out of order after A to B to C navigation', () => {
    const active = { chatId: 'C', generation: 2 }
    expect(matchesActiveSession({ chatId: 'A', generation: 0 }, active)).toBe(false)
    expect(matchesActiveSession({ chatId: 'B', generation: 1 }, active)).toBe(false)
    expect(matchesActiveSession(active, active)).toBe(true)
  })

  it('rejects late runs after A to B to A navigation or a newer run', () => {
    const original = { chatId: 'A', sessionGeneration: 0, runGeneration: 1 }
    expect(matchesActiveRun(original, original)).toBe(true)
    expect(matchesActiveRun(original, { chatId: 'B', sessionGeneration: 1, runGeneration: 2 })).toBe(false)
    expect(matchesActiveRun(original, { chatId: 'A', sessionGeneration: 2, runGeneration: 3 })).toBe(false)
    expect(matchesActiveRun(original, { chatId: 'A', sessionGeneration: 0, runGeneration: 2 })).toBe(false)
  })

  it('keeps the active session unchanged when an earlier session finishes', () => {
    const current = [{ id: 20, userText: 'session B' }]
    const fromA = [{ id: 10, userText: 'session A' }]
    expect(mergeVisibleHistory(current, fromA, false)).toBe(current)
    expect(mergeVisibleHistory(current, [{ id: 21, userText: 'new B' }], true))
      .toEqual([{ id: 20, userText: 'session B' }, { id: 21, userText: 'new B' }])
  })

  it('attaches agent activity by persisted turn ID when prompts are identical', () => {
    const turns = [
      { id: 10, userText: 'same prompt', agentActivity: [] as string[] },
      { id: 11, userText: 'same prompt', agentActivity: [] as string[] },
    ]
    const updated = attachActivityToTurn(turns, 10, ['tool call'])
    expect(updated[0].agentActivity).toEqual(['tool call'])
    expect(updated[1]).toBe(turns[1])
  })

  it('hides only the initiating turn, never a matching remote prompt', () => {
    const turns = [
      { id: 10, userText: 'same prompt' },
      { id: 11, userText: 'same prompt' },
    ]
    expect(visibleInteractionsDuringRun(turns, true, null)).toBe(turns)
    expect(visibleInteractionsDuringRun(turns, true, 11)).toEqual([turns[0]])
  })
})
