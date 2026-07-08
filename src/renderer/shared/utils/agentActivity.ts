/**
 * shared/utils/agentActivity.ts
 * Agent activity parsing and transformation logic.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import type { AgentProgress } from '../types'

export interface AgentActivity {
  label: string
  target: string
  kind: 'file' | 'folder' | 'search' | 'terminal' | 'tool'
  state: 'active' | 'done' | 'error'
  action?: string
}

export interface AgentActivityView {
  summary: string
  items: AgentActivity[]
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
  if (['search_code', 'semantic_search', 'knowledge_search', 'web_search', 'memory_recall'].includes(action)) return 'search'
  if (['run_shell', 'verify'].includes(action)) return 'terminal'
  if (['list_files', 'detect_project'].includes(action)) return 'folder'
  if (['read_file', 'symbols', 'read_diagnostics', 'git_diff', 'apply_patch', 'write_file', 'note_write', 'view_image'].includes(action)) return 'file'
  return target.includes('/') && !/\.[^/]+$/.test(target) ? 'folder' : 'tool'
}

export function describeTool(action: string, input: Record<string, unknown>): AgentActivity {
  const path = activityDetail(input, 'path')
  const query = activityDetail(input, 'query')
  const command = activityDetail(input, 'command')
  const name = activityDetail(input, 'name')
  const tool = activityDetail(input, 'tool')
  const rawTarget = path || query || command || name || tool || action.replaceAll('_', ' ')

  // Append line range for read_file when startLine / endLine are available
  let target = rawTarget
  if (action === 'read_file' && path) {
    const startLine = typeof input.startLine === 'number' ? input.startLine : undefined
    const endLine = typeof input.endLine === 'number' ? input.endLine : undefined
    if (startLine !== undefined && endLine !== undefined) {
      target = `${rawTarget} #L${startLine}-${endLine}`
    } else if (startLine !== undefined) {
      target = `${rawTarget} #L${startLine}`
    }
  }

  const labels: Record<string, string> = {
    apply_patch: 'Applying patch',
    ask_user: 'Asking user',
    create_plan: 'Creating plan',
    detect_project: 'Detecting project',
    git_branch: 'Reading branch',
    git_diff: 'Reading diff',
    git_log: 'Reading log',
    git_status: 'Reading git status',
    knowledge_search: 'Searching knowledge',
    list_files: 'Listing files',
    list_tests: 'Listing tests',
    mcp_tool: 'Calling MCP tool',
    memory_recall: 'Recalling memory',
    note_write: 'Writing note',
    read_diagnostics: 'Reading diagnostics',
    read_file: 'Reading file',
    request_user_approval: 'Requesting approval',
    run_plugin: 'Running plugin',
    run_shell: 'Running command',
    search_code: 'Searching code',
    semantic_index: 'Indexing code',
    semantic_search: 'Searching code',
    symbols: 'Inspecting symbols',
    update_plan: 'Updating plan',
    verify: 'Verifying',
    view_image: 'Viewing image',
    web_search: 'Searching web',
    write_file: 'Writing file',
  }
  return {
    label: labels[action] ?? 'Using tool',
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

export function activitiesFrom(progress: AgentProgress[]): AgentActivityView {
  const activities: AgentActivity[] = []
  for (const event of progress) {
    if (event.type === 'ToolStart') {
      activities.push(describeTool(event.data.action, event.data.input))
    } else if (event.type === 'ToolEnd') {
      for (let index = activities.length - 1; index >= 0; index -= 1) {
        if (activities[index].state !== 'active') continue
        activities[index].state = event.data.result.startsWith('Error:') ? 'error' : 'done'
        if (activities[index].state === 'error') {
          activities[index].label = 'Failed'
        }
        if (activities[index].action === 'ask_user' && event.data.result.startsWith('User answered:')) {
          const answer = event.data.result.replace('User answered:', '').trim()
          activities[index].target = `(Answered: "${answer}") ${activities[index].target}`
        }
        break
      }
    }
  }
  const items = activities.slice(-12)
  return { summary: activitySummary(activities), items }
}

export interface WebSearchSource {
  title: string
  url: string
  snippet: string
  domain: string
  faviconUrl: string
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
      if (!titleLine || !urlLine) continue

      const title = titleLine.replace(/^\d+\.\s/, '').trim()
      const url = urlLine.replace(/^URL:\s*/, '').trim()
      const snippet = lines
        .filter((l: string) => l !== titleLine && l !== urlLine)
        .join(' ')
        .trim()

      try {
        const { hostname } = new URL(url)
        const domain = hostname.replace(/^www\./, '')
        const faviconUrl = `https://www.google.com/s2/favicons?sz=32&domain=${hostname}`
        sources.push({ title, url, snippet, domain, faviconUrl })
      } catch {
        // skip malformed URLs
      }
    }
  }
  return sources
}

