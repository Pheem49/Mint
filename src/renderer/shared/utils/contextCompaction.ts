import type { AgentProgress } from '../types'

export function activeCompactionFrom(progress: AgentProgress[]) {
  const pending = new Map<string, { count: number; data: Extract<AgentProgress, { type: 'ContextCompaction' }>['data'] }>()
  let cycleIndex = 0
  for (let index = 0; index < progress.length; index++) {
    const event = progress[index]
    if (event.type !== 'ContextCompaction') continue
    const scope = event.data.subagent ?? ''
    if (event.data.status === 'started') {
      if (pending.size === 0) cycleIndex = index
      pending.set(scope, { count: (pending.get(scope)?.count ?? 0) + 1, data: event.data })
    } else {
      const entry = pending.get(scope)
      if (entry && entry.count > 1) entry.count--
      else pending.delete(scope)
    }
  }
  const active = pending.values().next().value
  return active ? { ...active.data, index: cycleIndex } : null
}
