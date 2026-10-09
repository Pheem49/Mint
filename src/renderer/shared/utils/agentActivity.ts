/**
 * shared/utils/agentActivity.ts
 * Agent activity parsing and transformation logic.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import type { AgentProgress } from '../types'
import { isInternalCot } from '../agentProgress'

export interface AgentActivity {
  label: string
  target: string
  kind: 'file' | 'folder' | 'search' | 'terminal' | 'tool' | 'calc'
  state: 'active' | 'done' | 'error' | 'retry'
  action?: string
  callId?: string
  /** Raw ToolEnd output text, shown when the row is expanded in the UI. */
  result?: string
}

export interface AgentActivityGroup {
  id: string
  action: string
  label: string
  kind: AgentActivity['kind']
  title: string
  count: number
  state: 'active' | 'done' | 'error' | 'retry'
  items: AgentActivity[]
}

export type TimelineItem =
  | { id: string; kind: 'activity'; activity: AgentActivity }
  | { id: string; kind: 'group'; group: AgentActivityGroup }
  | { id: string; kind: 'thought'; thought: string }
  | { id: string; kind: 'extendedThinking'; thought: string; streaming?: boolean; elapsedMs?: number }

export interface AgentActivityView {
  summary: string
  items: AgentActivity[]
  groups?: AgentActivityGroup[]
  /** Unified chronological list of thoughts, extended thinking, and tool activities. */
  timeline?: TimelineItem[]
}

function activityDetail(input: Record<string, unknown>, key: string): string {
  const value = input[key]
  return typeof value === 'string' && value.trim() ? value : ''
}

export function formatActivityTarget(value: string): string {
  const compact = value.replace(/^\/home\/([^/]+)/, '~')
  return compact || 'workspace'
}

export function activityKind(action: string, target: string): AgentActivity['kind'] {
  if (['search_code', 'semantic_search', 'knowledge_search', 'web_search', 'image_search', 'memory_recall', 'find_definition', 'find_references'].includes(action)) return 'search'
  if (['run_shell', 'verify', 'run_tests', 'run_typecheck', 'run_linter'].includes(action)) return 'terminal'
  if (['list_files', 'detect_project'].includes(action)) return 'folder'
  if (['read_file', 'symbols', 'read_diagnostics', 'git_diff', 'apply_patch', 'write_file', 'note_write', 'view_image'].includes(action)) return 'file'
  if (['calculation', 'calculator'].includes(action)) return 'calc'
  return target.includes('/') && !/\.[^/]+$/.test(target) ? 'folder' : 'tool'
}

export function describeTool(action: string, input: Record<string, unknown>): AgentActivity {
  const path = activityDetail(input, 'path')
  const query =
    activityDetail(input, 'query') ||
    activityDetail(input, 'q') ||
    activityDetail(input, 'keyword') ||
    activityDetail(input, 'search') ||
    activityDetail(input, 'prompt') ||
    activityDetail(input, 'searchTerm') ||
    activityDetail(input, 'search_term')
  const command = activityDetail(input, 'command') || activityDetail(input, 'cmd')
  const name = activityDetail(input, 'name')
  const tool = activityDetail(input, 'tool')
  const symbol = activityDetail(input, 'symbol') || activityDetail(input, 'ticker')
  const filter = activityDetail(input, 'filter')
  const expression = activityDetail(input, 'expression') || activityDetail(input, 'expr') || activityDetail(input, 'math')
  const city = activityDetail(input, 'city') || activityDetail(input, 'location') || activityDetail(input, 'place')
  const url = activityDetail(input, 'url')
  const selector = activityDetail(input, 'selector')

  const fallbackTarget =
    action === 'web_search' || action === 'search_code' || action === 'knowledge_search'
      ? (query || '(empty query)')
      : action === 'read_file' || action === 'write_file'
        ? (path || '(empty path)')
        : action === 'find_definition' || action === 'find_references'
          ? (symbol || '(empty symbol)')
          : action === 'run_tests'
            ? (filter || '(all tests)')
            : action === 'run_typecheck' || action === 'run_linter'
              ? (path || 'workspace')
              : action === 'run_shell'
                ? (command || '(empty command)')
                : action === 'calculation'
                  ? (expression || '(math expression)')
                  : action === 'weather'
                    ? (city || '(location)')
                    : action === 'stock'
                      ? (query || symbol || '(symbol)')
                      : action === 'browser_open'
                        ? (url || '(url)')
                        : action === 'browser_click'
                          ? (selector || '(element)')
                          : action.replaceAll('_', ' ')

  const rawTarget = path || query || command || expression || symbol || city || url || selector || filter || name || tool || fallbackTarget

  // Append line range for read_file when startLine / endLine are available
  let target = rawTarget
  if (action === 'read_file' && path) {
    const startLine =
      typeof input.startLine === 'number'
        ? input.startLine
        : typeof input.start_line === 'number'
          ? input.start_line
          : undefined
    const endLine =
      typeof input.endLine === 'number'
        ? input.endLine
        : typeof input.end_line === 'number'
          ? input.end_line
          : undefined
    if (startLine !== undefined && endLine !== undefined) {
      target = `${rawTarget} #L${startLine}-${endLine}`
    } else if (startLine !== undefined) {
      target = `${rawTarget} #L${startLine}`
    }
  }

  return {
    // Raw tool/action identifier, matching how the CLI labels activity (e.g. "[web_search]").
    label: action === 'read_file' && activityDetail(input, 'skill_name')
      ? `Reading skill: ${activityDetail(input, 'skill_name')}` : action,
    target: formatActivityTarget(target),
    kind: activityKind(action, target),
    state: 'active',
    action,
  }
}

