/**
 * shared/agentProgress.ts
 * Shared utilities for AgentProgress events — used by both Desktop and Web renderers.
 * Source of truth: import from this file, not from the per-renderer copies.
 */
import type { AgentProgress, InteractionMemory, DiffHunk, FileChange } from './types'
export type { AgentProgress, InteractionMemory }

export function isInternalCot(text: string): boolean {
  const trimmed = text.trim()
  if (trimmed.startsWith('<think>') || trimmed.includes('</think>')) {
    return true
  }
  const lineCount = trimmed.split('\n').length
  const charCount = trimmed.length
  if (trimmed.includes('\n\n') || lineCount > 2 || charCount > 180) {
    return true
  }
  const lower = trimmed.toLowerCase()
  if (
    lower.startsWith('the user ') ||
    lower.startsWith("let's think") ||
    lower.startsWith('let me analyze') ||
    lower.startsWith('per rule') ||
    lower.startsWith('note the ') ||
    lower.includes('thinking process:')
  ) {
    return true
  }
  return false
}

/** Short inline agent notes (excluding long chain-of-thought). */
export function thoughtsFrom(progress: AgentProgress[]): string[] {
  return progress
    .filter((event) => event.type === 'Thought')
    .map((event) => (event as Extract<AgentProgress, { type: 'Thought' }>).data.thought)
    .filter((t) => Boolean(t) && !isInternalCot(t))
}

/** Extended reasoning / chain-of-thought from the model's API thinking tokens or <think> tags. */
export function extendedThoughtsFrom(progress: AgentProgress[]): string[] {
  const extended: string[] = []
  for (const event of progress) {
    if (event.type === 'ThinkingDelta') {
      if (event.data.delta?.trim()) extended.push(event.data.delta)
    } else if (event.type === 'ExtendedThinking') {
      if (event.data.thought?.trim()) extended.push(event.data.thought)
    } else if (event.type === 'Thought' && event.data.thought && isInternalCot(event.data.thought)) {
      extended.push(event.data.thought)
    }
  }
  return extended
}

export function trimAgentProgress(progress: AgentProgress[], maxNonThought = 20): AgentProgress[] {
  if (progress.length === 0) return progress
  const keptNonThought = new Set(
    progress
      .filter((event) => event.type !== 'Thought' && event.type !== 'ThinkingDelta' && event.type !== 'ExtendedThinking')
      .slice(-maxNonThought),
  )
  return progress.filter(
    (event) =>
      event.type === 'Thought' ||
      event.type === 'ThinkingDelta' ||
      event.type === 'ExtendedThinking' ||
      keptNonThought.has(event),
  )
}

/** Fold live reasoning deltas into one replaceable record per model step. */
export function reduceLiveAgentProgress(
  current: AgentProgress[],
  event: AgentProgress,
): AgentProgress[] {
  if (event.type === 'ThinkingDelta') {
    const index = current.findIndex(
      (item) => item.type === 'ThinkingDelta' && item.data.id === event.data.id,
    )
    if (index < 0) return trimAgentProgress([...current, event])
    const next = current.slice()
    const previous = next[index] as Extract<AgentProgress, { type: 'ThinkingDelta' }>
    next[index] = {
      type: 'ThinkingDelta',
      data: {
        id: event.data.id,
        delta: `${previous.data.delta}${event.data.delta}`,
        elapsed_ms: event.data.elapsed_ms,
      },
    }
    return next
  }

  if (event.type === 'ExtendedThinking' && event.data.id) {
    const index = current.findIndex(
      (item) => item.type === 'ThinkingDelta' && item.data.id === event.data.id,
    )
    if (index >= 0) {
      const next = current.slice()
      next[index] = event
      return trimAgentProgress(next)
    }
  }
  return trimAgentProgress([...current, event])
}

export function reduceLiveAgentProgressBatch(
  current: AgentProgress[],
  events: AgentProgress[],
): AgentProgress[] {
  return events.reduce(reduceLiveAgentProgress, current)
}

