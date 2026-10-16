import { describe, expect, it } from 'vitest'
import { parseFileChangesFromProgress } from '../agentProgress'
import { activitiesFrom } from './agentActivity'
import type { AgentProgress } from '../types'

describe('immutable progress summaries', () => {
  it('reuses file summaries while a turn has no new events', () => {
    const progress: AgentProgress[] = []
    expect(parseFileChangesFromProgress(progress)).toBe(parseFileChangesFromProgress(progress))
  })
  it('reuses activity views across unrelated UI updates', () => {
    const progress: AgentProgress[] = [{ type: 'ToolStart', data: { action: 'read_file', input: { path: '/a.ts' } } }]
    expect(activitiesFrom(progress)).toBe(activitiesFrom(progress))
  })
  it('adds a completed file edit without changing the cached earlier snapshot', () => {
    const progress: AgentProgress[] = [{ type: 'ToolStart', data: { action: 'write_file', input: { path: '/a.ts', content: 'first\nsecond' } } }]
    const before = parseFileChangesFromProgress(progress)
    const next: AgentProgress[] = [...progress, { type: 'ToolEnd', data: { action: 'write_file', input: {}, result: 'ok' } }]
    expect(parseFileChangesFromProgress(next)).toEqual([{ path: '/a.ts', created: true, additions: 2, deletions: 0, hunks: [{ oldText: '', newText: 'first\nsecond' }] }])
    expect(before).toEqual([])
    expect(parseFileChangesFromProgress(progress)).toBe(before)
  })
  it('updates activity when a new progress snapshot arrives', () => {
    const progress: AgentProgress[] = [{ type: 'ToolStart', data: { action: 'read_file', input: { path: '/a.ts' } } }]
    const before = activitiesFrom(progress)
    const next: AgentProgress[] = [...progress, { type: 'ToolEnd', data: { action: 'read_file', input: { path: '/a.ts' }, result: 'content' } }]
    const after = activitiesFrom(next)
    expect(after).not.toBe(before)
    expect(before.items[0].state).not.toBe(after.items[0].state)
    expect(activitiesFrom(progress)).toBe(before)
  })
})
