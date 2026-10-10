export function visibleInteractionsDuringRun<T extends { id: number }>(
  interactions: T[], sending: boolean, sendingTurnId: number | null | undefined,
): T[] {
  if (!sending || sendingTurnId == null) return interactions
  return interactions.filter((interaction) => interaction.id !== sendingTurnId)
}