export function activitySummary(items: AgentActivity[]): string {
  const files = new Set<string>()
  const folders = new Set<string>()
  for (const item of items) {
    if (item.kind === 'file') files.add(item.target)
    if (item.kind === 'folder') folders.add(item.target)
  }
  const parts = [
    files.size ? `${files.size} ${files.size === 1 ? 'file' : 'files'}` : '',
    folders.size ? `${folders.size} ${folders.size === 1 ? 'folder' : 'folders'}` : '',
  ].filter(Boolean)
  return parts.length ? `Exploring ${parts.join(', ')}` : 'Working through task'
}

export function groupActivities(items: AgentActivity[]): AgentActivityGroup[] {
  if (items.length === 0) return []
  const groups: AgentActivityGroup[] = []
  let currentGroup: AgentActivityGroup | null = null

  for (let i = 0; i < items.length; i++) {
    const item = items[i]
    const actionKey = item.action || item.label

    if (!currentGroup || currentGroup.action !== actionKey) {
      if (currentGroup) {
        groups.push(finalizeGroup(currentGroup))
      }
      currentGroup = {
        id: `group-${i}-${actionKey}`,
        action: actionKey,
        label: item.label,
        kind: item.kind,
        title: item.target,
        count: 1,
        state: item.state,
        items: [item],
      }
    } else {
      currentGroup.items.push(item)
      currentGroup.count += 1
      // Update aggregated state
      if (item.state === 'active') {
        currentGroup.state = 'active'
      } else if (item.state === 'error' && currentGroup.state !== 'retry' && currentGroup.state !== 'done') {
        currentGroup.state = 'error'
      } else if (item.state === 'retry' && currentGroup.state !== 'done') {
        currentGroup.state = 'retry'
      }
    }
  }

  if (currentGroup) {
    groups.push(finalizeGroup(currentGroup))
  }

  return groups
}

function finalizeGroup(group: AgentActivityGroup): AgentActivityGroup {
  const count = group.items.length
  if (count === 1) {
    group.title = group.items[0].target
    group.state = group.items[0].state
    return group
  }

  const hasActive = group.items.some((it) => it.state === 'active')
  const hasDone = group.items.some((it) => it.state === 'done')
  const hasError = group.items.some((it) => it.state === 'error')
  const hasRetry = group.items.some((it) => it.state === 'retry')

  // If any item is actively executing, the group is active
  // If at least one item succeeded, the group task as a whole succeeded (partial success is success)
  // Only mark the entire group as error if NO items succeeded and an error occurred
  group.state = hasActive
    ? 'active'
    : hasDone
      ? 'done'
      : hasRetry
        ? 'retry'
        : hasError
          ? 'error'
          : 'done'

  const action = group.action
  const doneCount = group.items.filter((it) => it.state === 'done').length
  if (action === 'calculation') {
    group.title = group.state === 'active'
      ? `Calculating expressions (${doneCount}/${count})`
      : `Calculated ${count} expressions`
  } else if (action === 'read_file') {
    group.title = group.state === 'active'
      ? `Reading files (${doneCount}/${count})`
      : `Read ${count} files`
  } else if (action === 'write_file') {
    group.title = group.state === 'active'
      ? `Writing files (${doneCount}/${count})`
      : `Wrote ${count} files`
  } else if (action === 'search_code' || action === 'semantic_search') {
    group.title = `Searched codebase (${count} queries)`
  } else if (action === 'web_search') {
    group.title = `Searched web (${count} queries)`
  } else {
    const readableAction = action.replaceAll('_', ' ')
    group.title = `Executed ${count} ${readableAction}`
  }

  return group
}

