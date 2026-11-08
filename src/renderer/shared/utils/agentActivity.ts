/**
 * shared/utils/agentActivity.ts
 * Agent activity parsing and transformation logic.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import type { AgentProgress, ActivePlan, RunTelemetrySummary } from '../types'
import { isInternalCot } from '../agentProgress'
import { viewedImageDataUri, viewedImagesFrom, type ViewedImageItem } from './viewedImages'

export interface AskUserItem {
  question: string
  answer?: string
}

export interface AgentActivity {
  label: string
  target: string
  kind: 'file' | 'folder' | 'search' | 'terminal' | 'tool' | 'calc'
  state: 'active' | 'done' | 'error' | 'retry'
  action?: string
  callId?: string
  /** Raw ToolEnd output text, shown when the row is expanded in the UI. */
  result?: string
  viewedImage?: ViewedImageItem
  /** Semantic verb (e.g. 'Analyzed', 'Edited', 'Created', 'Ran', 'Searched') */
  semanticVerb?: string
  /** Human-readable intent or title (e.g. toolSummary for commands, or skill title) */
  intentTitle?: string
  /** Line range suffix if available, e.g. '#L25-50' */
  lineRange?: string
  /** Git diff additions/deletions if available */
  diffStats?: { additions: number; deletions: number }
  /** Elapsed duration in ms or s */
  elapsedMs?: number
  /** Structured ask_user questions and answers */
  askUserData?: {
    header?: string
    questions: AskUserItem[]
  }
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
  semanticVerb?: string
}

export type TimelineItem =
  | { id: string; kind: 'activity'; activity: AgentActivity }
  | { id: string; kind: 'group'; group: AgentActivityGroup }
  | { id: string; kind: 'thought'; thought: string }
  | { id: string; kind: 'extendedThinking'; thought: string; streaming?: boolean; elapsedMs?: number }

export interface AgentActivityView {
  summary: string
  /** Natural composite summary phrase, e.g. "Explored files, ran commands, edited files" */
  naturalSummary?: string
  /** Total elapsed time string if available, e.g. "2m", "14s" */
  durationText?: string
  items: AgentActivity[]
  groups?: AgentActivityGroup[]
  /** Unified chronological list of thoughts, extended thinking, and tool activities. */
  timeline?: TimelineItem[]
}

export function isDummyValue(value: unknown): boolean {
  if (typeof value !== 'string') return false
  const lower = value.trim().toLowerCase()
  return (
    lower === 'null' ||
    lower === 'undefined' ||
    lower === 'none' ||
    lower === 'nil' ||
    lower === '(null)' ||
    lower === '""' ||
    lower === "''"
  )
}

function activityDetail(input: Record<string, unknown>, key: string): string {
  if (!input || typeof input !== 'object') return ''
  const value = input[key]
  if (typeof value === 'string') {
    const trimmed = value.trim()
    return isDummyValue(trimmed) ? '' : trimmed
  }
  return ''
}

export function formatActivityTarget(value: string): string {
  const compact = value.replace(/^\/home\/([^/]+)/, '~')
  return compact || 'workspace'
}

