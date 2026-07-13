/**
 * shared/agentProgress.ts
 * Shared utilities for AgentProgress events — used by both Desktop and Web renderers.
 * Source of truth: import from this file, not from the per-renderer copies.
 */
import type { AgentProgress, InteractionMemory, DiffHunk, FileChange } from './types'
export type { AgentProgress, InteractionMemory }

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

export function parseFileChangesFromProgress(progress: AgentProgress[]): FileChange[] {
  const changes = new Map<string, FileChange>()
  let activeEdit: { action: string; path: string; created: boolean; additions: number; deletions: number; hunks: DiffHunk[] } | null = null

  for (const event of progress || []) {
    if (event.type === 'ToolStart') {
      if (event.data.action === 'apply_patch') {
        const patch = (event.data.input as any)?.patch
        if (patch && typeof patch.path === 'string') {
          let additions = 0
          let deletions = 0
          const hunksList: DiffHunk[] = []
          const hunks = patch.hunks
          if (Array.isArray(hunks)) {
            for (const hunk of hunks) {
              const oldText = hunk?.oldText || ''
              const newText = hunk?.newText || ''
              const oldLines = oldText ? oldText.split('\n').length : 0
              const newLines = newText ? newText.split('\n').length : 0
              deletions += oldLines
              additions += newLines
              hunksList.push({ oldText, newText })
            }
          }
          activeEdit = {
            action: 'apply_patch',
            path: patch.path,
            created: false,
            additions,
            deletions,
            hunks: hunksList
          }
        }
      } else if (event.data.action === 'write_file') {
        const path = (event.data.input as any)?.path
        const fileContent = (event.data.input as any)?.file_content || ''
        if (typeof path === 'string') {
          const additions = fileContent ? fileContent.split('\n').length : 0
          activeEdit = {
            action: 'write_file',
            path,
            created: true,
            additions,
            deletions: 0,
            hunks: [{ oldText: '', newText: fileContent }]
          }
        }
      } else {
        activeEdit = null
      }
    } else if (event.type === 'ToolEnd') {
      if (activeEdit && (event.data.action === 'apply_patch' || event.data.action === 'write_file')) {
        const isError = typeof event.data.result === 'string' && event.data.result.startsWith('Error:')
        if (!isError) {
          try {
            const applied = JSON.parse(event.data.result)
            const appliedPaths = Array.isArray(applied) ? applied.map(item => item?.path).filter(Boolean) : [activeEdit.path]
            
            for (const path of appliedPaths) {
              const existing = changes.get(path)
              if (existing) {
                existing.additions += activeEdit.additions
                existing.deletions += activeEdit.deletions
                existing.hunks.push(...activeEdit.hunks)
              } else {
                changes.set(path, {
                  path,
                  created: activeEdit.created,
                  additions: activeEdit.additions,
                  deletions: activeEdit.deletions,
                  hunks: [...activeEdit.hunks]
                })
              }
            }
          } catch (e) {
            const path = activeEdit.path
            const existing = changes.get(path)
            if (existing) {
              existing.additions += activeEdit.additions
              existing.deletions += activeEdit.deletions
              existing.hunks.push(...activeEdit.hunks)
            } else {
              changes.set(path, {
                path,
                created: activeEdit.created,
                additions: activeEdit.additions,
                deletions: activeEdit.deletions,
                hunks: [...activeEdit.hunks]
              })
            }
          }
        }
      }
      activeEdit = null
    }
  }

  return Array.from(changes.values())
}