export function activitiesFrom(progress: AgentProgress[]): AgentActivityView {
  const activities: AgentActivity[] = []
  const timeline: TimelineItem[] = []
  let currentGroup: AgentActivityGroup | null = null
  let nextId = 0

  const flushGroup = () => {
    if (!currentGroup) return
    const finalized = finalizeGroup(currentGroup)
    if (finalized.count > 1) {
      timeline.push({ id: finalized.id, kind: 'group', group: finalized })
    } else if (finalized.items.length === 1) {
      timeline.push({ id: finalized.id, kind: 'activity', activity: finalized.items[0] })
    }
    currentGroup = null
  }

  for (const event of progress) {
    if (event.type === 'ToolStart') {
      const item = describeTool(event.data.action, event.data.input)
      item.callId = event.data.callId
      activities.push(item)

      // Keep each skill's lifecycle visible instead of hiding it in a files group.
      if (event.data.action === 'read_file' && activityDetail(event.data.input, 'skill_name')) {
        flushGroup()
        timeline.push({ id: `skill-${nextId++}`, kind: 'activity', activity: item })
        continue
      }

      const actionKey = item.action || item.label
      if (!currentGroup || currentGroup.action !== actionKey) {
        flushGroup()
        currentGroup = {
          id: `timeline-group-${nextId++}-${actionKey}`,
          action: actionKey,
          label: item.label,
          kind: item.kind,
          title: item.target,
          count: 1,
          state: item.state,
          items: [item],
        }
      } else {
        currentGroup.items.push(item)
        currentGroup.count += 1
        if (item.state === 'active') {
          currentGroup.state = 'active'
        }
      }
    } else if (event.type === 'ToolEnd') {
      const finished = describeTool(event.data.action, event.data.input)
      for (let index = activities.length - 1; index >= 0; index -= 1) {
        if (activities[index].state !== 'active') continue
        if (event.data.callId && activities[index].callId !== event.data.callId) continue
        if (activities[index].action !== finished.action || activities[index].target !== finished.target) continue
        activities[index].state = event.data.status ? (event.data.status === 'success' ? 'done' : 'error') : /^(Error:|Blocked|User denied)/.test(event.data.result) ? 'error' : 'done'
        activities[index].result = event.data.result
        if (event.data.action === 'read_file') {
          const loaded = event.data.result.match(/^\[Loaded skill: ([^\n]+)\]\n/)
          if (loaded) activities[index].label = `Loaded skill: ${loaded[1]}`
          else if (activities[index].label.startsWith('Reading skill:')) {
            activities[index].label = activities[index].state === 'error'
              ? activities[index].label.replace('Reading skill:', 'Failed to read skill:')
              : activities[index].label.replace('Reading skill:', 'Read skill preview:')
          }
        }
        if (activities[index].action === 'ask_user' && event.data.result.startsWith('User answered:')) {
          const answer = event.data.result.replace('User answered:', '').trim()
          activities[index].target = `(Answered: "${answer}") ${activities[index].target}`
        }
        break
      }
      if (currentGroup) {
        const hasActive = currentGroup.items.some((it) => it.state === 'active')
        const hasDone = currentGroup.items.some((it) => it.state === 'done')
        const hasError = currentGroup.items.some((it) => it.state === 'error')
        const hasRetry = currentGroup.items.some((it) => it.state === 'retry')
        currentGroup.state = hasActive ? 'active' : hasDone ? 'done' : hasRetry ? 'retry' : hasError ? 'error' : 'done'
      }
    } else if (event.type === 'ContextCompaction') {
      if (event.data.status !== 'started') {
        flushGroup()
        timeline.push({ id: `context-${nextId++}`, kind: 'thought', thought: event.data.subagent ? `[${event.data.subagent}] ${event.data.message}` : event.data.message })
      }
    } else if (event.type === 'Thought') {
      const text = event.data?.thought?.trim()
      if (text) {
        flushGroup()
        const isCot = isInternalCot(text)
        const kind = isCot ? 'extendedThinking' : 'thought'
        const lastItem = timeline[timeline.length - 1]
        if (lastItem && 'thought' in lastItem && lastItem.thought === text) {
          if (lastItem.kind === 'extendedThinking' && kind === 'thought') {
            timeline[timeline.length - 1] = { id: lastItem.id, kind, thought: text }
          }
        } else if (!lastItem || lastItem.kind !== kind || ('thought' in lastItem && lastItem.thought !== text)) {
          timeline.push({ id: `thought-${nextId++}`, kind, thought: text })
        }
      }
    } else if (event.type === 'ThinkingDelta') {
      const text = event.data?.delta
      if (text) {
        flushGroup()
        timeline.push({
          id: event.data.id,
          kind: 'extendedThinking',
          thought: text,
          streaming: true,
          elapsedMs: event.data.elapsed_ms,
        })
      }
    } else if (event.type === 'ExtendedThinking') {
      const text = event.data?.thought?.trim()
      if (text) {
        flushGroup()
        const lastItem = timeline[timeline.length - 1]
        if (!lastItem || lastItem.kind !== 'extendedThinking' || ('thought' in lastItem && lastItem.thought !== text)) {
          timeline.push({
            id: event.data.id || `ext-thought-${nextId++}`,
            kind: 'extendedThinking',
            thought: text,
            streaming: false,
            elapsedMs: event.data.elapsed_ms,
          })
        }
      }
    }
  }

  flushGroup()

  const runSucceeded = progress.some(
    (e) => e.type === 'RunCompleted' && (e.data?.summary?.outcome === 'SUCCESS' || e.data?.summary?.outcome === 'success')
  )

  for (let i = 0; i < activities.length; i++) {
    if (activities[i].state === 'error') {
      const hasSubsequentSuccessOrActive = activities.slice(i + 1).some(
        (a) => a.state === 'done' || a.state === 'active'
      )
      if (hasSubsequentSuccessOrActive || runSucceeded) {
        activities[i].state = 'retry'
      }
    }
  }

  for (const item of timeline) {
    if (item.kind === 'group') {
      const hasActive = item.group.items.some((it) => it.state === 'active')
      const hasDone = item.group.items.some((it) => it.state === 'done')
      const hasError = item.group.items.some((it) => it.state === 'error')
      const hasRetry = item.group.items.some((it) => it.state === 'retry')
      item.group.state = hasActive ? 'active' : hasDone ? 'done' : hasRetry ? 'retry' : hasError ? 'error' : 'done'
    }
  }

  const items = activities.slice(-25)
  return { summary: activitySummary(activities), items, groups: groupActivities(items), timeline: timeline.slice(-60) }
}

