export type Status = 'queued' | 'thinking' | 'working' | 'responding' | 'waiting' | 'completed' | 'failed' | 'interrupted'
export type Turn = { chatId: string; turnId: number; seq: number; status: Status; tool: string | null; text: string; prompt: string }
export type Session = { chatId: string; title: string; workspace: string | null }
export type Host = { hostId: string; name: string; identity?: string | null; surface?: 'desktop' | 'web' | 'tui' | null; speech?: boolean }
export type Event = { type: 'snapshot'; hostId: string; version: number; chatId?: string | null; sessions: Session[]; turns: Turn[] } | { type: 'update'; hostId: string; turn: Turn } | { type: 'accepted'; requestId: string; turnId: number | null } | { type: 'error'; requestId: string | null; message: string }
export function terminal(status: Status) { return ['completed', 'failed', 'interrupted'].includes(status) }
export function applyTurn(turns: Turn[], turn: Turn, host: string, selected: string): Turn[] {
  if (host !== selected) return turns
  const previous = turns.find(t => t.chatId === turn.chatId && t.turnId === turn.turnId)
  if (previous && previous.seq >= turn.seq) return turns
  return [...turns.filter(t => t !== previous), turn].slice(-256)
}
export function activeTurn(turns: Turn[], chat: string): Turn | undefined {
  const session = turns.filter(t => t.chatId === chat).sort((a,b) => a.turnId-b.turnId)
  return session.find(t => !terminal(t.status) && t.status !== 'queued') ?? session.find(t => !terminal(t.status)) ?? session.at(-1)
}
export function modelExpression(manual: number, status: Status | 'idle'): number {
  return manual >= 0 ? manual : status === 'failed' ? 1 : 0
}
export function characterStatus(status: Status | 'idle', online: boolean, sessionExists: boolean): Status | 'idle' { return online && sessionExists ? status : 'idle' }

/** Preserve a live choice; recover only one matching executable/launch directory. */
export function recoverHost(hosts: Host[], hostId: string, identity: string, knownPeers: string[] = []): Host | undefined {
  const current = hosts.find(host => host.hostId === hostId)
  if (current) return current
  if (!hostId) return hosts.length === 1 ? hosts[0] : undefined
  if (!identity) return undefined
  const matches = hosts.filter(host => host.identity === identity && !knownPeers.includes(host.hostId))
  return matches.length === 1 ? matches[0] : undefined
}

export function handoffState(online: boolean, sessionExists: boolean, surface: Host['surface']) {
  if (!online) return 'offline'
  if (!sessionExists) return 'missing'
  if (!surface) return 'unsupported'
  return 'ready'
}