export function activityKind(action: string, target: string): AgentActivity['kind'] {
  if (['search_code', 'semantic_search', 'knowledge_search', 'web_search', 'image_search', 'memory_recall', 'find_definition', 'find_references'].includes(action)) return 'search'
  if (['run_shell', 'verify', 'run_tests', 'run_typecheck', 'run_linter'].includes(action)) return 'terminal'
  if (['list_files', 'detect_project'].includes(action)) return 'folder'
  if (['read_file', 'symbols', 'read_diagnostics', 'git_diff', 'apply_patch', 'write_file', 'edit_file', 'apply_code_edits', 'note_write', 'view_image'].includes(action)) return 'file'
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
  let command =
    activityDetail(input, 'command') ||
    activityDetail(input, 'cmd') ||
    activityDetail(input, 'script') ||
    activityDetail(input, 'exec') ||
    activityDetail(input, 'code') ||
    activityDetail(input, 'bash') ||
    activityDetail(input, 'sh')

  if (!command && input && typeof input === 'object') {
    if (typeof input.arguments === 'object' && input.arguments !== null) {
      const nested = input.arguments as Record<string, unknown>
      command =
        activityDetail(nested, 'command') ||
        activityDetail(nested, 'cmd') ||
        activityDetail(nested, 'script') ||
        activityDetail(nested, 'exec') ||
        activityDetail(nested, 'code') ||
        activityDetail(nested, 'bash') ||
        activityDetail(nested, 'sh')
    } else if (typeof input.arguments === 'string') {
      try {
        const parsed = JSON.parse(input.arguments)
        if (parsed && typeof parsed === 'object') {
          command =
            activityDetail(parsed, 'command') ||
            activityDetail(parsed, 'cmd') ||
            activityDetail(parsed, 'script') ||
            activityDetail(parsed, 'exec') ||
            activityDetail(parsed, 'code') ||
            activityDetail(parsed, 'bash') ||
            activityDetail(parsed, 'sh')
        }
      } catch {
        // best-effort
      }
    }
  }

  if (!command && typeof (input as unknown) === 'string') {
    const raw = (input as unknown as string).trim()
    if (raw && !raw.startsWith('{') && !isDummyValue(raw)) command = raw
  }

  const name = activityDetail(input, 'name')
  const tool = activityDetail(input, 'tool')
  const server = activityDetail(input, 'server')
  const symbol = activityDetail(input, 'symbol') || activityDetail(input, 'ticker')
  const filter = activityDetail(input, 'filter')
  const expression = activityDetail(input, 'expression') || activityDetail(input, 'expr') || activityDetail(input, 'math')
  const city = activityDetail(input, 'city') || activityDetail(input, 'location') || activityDetail(input, 'place')
  const url = activityDetail(input, 'url')
  const selector = activityDetail(input, 'selector')

  let rawTarget = ''
  switch (action) {
    case 'run_shell':
      rawTarget = command || '(empty command)'
      break
    case 'web_search':
    case 'search_code':
    case 'knowledge_search':
      rawTarget = query || '(empty query)'
      break
    case 'read_file':
    case 'write_file':
    case 'view_image':
      rawTarget = path || '(empty path)'
      break
    case 'find_definition':
    case 'find_references':
      rawTarget = symbol || '(empty symbol)'
      break
    case 'run_tests':
      rawTarget = filter || '(all tests)'
      break
    case 'run_typecheck':
    case 'run_linter':
      rawTarget = path || 'workspace'
      break
    case 'calculation':
    case 'calculator':
      rawTarget = expression || '(math expression)'
      break
    case 'weather':
      rawTarget = city || '(location)'
      break
    case 'stock':
      rawTarget = query || symbol || '(symbol)'
      break
    case 'mcp_tool':
      rawTarget = tool ? (server ? `${server}/${tool}` : tool) : (name || 'mcp tool')
      break
    case 'note_write':
      rawTarget = name || path || '(note)'
      break
    case 'exit_plan_mode':
      rawTarget = 'plan approval'
      break
    case 'create_plan':
    case 'update_plan': {
      const summary = activityDetail(input, 'summary') || activityDetail(input, 'title')
      const stepsCount = Array.isArray(input.steps) ? input.steps.length : 0
      const stepsLabel = stepsCount > 0 ? `${stepsCount} step${stepsCount === 1 ? '' : 's'}` : ''
      rawTarget = summary && stepsLabel
        ? `${summary} (${stepsLabel})`
        : summary || stepsLabel || 'task plan'
      break
    }
    default:
      rawTarget = path || query || command || expression || symbol || city || url || selector || filter || name || tool || action.replaceAll('_', ' ')
      break
  }

  // Append line range for read_file when startLine / endLine are available
  let target = rawTarget
  let lineRange: string | undefined
  if (action === 'read_file' && path) {
    const startLine =
      typeof input.startLine === 'number'
        ? input.startLine
        : typeof input.start_line === 'number'
          ? input.start_line
          : typeof input.StartLine === 'number'
            ? input.StartLine
            : undefined
    const endLine =
      typeof input.endLine === 'number'
        ? input.endLine
        : typeof input.end_line === 'number'
          ? input.end_line
          : typeof input.EndLine === 'number'
            ? input.EndLine
            : undefined
    if (startLine !== undefined && endLine !== undefined) {
      lineRange = `#L${startLine}-${endLine}`
      target = `${rawTarget} ${lineRange}`
    } else if (startLine !== undefined) {
      lineRange = `#L${startLine}`
      target = `${rawTarget} ${lineRange}`
    }
  }

  // Diff stats extraction for edit_file / write_file
  let diffStats: { additions: number; deletions: number } | undefined
  if (action === 'edit_file') {
    const targetText = typeof input.Target === 'string' ? input.Target : typeof input.target === 'string' ? input.target : ''
    const replacementText = typeof input.Replacement === 'string' ? input.Replacement : typeof input.replacement === 'string' ? input.replacement : ''
    const deletions = targetText ? targetText.split('\n').length : 0
    const additions = replacementText ? replacementText.split('\n').length : 0
    diffStats = { additions, deletions }
  } else if (action === 'write_file' || action === 'note_write') {
    const rawContent = input.fileContent ?? input.file_content ?? input.Content ?? input.content ?? input.text ?? input.body
    const content = typeof rawContent === 'string' ? rawContent : ''
    const additions = content ? content.split('\n').length : 0
    diffStats = { additions, deletions: 0 }
  }

  // Intent / summary extraction for commands
  let intentTitle: string | undefined
  if (action === 'run_shell' || action === 'run_command' || action === 'bash') {
    intentTitle = activityDetail(input, 'toolSummary') || activityDetail(input, 'summary') || activityDetail(input, 'description')
  }

  let semanticVerb = 'Executed'
  let askUserData: AgentActivity['askUserData']
  if (action === 'read_file') {
    semanticVerb = 'Analyzed'
  } else if (action === 'edit_file' || action === 'apply_patch' || action === 'apply_code_edits') {
    semanticVerb = 'Edited'
  } else if (action === 'write_file' || action === 'note_write') {
    semanticVerb = 'Edited'
  } else if (action === 'run_shell' || action === 'run_command' || action === 'bash') {
    semanticVerb = 'Ran'
  } else if (action?.includes('search')) {
    semanticVerb = 'Searched'
  } else if (action === 'calculation' || action === 'calculator') {
    semanticVerb = 'Calculated'
  } else if (action === 'view_image') {
    semanticVerb = 'Viewed'
  } else if (action === 'create_plan' || action === 'update_plan') {
    semanticVerb = 'Planned'
  } else if (action === 'ask_user') {
    semanticVerb = 'Asked'
    let questions: AskUserItem[] = []
    const rawQuestions = input?.questions
    if (Array.isArray(rawQuestions) && rawQuestions.length > 0) {
      questions = (rawQuestions as Array<Record<string, unknown> | string>)
        .map((q) => {
          if (typeof q === 'string') return { question: q.trim() }
          const questionText = String(q.question || q.query || q.title || '').trim()
          const answerText = typeof q.answer === 'string' ? q.answer.trim() : undefined
          return { question: questionText, answer: answerText }
        })
        .filter((q) => q.question.length > 0)
    } else if (query) {
      questions = [{ question: query }]
    }
    if (questions.length > 0) {
      const headerText = typeof input?.header === 'string' ? input.header.trim() : undefined
      askUserData = {
        header: headerText || undefined,
        questions,
      }
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
    semanticVerb,
    intentTitle,
    lineRange,
    diffStats,
    askUserData,
  }
}

export function formatDurationText(durationSecs?: number): string {
  if (durationSecs === undefined || durationSecs === null || durationSecs <= 0) return ''
  const secs = Math.round(durationSecs)
  if (secs < 60) return `${secs}s`
  const mins = Math.floor(secs / 60)
  const remSecs = secs % 60
  if (remSecs === 0) return `${mins}m`
  return `${mins}m ${remSecs}s`
}

export function isMarkdownContent(action?: string, target?: string, output?: string): boolean {
  if (!output || !output.trim()) return false
  if (action === 'run_shell' || action === 'run_command' || action === 'bash') return false
  const cleanTarget = (target || '').toLowerCase()
  if (cleanTarget.endsWith('.md') || cleanTarget.endsWith('.markdown') || cleanTarget.endsWith('.txt')) {
    return true
  }
  if (cleanTarget.includes('skill') || output.startsWith('[Loaded skill:')) {
    return true
  }
  if (action === 'note_read' || action === 'web_search') {
    return true
  }
  if (/^#{1,6}\s+/m.test(output) && (output.includes('\n\n') || output.includes('\n- '))) {
    return true
  }
  return false
}

export function cleanMarkdownOutput(text: string): string {
  let cleaned = text.trim()
  cleaned = cleaned.replace(/^\[Loaded skill:\s*([^\]]+)\][\s\n]*/i, '**Skill: $1**\n\n')
  const frontmatterMatch = cleaned.match(/^(\*\*Skill:[^\n]+\*\*[\s\n]*)?---\s*\n([\s\S]*?)\n---\s*\n/)
  if (frontmatterMatch) {
    const prefix = frontmatterMatch[1] ? frontmatterMatch[1].trim() + '\n\n' : ''
    const yaml = frontmatterMatch[2]
    const descMatch = yaml.match(/description:\s*([^\n]+(?:\n\s+[^\n]+)*)/)
    const desc = descMatch ? descMatch[1].replace(/\n\s+/g, ' ').trim() : ''
    const remaining = cleaned.slice(frontmatterMatch[0].length)
    if (desc) {
      cleaned = `${prefix}> ${desc}\n\n${remaining}`
    } else {
      cleaned = `${prefix}${remaining}`
    }
  }
  return cleaned
}