export interface WebSearchSource {
  title: string
  url: string
  snippet: string
  domain: string
  faviconUrl: string
  imageUrl?: string
}

/**
 * Scans AgentProgress events and extracts web search sources from ToolEnd results.
 * Parses the formatted text produced by orchestration.rs for `web_search` actions.
 */
export function parseWebSearchSources(progress: AgentProgress[]): WebSearchSource[] {
  const sources: WebSearchSource[] = []
  for (const event of progress) {
    if (event.type !== 'ToolEnd') continue
    if (event.data.action !== 'web_search') continue

    const result = event.data.result
    if (!result || result.startsWith('Web search error:') || result === 'No web search results found.') continue

    // Each result block looks like:
    //   1. Title text
    //      URL: https://example.com
    //      Snippet text
    const blocks = result.split(/\n\n+/)
    for (const block of blocks) {
      const lines = block.split('\n').map((l: string) => l.trim()).filter(Boolean)
      const titleLine = lines.find((l: string) => /^\d+\.\s/.test(l))
      const urlLine = lines.find((l: string) => l.startsWith('URL:'))
      const imageLine = lines.find((l: string) => l.startsWith('Image:'))
      if (!titleLine || !urlLine) continue

      const title = titleLine.replace(/^\d+\.\s/, '').trim()
      const url = urlLine.replace(/^URL:\s*/, '').trim()
      const imageUrl = imageLine ? imageLine.replace(/^Image:\s*/, '').trim() : undefined
      const snippet = lines
        .filter((l: string) => l !== titleLine && l !== urlLine && l !== imageLine)
        .join(' ')
        .trim()

      try {
        const { hostname } = new URL(url)
        const domain = hostname.replace(/^www\./, '')
        const faviconUrl = `https://www.google.com/s2/favicons?sz=32&domain=${hostname}`
        sources.push({ title, url, snippet, domain, faviconUrl, imageUrl })
      } catch {
        // skip malformed URLs
      }
    }
  }
  return sources
}

/**
 * Strips repeated conversation greetings (e.g. "สวัสดีค่ะพี่ภีม 🌿", "Hello...")
 * from intermediate agent progress notes so only the concrete action note is shown.
 */
export function cleanIntermediateThought(text: string): string {
  if (!text) return ''
  let cleaned = text.trim()
  // Strip leading greetings like "สวัสดีค่ะพี่ภีม 🌿", "สวัสดีครับ 🌿", "Hello...", etc.
  cleaned = cleaned.replace(
    /^(สวัสดีค่ะ|สวัสดีครับ|หวัดดีค่ะ|หวัดดีครับ|สวัสดี|hello|hi|hey)\s*(พี่[^\s,]+|คุณ[^\s,]+|[^\s,]+)?\s*(🌿|🍃|✨|🌱)?\s*[,—–-]?\s*/i,
    ''
  )
  // Strip leading bullet characters
  cleaned = cleaned.replace(/^[\s•\-\*]+\s*/, '')
  return cleaned.trim() || text.trim()
}
