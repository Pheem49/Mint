export interface SessionIdentity {
  chatId: string
  generation: number
}

export function matchesActiveSession(origin: SessionIdentity, active: SessionIdentity): boolean {
  return origin.chatId === active.chatId && origin.generation === active.generation
}

export interface RunIdentity {
  chatId: string
  sessionGeneration: number
  runGeneration: number
}

export function matchesActiveRun(origin: RunIdentity, active: RunIdentity): boolean {
  return origin.chatId === active.chatId
    && origin.sessionGeneration === active.sessionGeneration
    && origin.runGeneration === active.runGeneration
}

export function mergeVisibleHistory<T extends { id: number }>(
  current: T[], incoming: T[], isCurrentSession: boolean,
): T[] {
  if (!isCurrentSession) return current
  const byId = new Map([...current, ...incoming].map((interaction) => [interaction.id, interaction]))
  return Array.from(byId.values()).sort((a, b) => a.id - b.id)
}

export function attachActivityToTurn<T extends { id: number; agentActivity?: A }, A>(
  interactions: T[], turnId: number, activity: A,
): T[] {
  return interactions.map((interaction) => interaction.id === turnId
    ? { ...interaction, agentActivity: activity }
    : interaction)
}

export function visibleInteractionsDuringRun<T extends { id: number }>(
  interactions: T[], sending: boolean, sendingTurnId: number | null | undefined,
): T[] {
  if (!sending || sendingTurnId == null) return interactions
  return interactions.filter((interaction) => interaction.id !== sendingTurnId)
}
