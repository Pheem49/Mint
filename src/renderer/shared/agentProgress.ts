/**
 * shared/agentProgress.ts
 * Shared utilities for AgentProgress events — used by both Desktop and Web renderers.
 * Source of truth: import from this file, not from the per-renderer copies.
 */
import type { AgentProgress, InteractionMemory } from './types'

export function thoughtsFrom(progress: AgentProgress[]): string[] {
  return progress
    .filter((event) => event.type === 'Thought')
    .map((event) => (event as Extract<AgentProgress, { type: 'Thought' }>).data.thought)
    .filter(Boolean)
}

export function trimAgentProgress(progress: AgentProgress[], maxNonThought = 20): AgentProgress[] {
  if (progress.length === 0) return progress
  const keptNonThought = new Set(
    progress.filter((event) => event.type !== 'Thought').slice(-maxNonThought),
  )
  return progress.filter((event) => event.type === 'Thought' || keptNonThought.has(event))
}

export function hasAgentToolActivity(progress: AgentProgress[]): boolean {
  return progress.some((event) => event.type === 'ToolStart' || event.type === 'ToolEnd')
}

export function mergeActivitySnapshots(
  current: Record<string, AgentProgress[]>,
  interactions: InteractionMemory[],
): Record<string, AgentProgress[]> {
  const merged = { ...current }
  for (const interaction of interactions) {
    const key = String(interaction.id)
    if (interaction.agentActivity?.length && !merged[key]) {
      merged[key] = interaction.agentActivity
    }
  }
  return merged
}

export function parseAgentActivity(value: unknown): AgentProgress[] | undefined {
  if (!Array.isArray(value)) return undefined
  return value as AgentProgress[]
}