/** Remove transport-only deltas before activity is stored in conversation history. */
export function compactAgentProgressForPersistence(progress: AgentProgress[]): AgentProgress[] {
  const completedIds = new Set(
    progress
      .filter((event): event is Extract<AgentProgress, { type: 'ExtendedThinking' }> => event.type === 'ExtendedThinking')
      .map((event) => event.data.id)
      .filter((id): id is string => Boolean(id)),
  )
  const result: AgentProgress[] = []
  const interrupted = new Map<string, Extract<AgentProgress, { type: 'ThinkingDelta' }>>()
  for (const event of progress) {
    if (event.type === 'ThinkingDelta') {
      if (!completedIds.has(event.data.id)) {
        const previous = interrupted.get(event.data.id)
        interrupted.set(event.data.id, previous
          ? {
              type: 'ThinkingDelta',
              data: {
                id: event.data.id,
                delta: `${previous.data.delta}${event.data.delta}`,
                elapsed_ms: event.data.elapsed_ms,
              },
            }
          : event)
      }
      continue
    }
    result.push(event)
  }
  for (const event of interrupted.values()) {
    result.push({
      type: 'ExtendedThinking',
      data: {
        id: event.data.id,
        thought: event.data.delta,
        elapsed_ms: event.data.elapsed_ms,
      },
    })
  }
  return result
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

  const recordChange = (
    path: string,
    isCreated: boolean,
    additions: number,
    deletions: number,
    hunks: DiffHunk[]
  ) => {
    const existing = changes.get(path)
    if (existing) {
      existing.additions += additions
      existing.deletions += deletions
      existing.hunks.push(...hunks)
      if (isCreated) existing.created = true
    } else {
      changes.set(path, {
        path,
        created: isCreated,
        additions,
        deletions,
        hunks: [...hunks]
      })
    }
  }

  for (const event of progress || []) {
    if (event.type === 'ToolStart') {
      if (event.data.action === 'apply_patch') {
        const patch = (event.data.input as any)?.patch
        if (patch && typeof patch.path === 'string') {
          let additions = 0
          let deletions = 0
          const hunksList: DiffHunk[] = []
          const hunks = patch.hunks
          let isAllNew = false
          if (Array.isArray(hunks)) {
            isAllNew = hunks.length > 0 && hunks.every((h: any) => !h?.oldText && Boolean(h?.newText))
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
            created: isAllNew && deletions === 0,
            additions,
            deletions,
            hunks: hunksList
          }
        }
      } else if (event.data.action === 'write_file' || event.data.action === 'note_write') {
        const input = (event.data.input as any) || {}
        let path = input.path || input.name || input.title || input.filePath || ''
        if (event.data.action === 'note_write' && path && !path.includes('/') && !path.startsWith('.config/')) {
          path = `.config/mint/notes/${path}`
        }
        const fileContent = input.fileContent ?? input.file_content ?? input.content ?? input.body ?? input.text ?? ''
        if (typeof path === 'string' && path.trim()) {
          const additions = fileContent ? fileContent.split('\n').length : 0
          activeEdit = {
            action: event.data.action,
            path: path.trim(),
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
      if (activeEdit && (event.data.action === 'apply_patch' || event.data.action === 'write_file' || event.data.action === 'note_write')) {
        const isError = typeof event.data.result === 'string' && (event.data.result.startsWith('Error:') || event.data.result.startsWith('Failed:'))
        if (!isError) {
          try {
            const applied = JSON.parse(event.data.result)
            if (Array.isArray(applied) && applied.length > 0) {
              for (const item of applied) {
                const path = item?.path || activeEdit.path
                const isCreated = typeof item?.created === 'boolean' ? item.created : activeEdit.created
                recordChange(path, isCreated, activeEdit.additions, activeEdit.deletions, activeEdit.hunks)
              }
            } else {
              recordChange(activeEdit.path, activeEdit.created, activeEdit.additions, activeEdit.deletions, activeEdit.hunks)
            }
          } catch {
            recordChange(activeEdit.path, activeEdit.created, activeEdit.additions, activeEdit.deletions, activeEdit.hunks)
          }
        }
      }
      activeEdit = null
    }
  }

  return Array.from(changes.values())
}