export function generateNaturalActivitySummary(items: AgentActivity[], timeline?: TimelineItem[]): string {
  if (!items || items.length === 0) {
    if (timeline && timeline.some(t => t.kind === 'extendedThinking' || t.kind === 'thought')) {
      return 'Thinking'
    }
    return 'Working'
  }

  let hasRead = false
  let hasWrite = false
  let hasCommand = false
  let hasSearch = false
  let hasThought = Boolean(timeline && timeline.some(t => t.kind === 'extendedThinking'))

  for (const item of items) {
    if (item.action === 'read_file') hasRead = true
    else if (item.action === 'write_file' || item.action === 'edit_file' || item.action === 'apply_patch') hasWrite = true
    else if (item.action === 'run_shell' || item.action === 'run_command' || item.action === 'bash') hasCommand = true
    else if (item.action?.includes('search')) hasSearch = true
  }

  const parts: string[] = []
  if (hasRead) parts.push('Explored files')
  if (hasCommand) parts.push('ran commands')
  if (hasWrite) parts.push('edited files')
  if (hasSearch && !hasRead) parts.push('searched codebase')
  if (parts.length === 0) {
    if (hasThought) return 'Analyzed task'
    return 'Working through task'
  }

  return parts.join(', ')
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
    group.semanticVerb = group.items[0].semanticVerb
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
  if (action === 'calculation' || action === 'calculator') {
    group.semanticVerb = 'Calculated'
    group.title = `${count} expressions`
  } else if (action === 'read_file') {
    group.semanticVerb = 'Analyzed'
    group.title = group.state === 'active' ? `reading files (${doneCount}/${count})` : `${count} files`
  } else if (action === 'write_file' || action === 'edit_file' || action === 'apply_patch') {
    group.semanticVerb = 'Edited'
    group.title = group.state === 'active' ? `writing files (${doneCount}/${count})` : `${count} files`
  } else if (action === 'run_shell' || action === 'run_command' || action === 'bash') {
    group.semanticVerb = 'Ran'
    group.title = `${count} commands`
  } else if (action === 'search_code' || action === 'semantic_search' || action === 'web_search') {
    group.semanticVerb = 'Searched'
    group.title = `${count} queries`
  } else {
    group.semanticVerb = 'Executed'
    const readableAction = action.replaceAll('_', ' ')
    group.title = `${count} ${readableAction}`
  }

  return group
}

