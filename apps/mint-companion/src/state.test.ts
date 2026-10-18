import { describe, it, expect } from 'vitest'
import { applyTurn, activeTurn, modelExpression, type Turn } from './state'
const turn = (chatId: string, turnId: number, seq: number, status: Turn['status']): Turn => ({ chatId, turnId, seq, status, text: '', prompt: '', tool: null })
describe('Companion state', () => {
  it('ignores another host and stale turn updates after reconnect', () => {
    let turns = applyTurn([], turn('a', 1, 3, 'completed'), 'host-a', 'host-a')
    turns = applyTurn(turns, turn('a', 1, 2, 'thinking'), 'host-a', 'host-a')
    turns = applyTurn(turns, turn('b', 2, 4, 'working'), 'host-b', 'host-a')
    expect(turns).toHaveLength(1)
    expect(turns[0].status).toBe('completed')
  })
  it('queued chat does not replace the running agent display', () => {
    const turns = [turn('a', 1, 1, 'working'), turn('a', 2, 2, 'queued'), turn('b', 3, 3, 'responding')]
    expect(activeTurn(turns, 'a')?.turnId).toBe(1)
    expect(activeTurn(turns, 'b')?.turnId).toBe(3)
  })
  it('manual expressions win over failures while Auto returns to normal', () => {
    expect(modelExpression(4, 'failed')).toBe(4)
    expect(modelExpression(-1, 'failed')).toBe(1)
    expect(modelExpression(-1, 'completed')).toBe(0)
  })
})
it('a disconnected or deleted conversation leaves the character idle', async () => {
  const { characterStatus } = await import('./state')
  expect(characterStatus('working', false, true)).toBe('idle')
  expect(characterStatus('responding', true, false)).toBe('idle')
  expect(characterStatus('working', true, true)).toBe('working')
})

// Reconnecting must never pick an unrelated or ambiguous process.
describe('host recovery', () => {
  it('recovers a restarted host while preserving an existing live selection', async () => {
    const { recoverHost } = await import('./state')
    const hosts = [{ hostId: 'new', name: 'Mint', identity: 'desktop:home' }]
    expect(recoverHost(hosts, 'old', 'desktop:home')?.hostId).toBe('new')
    expect(recoverHost([...hosts, { hostId: 'old', name: 'Mint', identity: 'desktop:home' }], 'old', 'desktop:home')?.hostId).toBe('old')
  })
  it('waits when a replacement is unrelated, ambiguous, or from an older host', async () => {
    const { recoverHost } = await import('./state')
    const host = { hostId: 'new', name: 'Mint', identity: 'desktop:home' }
    expect(recoverHost([host], 'old', 'cli:project')).toBeUndefined()
    expect(recoverHost([host, { ...host, hostId: 'other' }], 'old', host.identity)).toBeUndefined()
    expect(recoverHost([{ hostId: 'new', name: 'Mint' }], 'old', '')).toBeUndefined()
    expect(recoverHost([], 'old', host.identity)).toBeUndefined()
    expect(recoverHost([host], '', '')?.hostId).toBe('new')
  })
})

it('does not mistake a previously running sibling for the restarted selection', async () => {
  const { recoverHost } = await import('./state')
  const sibling = { hostId: 'sibling', name: 'Mint', identity: 'desktop:home' }
  expect(recoverHost([sibling], 'old', 'desktop:home', ['sibling'])).toBeUndefined()
  expect(recoverHost([{ ...sibling, hostId: 'restarted' }], 'old', 'desktop:home', ['sibling'])?.hostId).toBe('restarted')
})

it('only enables conversation handoff for an online, existing session and supported host', async () => {
  const { handoffState } = await import('./state')
  expect(handoffState(false, true, 'desktop')).toBe('offline')
  expect(handoffState(true, false, 'web')).toBe('missing')
  expect(handoffState(true, true, undefined)).toBe('unsupported')
  expect(handoffState(true, true, 'tui')).toBe('ready')
})