export function activitiesFrom(progress: AgentProgress[]): AgentActivityView {
  const cached = activityCache.get(progress)
  if (cached?.length === progress.length) return cached.view
  const view = deriveActivitiesFrom(progress)
  activityCache.set(progress, { length: progress.length, view })
  return view
}

const activityCache = new WeakMap<AgentProgress[], { length: number; view: AgentActivityView }>()

function deriveActivitiesFrom(progress: AgentProgress[]): AgentActivityView {
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
      if (event.data.subagent) continue
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
      if (event.data.subagent) continue
      const finished = describeTool(event.data.action, event.data.input)
      for (let index = activities.length - 1; index >= 0; index -= 1) {
        if (activities[index].state !== 'active') continue
        if (event.data.callId && activities[index].callId !== event.data.callId) continue
        if (activities[index].action !== finished.action) continue
        if (event.data.input && activities[index].target !== finished.target) continue
        activities[index].state = event.data.status ? (event.data.status === 'success' ? 'done' : 'error') : /^(Error:|Blocked|User denied)/.test(event.data.result) ? 'error' : 'done'
        const elapsed = event.data.elapsed_ms ?? event.data.elapsedMs
        if (typeof elapsed === 'number') {
          activities[index].elapsedMs = elapsed
        }
        const lineMatch = typeof event.data.result === 'string' && event.data.result.match(/\+(\d+)\/-(\d+) lines/)
        if (lineMatch) {
          const add = parseInt(lineMatch[1], 10)
          const del = parseInt(lineMatch[2], 10)
          if (!isNaN(add) && !isNaN(del)) {
            activities[index].diffStats = { additions: add, deletions: del }
          }
        }
        const viewedImage = event.data.action === 'view_image' ? viewedImageDataUri(event.data.result) : null
        activities[index].result = viewedImage ? event.data.result.replace(viewedImage, '[Image displayed below]') : event.data.result
        if (event.data.action === 'read_file') {
          const loaded = event.data.result.match(/^\[Loaded skill: ([^\n]+)\]\n/)
          if (loaded) activities[index].label = `Loaded skill: ${loaded[1]}`
          else if (activities[index].label.startsWith('Reading skill:')) {
            activities[index].label = activities[index].state === 'error'
              ? activities[index].label.replace('Reading skill:', 'Failed to read skill:')
              : activities[index].label.replace('Reading skill:', 'Read skill preview:')
          }
        }
        if (activities[index].action === 'ask_user') {
          let answer: string | undefined
          if (typeof event.data.result === 'string') {
            if (event.data.result.startsWith('User answered:')) {
              answer = event.data.result.replace(/^User answered:\s*/, '').trim()
              activities[index].target = `(Answered: "${answer}") ${activities[index].target}`
            } else if (event.data.result.startsWith('User approved')) {
              answer = 'Approved'
            } else if (event.data.result.startsWith('User declined')) {
              answer = 'Declined'
            }
          }

          if (answer) {
            if (!activities[index].askUserData) {
              const cleanQuestion = activities[index].target.replace(/^\(Answered:\s*".*?"\)\s*/s, '').trim()
              activities[index].askUserData = {
                questions: [{ question: cleanQuestion, answer }],
              }
            } else if (activities[index].askUserData.questions.length > 0) {
              let assigned = false
              if (answer.startsWith('[') && answer.endsWith(']')) {
                try {
                  const parsed = JSON.parse(answer)
                  if (Array.isArray(parsed) && parsed.length === activities[index].askUserData.questions.length) {
                    parsed.forEach((ans, i) => {
                      activities[index].askUserData!.questions[i].answer =
                        typeof ans === 'string' ? ans : JSON.stringify(ans)
                    })
                    assigned = true
                  }
                } catch {
                  // ignore
                }
              }
              if (!assigned) {
                activities[index].askUserData.questions[0].answer = answer
              }
            }
          }
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
      if (event.data.subagent) continue
      if (event.data.status !== 'started') {
        flushGroup()
        timeline.push({ id: `context-${nextId++}`, kind: 'thought', thought: event.data.message })
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

  const viewedImages = viewedImagesFrom(progress)
  if (viewedImages.length > 0) {
    const byCallId = new Map<string, ViewedImageItem>()
    const byPath = new Map<string, ViewedImageItem[]>()
    for (const img of viewedImages) {
      if (img.id) byCallId.set(img.id, img)
      if (img.path) {
        const list = byPath.get(img.path) || []
        list.push(img)
        byPath.set(img.path, list)
      }
    }
    for (const act of activities) {
      if (act.action === 'view_image' && act.state === 'done') {
        if (act.callId && byCallId.has(act.callId)) {
          act.viewedImage = byCallId.get(act.callId)
        } else if (byPath.has(act.target)) {
          const list = byPath.get(act.target)!
          if (list.length > 0) {
            act.viewedImage = list.shift()
          }
        }
      }
    }
  }

  let totalDurationSecs: number | undefined
  for (const event of progress) {
    if (event.type === 'RunCompleted' && typeof event.data?.summary?.durationSecs === 'number') {
      totalDurationSecs = event.data.summary.durationSecs
    }
  }

  const naturalSummary = generateNaturalActivitySummary(activities, timeline)
  const durationText = formatDurationText(totalDurationSecs)

  const items = activities.slice(-50)
  return {
    summary: activitySummary(activities),
    naturalSummary,
    durationText,
    items,
    groups: groupActivities(items),
    timeline: timeline.slice(-80),
  }
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

export function extractLatestPlan(progress: AgentProgress[]): ActivePlan | undefined {
  for (let i = progress.length - 1; i >= 0; i--) {
    const event = progress[i]
    if (event.type === 'PlanUpdated') {
      return event.data.plan
    }
  }
  return undefined
}

export function extractLatestRunSummary(progress: AgentProgress[]): RunTelemetrySummary | undefined {
  for (let i = progress.length - 1; i >= 0; i--) {
    const event = progress[i]
    if (event.type === 'RunCompleted') {
      return event.data.summary
    }
  }
  return undefined
}

export function hasSubagentActivity(progress: AgentProgress[]): boolean {
  return progress.some(
    (e) =>
      e.type === 'SubagentStarted' ||
      e.type === 'SubagentFinished' ||
      ((e.type === 'ToolStart' || e.type === 'ToolEnd') &&
        (e.data.subagent || e.data.action === 'dispatch_agents' || e.data.action === 'dispatch_subagent')),
  )
}

export function isAskUserActivity(activity: AgentActivity): boolean {
  return (
    activity.action === 'ask_user' ||
    activity.label === 'ask_user' ||
    Boolean(activity.askUserData) ||
    activity.target.startsWith('(Answered:')
  )
}

export function extractAskUserQuestions(activity: AgentActivity): AskUserItem[] {
  if (activity.askUserData?.questions && activity.askUserData.questions.length > 0) {
    return activity.askUserData.questions
  }
  const match = activity.target.match(/^\(Answered:\s*"(.*?)"\)\s*(.*)$/s)
  if (match) {
    return [{ question: match[2].trim(), answer: match[1].trim() }]
  }
  let answer: string | undefined
  if (typeof activity.result === 'string') {
    if (activity.result.startsWith('User answered:')) {
      answer = activity.result.replace(/^User answered:\s*/, '').trim()
    } else if (activity.result.startsWith('User approved')) {
      answer = 'Approved'
    } else if (activity.result.startsWith('User declined')) {
      answer = 'Declined'
    }
  }
  const cleanQuestion = activity.target.replace(/^\(Answered:\s*".*?"\)\s*/s, '').trim()
  if (cleanQuestion) {
    return [{ question: cleanQuestion, answer }]
  }
  return []
}
