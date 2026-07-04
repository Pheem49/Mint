import { useEffect, useRef, useState, Fragment, type ChangeEvent, type ClipboardEvent, type DragEvent, type FormEvent, type KeyboardEvent, type RefObject } from 'react'
import { hasAgentToolActivity, thoughtsFrom } from '../agentProgress'
import {
  type AgentProgress,
  type ChatResponse,
  type RuntimeStatus,
  getTtsUrls,
} from '../tauri'

const GEMINI_MODELS = [
  'gemini-2.5-flash',
  'gemini-2.5-pro',
  'gemini-2.0-flash',
  'gemini-1.5-flash',
  'gemini-1.5-pro',
  'gemini-3.1-flash-lite',
  'gemini-3.1-flash-lite-preview'
]

const OPENAI_MODELS = [
  'gpt-4o',
  'gpt-4o-mini',
  'o1',
  'o3-mini',
  'o1-preview',
  'o1-mini',
  'gpt-4-turbo'
]

const OPENROUTER_MODELS = [
  'openai/gpt-4o-mini',
  'openai/gpt-4o',
  'anthropic/claude-3.5-sonnet',
  'anthropic/claude-3.5-haiku',
  'google/gemini-2.5-flash',
  'meta-llama/llama-3.3-70b-instruct',
  'mistralai/mistral-large'
]

const DEEPSEEK_MODELS = [
  'deepseek-v4-flash',
  'deepseek-v4-pro',
  'deepseek-chat',
  'deepseek-reasoner'
]

const ANTHROPIC_MODELS = [
  'claude-3-7-sonnet-latest',
  'claude-3-5-sonnet-latest',
  'claude-3-5-haiku-latest',
  'claude-3-opus-latest'
]

const HF_MODELS = [
  'meta-llama/Llama-3.3-70B-Instruct',
  'meta-llama/Meta-Llama-3-8B-Instruct',
  'meta-llama/Llama-3.2-3B-Instruct',
  'Qwen/Qwen2.5-72B-Instruct',
  'Qwen/Qwen2.5-Coder-32B-Instruct',
  'mistralai/Mistral-7B-Instruct-v0.3',
  'google/gemma-2-9b-it'
]

const LOCAL_MODELS = [
  'local-model',
  'Qwen/Qwen2.5-7B-Instruct-GGUF',
  'meta-llama/Llama-3.2-3B-Instruct-GGUF',
  'lmstudio-community/gemma-2-9b-it-GGUF'
]

const OLLAMA_MODELS: string[] = []

interface ApprovalDetails {
  title: string
  body: string
  reason?: string
  isDangerous: boolean
}

function badge(provider: string, model: string) {
  return [provider, model].filter(Boolean).join(' / ')
}

function providerLabel(provider: string) {
  switch (provider) {
    case 'gemini':
      return 'Gemini'
    case 'openai':
      return 'OpenAI'
    case 'openrouter':
      return 'OpenRouter'
    case 'deepseek':
      return 'DeepSeek'
    case 'anthropic':
      return 'Claude'
    case 'huggingface':
      return 'Hugging Face'
    case 'local_openai':
      return 'Local OpenAI'
    case 'ollama':
      return 'Ollama'
    default:
      return provider || 'Primary provider'
  }
}

function fallbackNotice(response: Pick<ChatResponse, 'provider' | 'fallbackProvider'> | null | undefined) {
  if (!response?.fallbackProvider) return ''
  return `${providerLabel(response.fallbackProvider)} unavailable, fell back to ${providerLabel(response.provider)}.`
}

interface AgentActivity {
  label: string
  target: string
  kind: 'file' | 'folder' | 'search' | 'terminal' | 'tool'
  state: 'active' | 'done' | 'error'
}

interface AgentActivityView {
  summary: string
  items: AgentActivity[]
}

function activityDetail(input: Record<string, unknown>, key: string) {
  const value = input[key]
  return typeof value === 'string' && value.trim() ? value : ''
}

function formatActivityTarget(value: string) {
  const compact = value.replace(/^\/home\/([^/]+)/, '~')
  return compact || 'workspace'
}

function activityKind(action: string, target: string): AgentActivity['kind'] {
  if (['search_code', 'semantic_search', 'knowledge_search', 'web_search', 'memory_recall'].includes(action)) return 'search'
  if (['run_shell', 'verify'].includes(action)) return 'terminal'
  if (['list_files', 'detect_project'].includes(action)) return 'folder'
  if (['read_file', 'symbols', 'read_diagnostics', 'git_diff', 'apply_patch', 'write_file', 'note_write', 'view_image'].includes(action)) return 'file'
  return target.includes('/') && !/\.[^/]+$/.test(target) ? 'folder' : 'tool'
}

function describeTool(action: string, input: Record<string, unknown>): AgentActivity {
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
  }
}

function activitySummary(items: AgentActivity[]) {
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

function activitiesFrom(progress: AgentProgress[]): AgentActivityView {
  const activities: AgentActivity[] = []
  for (const event of progress) {
    if (event.type === 'ToolStart') {
      activities.push(describeTool(event.data.action, event.data.input))
    } else if (event.type === 'ToolEnd') {
      for (let index = activities.length - 1; index >= 0; index -= 1) {
        if (activities[index].state !== 'active') continue
        activities[index].state = event.data.result.startsWith('Error:') ? 'error' : 'done'
        if (activities[index].state === 'error') activities[index].label = 'Failed'
        break
      }
    }
  }
  const items = activities.slice(-12)
  return { summary: activitySummary(activities), items }
}

function renderAgentActivityTable(activityView: AgentActivityView) {
  return (
    <div className="agent-activity-list">
      <div className="agent-activity-table-head" aria-hidden="true">
        <span>Tool</span>
        <span />
        <span>Target</span>
        <span />
      </div>
      {activityView.items.map((activity, index) => (
        <div className="agent-activity-item" data-kind={activity.kind} data-state={activity.state} key={`${index}-${activity.label}-${activity.target}`}>
          <span className="agent-activity-label">{activity.label}</span>
          <span className="agent-activity-icon" aria-hidden="true" />
          <span className="agent-activity-text">{activity.target}</span>
          <span className="agent-activity-chevron" aria-hidden="true">&gt;</span>
        </div>
      ))}
    </div>
  )
}

function renderApprovalDetails(approval: any): ApprovalDetails {
  if (!approval) return { title: 'Action Pending Approval', body: 'No action details available.', isDangerous: false }
  if (approval.WriteFile) return { title: 'Write File', body: `Path: ${approval.WriteFile.path}`, reason: approval.WriteFile.diff ? `Diff:\n${approval.WriteFile.diff}` : 'Writing new file content.', isDangerous: false }
  if (approval.ApplyPatch) return { title: 'Apply Patch', body: `Path: ${approval.ApplyPatch.path}`, reason: approval.ApplyPatch.diff ? `Diff:\n${approval.ApplyPatch.diff}` : 'Applying code patch.', isDangerous: false }
  if (approval.RunShell) return { title: 'Run Shell Command', body: approval.RunShell.command, reason: 'Executing shell commands can modify your system.', isDangerous: true }
  if (approval.NoteWrite) return { title: 'Write Note', body: `Path: ${approval.NoteWrite.path}`, reason: 'Creating or updating workspace notes.', isDangerous: false }
  if (approval.RunPlugin) return { title: `Run Plugin: ${approval.RunPlugin.name}`, body: approval.RunPlugin.instruction, reason: 'Executing a native plugin action.', isDangerous: false }
  if (approval.McpTool) {
    const { server, tool, arguments: args } = approval.McpTool
    return { title: `Run MCP Tool: ${server}/${tool}`, body: typeof args === 'string' ? args : JSON.stringify(args, null, 2), reason: 'Running external MCP tool.', isDangerous: false }
  }
  if (approval.UserApproval) return { title: approval.UserApproval.title, body: approval.UserApproval.prompt, reason: 'The agent requested explicit approval.', isDangerous: false }
  if (approval.AskUser) return { title: 'Question From Agent', body: approval.AskUser.question, reason: 'Approve to continue without a typed answer, or cancel to decline.', isDangerous: false }
  return { title: 'Unknown Action', body: JSON.stringify(approval, null, 2), reason: 'Requires approval to proceed.', isDangerous: false }
}

function renderDiff(diffText: string) {
  if (!diffText) return null
  const lines = diffText.split('\n')
  return (
    <div style={{ 
      background: '#0b0f19', 
      borderRadius: '6px', 
      padding: '8px', 
      border: '1px solid rgba(255, 255, 255, 0.08)', 
      overflowX: 'auto', 
      maxHeight: '400px',
      fontFamily: 'monospace',
      fontSize: '0.74rem',
      lineHeight: '1.4',
    }}>
      {lines.map((line, idx) => {
        let style: any = {
          whiteSpace: 'pre-wrap',
          padding: '2px 6px',
        }
        
        if (line.startsWith('@@') || line.startsWith('---') || line.startsWith('+++')) {
          style = {
            ...style,
            color: '#64748b',
            fontWeight: 'bold',
          }
        } else if (line.startsWith('+')) {
          style = {
            ...style,
            background: 'rgba(16, 185, 129, 0.12)',
            borderLeft: '3px solid #10b981',
            color: '#a7f3d0'
          }
        } else if (line.startsWith('-')) {
          style = {
            ...style,
            background: 'rgba(239, 68, 68, 0.12)',
            borderLeft: '3px solid #ef4444',
            color: '#fca5a5'
          }
        } else {
          style = {
            ...style,
            color: '#e2e8f0',
          }
        }
        
        return (
          <div key={idx} style={style}>
            {line}
          </div>
        )
      })}
    </div>
  )
}

interface ChatPanelProps {
  interactions: any[]
  sending: boolean
  sendingMessage: string
  sendingImageCount: number
  streamedReply: string
  streamedResponse: ChatResponse | null
  agentProgress: AgentProgress[]
  agentActivitySnapshots: Record<string, AgentProgress[]>
  thinkingExpanded: Record<string, boolean>
  onThinkingExpandedChange: (key: string, open: boolean) => void
  message: string
  imageAttachments: Array<{ dataUri: string; name: string; previewDataUri?: string }>
  documentName: string
  pendingApproval: any | null
  smartContext: boolean
  agentMode: boolean
  status: RuntimeStatus | null
  workspacePath: string
  chatEnd: RefObject<HTMLDivElement | null>
  welcomeInteraction: any
  onSubmit: (event: FormEvent<HTMLFormElement>) => void
  onSelectImage: (event: ChangeEvent<HTMLInputElement>) => void
  onSelectDocument: (event: ChangeEvent<HTMLInputElement>) => void
  onPasteImage: (clipboardData: DataTransfer) => boolean
  onReadClipboardImage: () => Promise<boolean>
  onSetMessage: (message: string) => void
  onSendVoiceMessage: (message: string, audioDataUri?: string | null) => Promise<void>
  onRemoveImage: (idx: number) => void
  onRemoveDocument: () => void
  onStartWebSearch: () => void
  onCaptureScreen: () => void
  onSetSmartContext: (enabled: boolean) => void
  onSetAgentMode: (enabled: boolean) => void
  onSetProvider: (provider: string) => void
  onSelectWorkspace: () => void
  onApproval: (approved: boolean, autoApproveSession?: boolean) => void
  settingsConfig: any
  onSetModel: (model: string) => void
}

function ChatCodeBlock({ code, language }: { code: string; language: string; key?: any }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy code: ', err)
    }
  }

  const handleDownload = () => {
    try {
      const blob = new Blob([code], { type: 'text/plain;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `code.${language === 'plaintext' ? 'txt' : language}`
      link.click()
      URL.revokeObjectURL(url)
    } catch (err) {
      console.error('Failed to download code: ', err)
    }
  }

  const displayLang = language.charAt(0).toUpperCase() + language.slice(1).toLowerCase()

  return (
    <div className="chat-code-block-container" style={{ whiteSpace: 'normal' }}>
      <div className="chat-code-block-header">
        <span className="chat-code-block-lang">{displayLang}</span>
        <div className="chat-code-block-actions">
          <button type="button" onClick={handleDownload} title="Download code" className="chat-code-action-btn">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
          </button>
          <button type="button" onClick={handleCopy} title="Copy code" className="chat-code-action-btn">
            {copied ? (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            ) : (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
            )}
          </button>
        </div>
      </div>
      <pre className="chat-code-block-body">
        <code>{code}</code>
      </pre>
    </div>
  )
}

const isTableLine = (line: string): boolean => {
  const trimmed = line.trim()
  return trimmed.startsWith('|') && trimmed.endsWith('|') && trimmed.length > 1
}

function renderFormattedMessage(text: string) {
  const displayText = readableAssistantText(text)
  if (!displayText) return null

  const formatInline = (str: string) => {
    const codeParts = str.split(/`([\s\S]*?)`/g)
    return codeParts.map((codePart, codeIndex) => {
      if (codeIndex % 2 === 1) {
        return (
          <code key={`code-${codeIndex}`} className="chat-inline-code">
            {codePart}
          </code>
        )
      }
      const boldParts = codePart.split(/\*\*([\s\S]*?)\*\*/g)
      return boldParts.map((boldPart, boldIndex) => {
        if (boldIndex % 2 === 1) {
          return (
            <strong key={`bold-${boldIndex}`} className="chat-bold-highlight">
              {boldPart}
            </strong>
          )
        }
        const mentionParts = boldPart.split(/(@[\w\-\.\/]+)/g)
        return mentionParts.map((mentionPart, mentionIndex) => {
          if (mentionIndex % 2 === 1) {
            return (
              <span key={`mention-${mentionIndex}`} className="chat-mention">
                {mentionPart}
              </span>
            )
          }
          return mentionPart
        })
      })
    })
  }

  const renderHtmlTable = (tableLines: string[], key: string) => {
    const rows: string[][] = []
    for (const line of tableLines) {
      const trimmed = line.trim()
      if (!trimmed) continue
      const content = trimmed.slice(1, -1)
      const contentEscaped = content.replace(/\\\|/g, '\u0000')
      const cols = contentEscaped.split('|').map(s => s.replace(/\u0000/g, '|').trim())
      rows.push(cols)
    }

    if (rows.length === 0) return null

    let hasSeparator = false
    let separatorIdx = -1
    for (let i = 0; i < rows.length; i++) {
      const row = rows[i]
      if (row.length > 0 && row.every(col => /^[-\s:]+$/.test(col))) {
        hasSeparator = true
        separatorIdx = i
        break
      }
    }

    let headerRow: string[] | null = null
    const dataRows: string[][] = []

    if (hasSeparator) {
      if (separatorIdx > 0) {
        headerRow = rows[separatorIdx - 1]
        for (let i = 0; i < rows.length; i++) {
          if (i !== separatorIdx && i !== separatorIdx - 1) {
            dataRows.push(rows[i])
          }
        }
      } else {
        for (let i = 0; i < rows.length; i++) {
          if (i !== separatorIdx) {
            dataRows.push(rows[i])
          }
        }
      }
    } else {
      headerRow = rows[0]
      for (let i = 1; i < rows.length; i++) {
        dataRows.push(rows[i])
      }
    }

    return (
      <div key={key} className="chat-table-container" style={{
        overflowX: 'auto',
        margin: '14px 0',
        width: '100%',
        borderRadius: '8px',
        border: '1px solid rgba(255, 255, 255, 0.12)',
        background: 'rgba(30, 41, 59, 0.35)',
        boxShadow: '0 4px 12px rgba(0, 0, 0, 0.15)',
      }}>
        <table className="chat-table" style={{
          width: '100%',
          borderCollapse: 'collapse',
          fontSize: '0.86rem',
          textAlign: 'left',
          lineHeight: '1.5',
        }}>
          {headerRow && (
            <thead>
              <tr style={{
                background: 'rgba(255, 255, 255, 0.04)',
                borderBottom: '2px solid rgba(255, 255, 255, 0.15)',
              }}>
                {headerRow.map((col, idx) => (
                  <th key={`th-${idx}`} style={{
                    padding: '12px 16px',
                    fontWeight: 700,
                    color: 'var(--accent, #38bdf8)',
                    fontFamily: 'ui-sans-serif, system-ui, sans-serif',
                  }}>
                    {formatInline(col)}
                  </th>
                ))}
              </tr>
            </thead>
          )}
          <tbody>
            {dataRows.map((row, rIdx) => (
              <tr key={`tr-${rIdx}`} style={{
                background: rIdx % 2 === 1 ? 'rgba(255, 255, 255, 0.015)' : 'transparent',
                borderBottom: rIdx < dataRows.length - 1 ? '1px solid rgba(255, 255, 255, 0.08)' : 'none',
              }}>
                {row.map((col, cIdx) => (
                  <td key={`td-${cIdx}`} style={{
                    padding: '12px 16px',
                    color: 'var(--text-main, #e2e8f0)',
                  }}>
                    {formatInline(col)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    )
  }

  const lines = displayText.split('\n')
  const items: any[] = []

  let inCodeBlock = false
  let codeBlockLang = ''
  let codeBlockLines: string[] = []

  let inTable = false
  let tableLines: string[] = []

  const flushTable = (index: number) => {
    if (tableLines.length > 0) {
      items.push(renderHtmlTable(tableLines, `table-${index}`))
      tableLines = []
      inTable = false
    }
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]

    if (line.trim().startsWith('```')) {
      if (inTable) {
        flushTable(i)
      }
      if (inCodeBlock) {
        const codeText = codeBlockLines.join('\n')
        items.push(
          <ChatCodeBlock
            key={`code-block-${i}`}
            code={codeText}
            language={codeBlockLang}
          />
        )
        inCodeBlock = false
        codeBlockLines = []
      } else {
        inCodeBlock = true
        codeBlockLang = line.trim().slice(3).trim() || 'plaintext'
      }
      continue
    }

    if (inCodeBlock) {
      codeBlockLines.push(line)
      continue
    }

    if (isTableLine(line)) {
      inTable = true
      tableLines.push(line)
    } else {
      if (inTable) {
        flushTable(i)
      }

      const headerMatch = line.match(/^(#{1,6})\s+(.*)$/)
      if (headerMatch) {
        const level = headerMatch[1].length
        const content = headerMatch[2]

        const style = {
          fontWeight: 'bold',
          display: 'block',
          marginTop: level === 1 ? '16px' : level === 2 ? '14px' : '10px',
          marginBottom: '6px',
          fontSize: level === 1 ? '1.25em' : level === 2 ? '1.15em' : '1.05em',
          color: 'var(--text-main)',
        }

        items.push(
          <span key={`line-${i}`} style={style}>
            {formatInline(content)}
          </span>
        )
      } else {
        const listMatch = line.match(/^(\s*)([-*+])\s+(.*)$/)
        if (listMatch) {
          const indent = listMatch[1]
          const content = listMatch[3]
          items.push(
            <Fragment key={`line-${i}`}>
              {indent}• {formatInline(content)}
              {i < lines.length - 1 && '\n'}
            </Fragment>
          )
        } else {
          items.push(
            <Fragment key={`line-${i}`}>
              {formatInline(line)}
              {i < lines.length - 1 && '\n'}
            </Fragment>
          )
        }
      }
    }
  }

  if (inTable) {
    flushTable(lines.length)
  }

  if (inCodeBlock && codeBlockLines.length > 0) {
    const codeText = codeBlockLines.join('\n')
    items.push(
      <ChatCodeBlock
        key={`code-block-end`}
        code={codeText}
        language={codeBlockLang}
      />
    )
  }

  return items
}

const THINKING_LABELS = {
  live: 'Thinking…',
  completed: (count: number) => `Thinking process (${count} steps)`,
  step: (index: number) => `Step ${index}`,
  emptyHint: 'Model didn\'t send thinking steps this time',
} as const

function ThinkingBlock({
  thoughts,
  isLive = false,
  blockKey,
  expanded,
  onExpandedChange,
  showEmptyHint = false,
}: {
  thoughts: string[]
  isLive?: boolean
  blockKey: string
  expanded?: boolean
  onExpandedChange?: (key: string, open: boolean) => void
  showEmptyHint?: boolean
}) {
  const contentRef = useRef<HTMLDivElement>(null)
  const isControlled = expanded !== undefined && Boolean(onExpandedChange)
  const [localOpen, setLocalOpen] = useState(isLive)
  const isOpen = isControlled ? Boolean(expanded) : localOpen

  const setOpen = (open: boolean) => {
    if (isControlled && onExpandedChange) onExpandedChange(blockKey, open)
    else setLocalOpen(open)
  }

  useEffect(() => {
    if (isLive && !isControlled) setLocalOpen(true)
  }, [isLive, isControlled])

  useEffect(() => {
    if (!isLive || !isOpen || !contentRef.current) return
    contentRef.current.scrollTop = contentRef.current.scrollHeight
  }, [thoughts, isLive, isOpen])

  if (thoughts.length === 0 && !showEmptyHint) return null

  if (thoughts.length === 0) {
    return (
      <div className="thinking-block thinking-block-empty-state" data-live={isLive ? 'true' : 'false'}>
        <span className="thinking-block-empty">{THINKING_LABELS.emptyHint}</span>
      </div>
    )
  }

  return (
    <div className={`thinking-block${isLive ? ' is-live' : ''}`}>
      <button
        type="button"
        className="thinking-block-header"
        onClick={() => setOpen(!isOpen)}
        aria-expanded={isOpen}
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
          <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
          <line x1="12" y1="17" x2="12.01" y2="17" />
          <circle cx="12" cy="12" r="10" />
        </svg>
        <span className="thinking-block-label">
          {isLive ? THINKING_LABELS.live : THINKING_LABELS.completed(thoughts.length)}
        </span>
        {isLive && <span className="thinking-block-live-dot" aria-hidden="true" />}
        <span className={`thinking-block-chevron${isOpen ? ' is-open' : ''}`} aria-hidden="true">
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="9 18 15 12 9 6" />
          </svg>
        </span>
      </button>
      {isOpen && (
        <div className="thinking-block-body">
          <div className="thinking-block-content" ref={contentRef}>
            {thoughts.map((thought, index) => (
              <div className="thinking-block-step" key={`${blockKey}-${index}`}>
                {thoughts.length > 1 && (
                  <div className="thinking-block-step-label">{THINKING_LABELS.step(index + 1)}</div>
                )}
                <div className="thinking-block-step-body">{renderFormattedMessage(thought)}</div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

function readableAssistantText(text: string) {
  if (typeof text !== 'string') return ''
  const trimmed = text.trim()
  if (!trimmed.startsWith('{')) return text
  try {
    const value = JSON.parse(trimmed)
    if (value?.action === 'finish' && typeof value?.input?.summary === 'string' && value.input.summary.trim()) {
      return value.input.summary
    }
    if (typeof value?.finish?.summary === 'string' && value.finish.summary.trim()) {
      return value.finish.summary
    }
  } catch {
    return text
  }
  return text
}

function renderSpeakerIcon(isSpeaking: boolean) {
  if (isSpeaking) {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
        <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
        <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
      </svg>
    )
  }
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
      <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
    </svg>
  )
}

function cleanSpeechText(text: string) {
  return readableAssistantText(text)
    .replace(/\*\*([\s\S]*?)\*\*/g, '$1')
    .replace(/[*_`#]/g, '')
    .trim()
}

function numericSetting(value: unknown, fallback: number) {
  const numeric = Number(value)
  return Number.isFinite(numeric) ? numeric : fallback
}

interface DiffHunk {
  oldText: string
  newText: string
}

interface FileChange {
  path: string
  created: boolean
  additions: number
  deletions: number
  hunks: DiffHunk[]
}

function parseFileChangesFromProgress(progress: AgentProgress[]): FileChange[] {
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

export default function ChatPanel({
  interactions,
  sending,
  sendingMessage,
  sendingImageCount,
  streamedReply,
  streamedResponse,
  agentProgress,
  agentActivitySnapshots,
  thinkingExpanded,
  onThinkingExpandedChange,
  message,
  imageAttachments,
  documentName,
  pendingApproval,
  smartContext,
  agentMode,
  status,
  workspacePath,
  chatEnd,
  welcomeInteraction,
  onSubmit,
  onSelectImage,
  onSelectDocument,
  onPasteImage,
  onReadClipboardImage,
  onSetMessage,
  onSendVoiceMessage,
  onRemoveImage,
  onRemoveDocument,
  onStartWebSearch,
  onCaptureScreen,
  onSetSmartContext,
  onSetAgentMode,
  onSetProvider,
  onSelectWorkspace,
  onApproval,
  settingsConfig,
  onSetModel,
}: ChatPanelProps) {
  const agentActivities = activitiesFrom(agentProgress)
  const activeFallbackNotice = fallbackNotice(streamedResponse)
  const [openActivityIds, setOpenActivityIds] = useState<Record<string, boolean>>({})
  const [openReviewIds, setOpenReviewIds] = useState<Record<string, boolean>>({})
  const [openFileDiffs, setOpenFileDiffs] = useState<Record<string, boolean>>({})
  const [toolMenuOpen, setToolMenuOpen] = useState(false)
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const [dynamicOllamaModels, setDynamicOllamaModels] = useState<string[]>(OLLAMA_MODELS)

  useEffect(() => {
    const fetchOllamaModels = async () => {
      if (status?.activeProvider !== 'ollama') return;
      const host = settingsConfig?.ollamaHost || 'http://localhost:11434';
      const cleanHost = host.endsWith('/') ? host.slice(0, -1) : host;
      try {
        const res = await fetch(`${cleanHost}/api/tags`);
        if (res.ok) {
          const data = await res.json();
          if (data && Array.isArray(data.models)) {
            setDynamicOllamaModels(data.models.map((m: any) => m.name));
            return;
          }
        }
      } catch (err) {
        // fallback to default if fetch fails
      }
      setDynamicOllamaModels(OLLAMA_MODELS);
    }
    fetchOllamaModels();
  }, [status?.activeProvider, settingsConfig?.ollamaHost])

  useEffect(() => {
    if (!sending) {
      setElapsedSeconds(0)
      return
    }

    const startTime = Date.now()
    setElapsedSeconds(0)

    const timer = setInterval(() => {
      const elapsed = Math.round((Date.now() - startTime) / 1000)
      setElapsedSeconds(elapsed)
    }, 1000)

    return () => clearInterval(timer)
  }, [sending])
  const [isRecording, setIsRecording] = useState(false)
  const [voiceMode, setVoiceMode] = useState(false)
  const [voiceTranscript, setVoiceTranscript] = useState('')
  const [voiceAwaitingResponse, setVoiceAwaitingResponse] = useState(false)
  const [speakingText, setSpeakingText] = useState<string | null>(null)
  const toolMenuRef = useRef<HTMLDivElement | null>(null)
  const recognitionRef = useRef<SpeechRecognition | null>(null)
  const mediaRecorderRef = useRef<MediaRecorder | null>(null)
  const mediaStreamRef = useRef<MediaStream | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const vadTimerRef = useRef<number | null>(null)
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const speechRunRef = useRef(0)
  const historyReadyRef = useRef(false)
  const submittedDuringSessionRef = useRef(false)
  const lastAutoSpokenIdRef = useRef<number | string | null>(null)
  const voiceModeRef = useRef(false)
  const sendingRef = useRef(false)
  const voiceAwaitingResponseRef = useRef(false)
  const speakingRef = useRef<string | null>(null)
  const restartTimerRef = useRef<number | null>(null)
  const canSubmit = Boolean(message.trim() || imageAttachments.length > 0 || documentName)
  const sendingImageMarkers = Array.from({ length: sendingImageCount }, (_, index) => `[Image #${index + 1}]`).join(' ')
  const voiceStatus = speakingText ? 'speaking' : (sending || voiceAwaitingResponse) ? 'thinking' : isRecording ? 'listening' : voiceMode ? 'ready' : 'off'
  const voiceStatusLabel = voiceStatus === 'speaking' ? 'Speaking' : voiceStatus === 'thinking' ? 'Thinking' : voiceStatus === 'listening' ? 'Listening' : 'Ready'

  const getAvailableModels = (provider: string) => {
    switch (provider) {
      case 'gemini':
        return GEMINI_MODELS
      case 'openai':
        return OPENAI_MODELS
      case 'openrouter':
        return OPENROUTER_MODELS
      case 'deepseek':
        return DEEPSEEK_MODELS
      case 'anthropic':
        return ANTHROPIC_MODELS
      case 'huggingface':
        return HF_MODELS
      case 'local_openai':
        return LOCAL_MODELS
      case 'ollama':
        return dynamicOllamaModels
      default:
        return []
    }
  }

  const activeProvider = status?.activeProvider ?? ''
  const availableModels = getAvailableModels(activeProvider)

  const getActiveModel = (provider: string) => {
    if (!settingsConfig) return ''
    switch (provider) {
      case 'gemini':
        return settingsConfig.geminiModel
      case 'openai':
        return settingsConfig.openaiModel
      case 'openrouter':
        return settingsConfig.openrouterModel
      case 'deepseek':
        return settingsConfig.deepseekModel
      case 'anthropic':
        return settingsConfig.anthropicModel
      case 'huggingface':
        return settingsConfig.hfModel
      case 'local_openai':
        return settingsConfig.localModelName
      case 'ollama':
        return settingsConfig.ollamaModel
      default:
        return ''
    }
  }
  const activeModel = getActiveModel(activeProvider)
  const workspaceName = workspacePath
    ? workspacePath.split(/[\\/]/).filter(Boolean).pop() || workspacePath
    : 'Select Project'
  const submitOnEnter = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return
    event.preventDefault()
    event.currentTarget.form?.requestSubmit()
  }
  const handleInputKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    submitOnEnter(event)
  }
  const clearRestartTimer = () => {
    if (restartTimerRef.current === null) return
    window.clearTimeout(restartTimerRef.current)
    restartTimerRef.current = null
  }
  const clearVadTimer = () => {
    if (vadTimerRef.current === null) return
    window.clearInterval(vadTimerRef.current)
    vadTimerRef.current = null
  }
  const stopRecognition = () => {
    clearRestartTimer()
    recognitionRef.current?.stop()
    recognitionRef.current = null
    clearVadTimer()
    if (mediaRecorderRef.current?.state === 'recording') {
      mediaRecorderRef.current.stop()
    }
    mediaRecorderRef.current = null
    mediaStreamRef.current?.getTracks().forEach((track) => track.stop())
    mediaStreamRef.current = null
    audioContextRef.current?.close().catch(() => {})
    audioContextRef.current = null
    setIsRecording(false)
  }
  const scheduleVoiceListen = (delayMs = 350) => {
    clearRestartTimer()
    if (!voiceModeRef.current || sendingRef.current || voiceAwaitingResponseRef.current || speakingRef.current) return
    restartTimerRef.current = window.setTimeout(() => {
      restartTimerRef.current = null
      startRecognition(true)
    }, delayMs)
  }
  const cancelSpeech = () => {
    speechRunRef.current += 1
    audioRef.current?.pause()
    audioRef.current = null
    speakingRef.current = null
    if (typeof window !== 'undefined' && window.speechSynthesis) {
      window.speechSynthesis.cancel()
    }
    setSpeakingText(null)
  }
  const speakNative = (text: string, displayText: string) => {
    if (typeof window === 'undefined' || !window.speechSynthesis) {
      setSpeakingText(null)
      return
    }
    const hasThai = /[\u0e00-\u0e7f]/.test(text)
    const utterance = new SpeechSynthesisUtterance(text)
    utterance.lang = hasThai ? 'th-TH' : 'en-US'
    utterance.volume = Math.max(0, Math.min(1, numericSetting(settingsConfig?.ttsVolume, 1)))
    utterance.rate = Math.max(0.1, Math.min(10, numericSetting(settingsConfig?.ttsSpeed, 1)))
    utterance.pitch = Math.max(0, Math.min(2, numericSetting(settingsConfig?.ttsPitch, 1)))
    const voice = window.speechSynthesis.getVoices().find((item) => item.lang.startsWith(hasThai ? 'th' : 'en'))
    if (voice) utterance.voice = voice
    const finishSpeech = () => {
      setSpeakingText((current) => (current === displayText ? null : current))
      speakingRef.current = null
      scheduleVoiceListen(900)
    }
    utterance.onend = finishSpeech
    utterance.onerror = finishSpeech
    speakingRef.current = displayText
    setSpeakingText(displayText)
    window.speechSynthesis.speak(utterance)
  }
  const playGoogleTts = async (text: string, displayText: string, runId: number) => {
    const chunks = await getTtsUrls(text)
    for (const chunk of chunks) {
      if (speechRunRef.current !== runId) return
      await new Promise<void>((resolve, reject) => {
        const audio = new Audio(chunk.url)
        audio.volume = Math.max(0, Math.min(1, numericSetting(settingsConfig?.ttsVolume, 1)))
        audio.playbackRate = Math.max(0.25, Math.min(4, numericSetting(settingsConfig?.ttsSpeed, 1)))
        audioRef.current = audio
        audio.onended = () => resolve()
        audio.onpause = () => resolve()
        audio.onerror = () => reject(new Error('Google TTS playback failed'))
        audio.play().catch(reject)
      })
    }
    if (speechRunRef.current === runId) {
      setSpeakingText((current) => (current === displayText ? null : current))
      speakingRef.current = null
      scheduleVoiceListen(900)
    }
  }
  const speak = (text: string) => {
    const speechText = cleanSpeechText(text)
    if (!speechText) return
    if (speakingText === text) {
      cancelSpeech()
      return
    }

    cancelSpeech()
    const runId = speechRunRef.current
    speakingRef.current = text
    setSpeakingText(text)
    if (settingsConfig?.ttsProvider === 'google') {
      playGoogleTts(speechText, text, runId).catch((error) => {
        console.warn('Google TTS failed, falling back to native speech synthesis:', error)
        if (speechRunRef.current === runId) speakNative(speechText, text)
      })
      return
    }
    speakNative(speechText, text)
  }
  const blobToDataUri = (blob: Blob) => new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onerror = () => reject(reader.error ?? new Error('Failed to read audio recording'))
    reader.onload = () => resolve(String(reader.result))
    reader.readAsDataURL(blob)
  })
  const preferredAudioMimeType = () => {
    const candidates = ['audio/webm;codecs=opus', 'audio/webm', 'audio/ogg;codecs=opus', 'audio/ogg']
    return candidates.find((mimeType) => MediaRecorder.isTypeSupported(mimeType)) ?? ''
  }
  const startAudioRecording = async (autoSend: boolean) => {
    if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === 'undefined') {
      alert('Sorry, this system cannot access the microphone or record audio in this WebView')
      voiceModeRef.current = false
      setVoiceMode(false)
      return
    }

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
      const mimeType = preferredAudioMimeType()
      const recorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined)
      const chunks: Blob[] = []
      let heardVoice = false
      let quietSince = 0
      const startedAt = Date.now()
      const audioContext = new AudioContext()
      const source = audioContext.createMediaStreamSource(stream)
      const analyser = audioContext.createAnalyser()
      const samples = new Uint8Array(analyser.fftSize)
      source.connect(analyser)

      mediaStreamRef.current = stream
      mediaRecorderRef.current = recorder
      audioContextRef.current = audioContext
      setVoiceTranscript('Listening to microphone...')

      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) chunks.push(event.data)
      }
      recorder.onstop = async () => {
        clearVadTimer()
        mediaRecorderRef.current = null
        mediaStreamRef.current?.getTracks().forEach((track) => track.stop())
        mediaStreamRef.current = null
        audioContextRef.current?.close().catch(() => {})
        audioContextRef.current = null
        setIsRecording(false)
        if (!autoSend || !voiceModeRef.current || chunks.length === 0) return
        if (!heardVoice) {
          setVoiceTranscript('No speech detected')
          scheduleVoiceListen(700)
          return
        }
        const blob = new Blob(chunks, { type: recorder.mimeType || 'audio/webm' })
        const audioDataUri = await blobToDataUri(blob)
        setVoiceTranscript('Sent audio to AI')
        voiceAwaitingResponseRef.current = true
        setVoiceAwaitingResponse(true)
        onSendVoiceMessage('', audioDataUri)
          .catch((error) => console.error('Voice audio message failed', error))
          .finally(() => {
            voiceAwaitingResponseRef.current = false
            setVoiceAwaitingResponse(false)
            scheduleVoiceListen()
          })
      }

      recorder.start()
      setIsRecording(true)
      vadTimerRef.current = window.setInterval(() => {
        analyser.getByteTimeDomainData(samples)
        let peak = 0
        for (const sample of samples) {
          peak = Math.max(peak, Math.abs(sample - 128))
        }
        const now = Date.now()
        if (peak > 12) {
          heardVoice = true
          quietSince = 0
          setVoiceTranscript('Listening...')
        } else if (heardVoice) {
          quietSince = quietSince || now
        }
        const silenceElapsed = quietSince ? now - quietSince : 0
        const totalElapsed = now - startedAt
        if ((heardVoice && silenceElapsed > 1300) || totalElapsed > 12000) {
          recorder.stop()
        }
      }, 120)
    } catch (error) {
      console.error('Failed to record microphone audio', error)
      setIsRecording(false)
      voiceModeRef.current = false
      setVoiceMode(false)
      alert('Failed to open microphone. Please check microphone permissions for this app')
    }
  }
  const startRecognition = (autoSend: boolean) => {
    if (recognitionRef.current || sendingRef.current || voiceAwaitingResponseRef.current || speakingRef.current) return

    const SpeechRecognitionApi = window.SpeechRecognition || window.webkitSpeechRecognition
    if (!SpeechRecognitionApi) {
      startAudioRecording(autoSend)
      return
    }

    let sentTranscript = false

    try {
      const recognition = new SpeechRecognitionApi()
      recognition.continuous = false
      recognition.interimResults = true
      recognition.lang = settingsConfig?.language === 'en' ? 'en-US' : 'th-TH'
      recognition.onstart = () => setIsRecording(true)
      recognition.onresult = (event: SpeechRecognitionEvent) => {
        let interimText = ''
        let finalText = ''
        for (let index = event.resultIndex; index < event.results.length; index += 1) {
          const transcript = event.results[index]?.[0]?.transcript ?? ''
          if (event.results[index]?.isFinal) finalText += transcript
          else interimText += transcript
        }
        const displayText = (finalText || interimText).trim()
        if (displayText) setVoiceTranscript(displayText)
        const resultText = finalText.trim()
        if (!resultText) return
        sentTranscript = true
        recognition.stop()
        setVoiceTranscript(resultText)
        if (autoSend) {
          voiceAwaitingResponseRef.current = true
          setVoiceAwaitingResponse(true)
          onSendVoiceMessage(resultText)
            .catch((error) => console.error('Voice message failed', error))
            .finally(() => {
              voiceAwaitingResponseRef.current = false
              setVoiceAwaitingResponse(false)
              scheduleVoiceListen()
            })
        } else {
          onSetMessage(message.trim() ? `${message.trimEnd()} ${resultText}` : resultText)
        }
      }
      recognition.onerror = (event: Event) => {
        console.error('Speech recognition error', event)
        setIsRecording(false)
      }
      recognition.onend = () => {
        recognitionRef.current = null
        setIsRecording(false)
        if (autoSend && voiceModeRef.current && !sentTranscript) scheduleVoiceListen()
      }
      recognitionRef.current = recognition
      recognition.start()
    } catch (error) {
      console.error('Failed to start speech recognition', error)
      recognitionRef.current = null
      setIsRecording(false)
    }
  }
  const toggleRecording = () => {
    if (voiceMode) {
      voiceModeRef.current = false
      voiceAwaitingResponseRef.current = false
      setVoiceMode(false)
      setVoiceAwaitingResponse(false)
      setVoiceTranscript('')
      stopRecognition()
      cancelSpeech()
      return
    }

    voiceModeRef.current = true
    setVoiceMode(true)
    setVoiceAwaitingResponse(false)
    setVoiceTranscript('')
    startRecognition(true)
  }
  const resizeInput = (element: HTMLTextAreaElement) => {
    element.style.height = 'auto'
    element.style.height = `${Math.min(element.scrollHeight, 120)}px`
  }
  useEffect(() => {
    return () => {
      clearRestartTimer()
      cancelSpeech()
      stopRecognition()
    }
  }, [])
  useEffect(() => {
    voiceModeRef.current = voiceMode
    if (!voiceMode) {
      clearRestartTimer()
      setVoiceTranscript('')
    }
  }, [voiceMode])
  useEffect(() => {
    sendingRef.current = sending
  }, [sending])
  useEffect(() => {
    voiceAwaitingResponseRef.current = voiceAwaitingResponse
  }, [voiceAwaitingResponse])
  useEffect(() => {
    speakingRef.current = speakingText
  }, [speakingText])
  useEffect(() => {
    if (!voiceMode || sending || voiceAwaitingResponse || speakingText || isRecording) return
    scheduleVoiceListen()
  }, [voiceMode, sending, voiceAwaitingResponse, speakingText, isRecording])
  useEffect(() => {
    if (sending) submittedDuringSessionRef.current = true
  }, [sending])
  useEffect(() => {
    if (interactions.length === 0) return
    const latest = interactions[interactions.length - 1]
    if (!historyReadyRef.current) {
      historyReadyRef.current = true
      if (!submittedDuringSessionRef.current) {
        lastAutoSpokenIdRef.current = latest?.id ?? null
        return
      }
    }
    if (sending) return
    if (!settingsConfig?.enableVoiceReply && !voiceMode) {
      lastAutoSpokenIdRef.current = latest?.id ?? null
      return
    }
    if (!latest?.aiText || latest.id === lastAutoSpokenIdRef.current) return
    lastAutoSpokenIdRef.current = latest.id
    speak(latest.aiText)
  }, [interactions, sending, settingsConfig?.enableVoiceReply])
  useEffect(() => {
    if (!toolMenuOpen) return
    const closeMenu = (event: MouseEvent) => {
      if (toolMenuRef.current?.contains(event.target as Node)) return
      setToolMenuOpen(false)
    }
    window.addEventListener('mousedown', closeMenu)
    return () => window.removeEventListener('mousedown', closeMenu)
  }, [toolMenuOpen])
  useEffect(() => {
    const handleWindowPaste = (event: globalThis.ClipboardEvent) => {
      if (!event.clipboardData) return
      if (onPasteImage(event.clipboardData)) {
        event.preventDefault()
        event.stopPropagation()
      } else {
        window.setTimeout(() => void onReadClipboardImage(), 0)
      }
    }
    window.addEventListener('paste', handleWindowPaste, true)
    return () => window.removeEventListener('paste', handleWindowPaste, true)
  }, [onPasteImage, onReadClipboardImage])
  useEffect(() => {
    const handleWindowKeyDown = (event: globalThis.KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'v') return
      window.setTimeout(() => void onReadClipboardImage(), 0)
    }
    window.addEventListener('keydown', handleWindowKeyDown, true)
    return () => window.removeEventListener('keydown', handleWindowKeyDown, true)
  }, [onReadClipboardImage])
  useEffect(() => {
    if (message) return
    const input = document.getElementById('chat-input') as HTMLTextAreaElement | null
    if (input) input.style.height = ''
  }, [message])

  const openImagePicker = () => {
    setToolMenuOpen(false)
    document.getElementById('vision-file-input')?.click()
  }
  const openDocumentPicker = () => {
    setToolMenuOpen(false)
    document.getElementById('document-file-input')?.click()
  }
  const startWebSearch = () => {
    setToolMenuOpen(false)
    onStartWebSearch()
  }
  const appendWorkspaceReference = (reference: string) => {
    const trimmed = reference.trim()
    if (!trimmed) return
    onSetMessage(message.trim() ? `${message.trimEnd()} ${trimmed} ` : `${trimmed} `)
  }
  const handleWorkspaceDrop = (event: DragEvent<HTMLElement>) => {
    const reference =
      event.dataTransfer.getData('application/x-mint-workspace-path') ||
      event.dataTransfer.getData('text/plain')
    if (!reference.trim().startsWith('@')) return
    event.preventDefault()
    appendWorkspaceReference(reference)
  }
  const isEmptyChat = interactions.length === 0 && !sending && !pendingApproval
  const renderCompletedActivity = (interaction: any) => {
    const interactionId = String(interaction.id)
    const activityView = activitiesFrom(agentActivitySnapshots[interactionId] ?? [])
    if (activityView.items.length === 0) return null
    const isOpen = Boolean(openActivityIds[interactionId])
    return (
      <div className="agent-activity-history">
        <button
          type="button"
          className="agent-activity-toggle"
          aria-expanded={isOpen}
          onClick={() => setOpenActivityIds((current) => ({ ...current, [interactionId]: !current[interactionId] }))}
        >
          <span>{activityView.summary}</span>
          <span aria-hidden="true">{isOpen ? '^' : '>'}</span>
        </button>
        {isOpen && (
          <div className="agent-activity-card agent-activity-card-history">
            {renderAgentActivityTable(activityView)}
          </div>
        )}
      </div>
    )
  }

  const renderFileChanges = (interaction: any) => {
    const interactionId = String(interaction.id)
    const progress = agentActivitySnapshots[interactionId] ?? []
    const changes = parseFileChangesFromProgress(progress)
    if (changes.length === 0) return null

    const totalAdditions = changes.reduce((sum, c) => sum + c.additions, 0)
    const totalDeletions = changes.reduce((sum, c) => sum + c.deletions, 0)
    const isOpen = Boolean(openReviewIds[interactionId])

    return (
      <div className="file-changes-summary-container" style={{ marginBottom: '8px' }}>
        <button
          type="button"
          className="agent-activity-toggle"
          aria-expanded={isOpen}
          onClick={() => setOpenReviewIds((current) => ({ ...current, [interactionId]: !current[interactionId] }))}
          style={{ display: 'flex', alignItems: 'center', gap: '6px', color: '#10b981', fontWeight: 500 }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ marginRight: '2px' }}>
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
            <polyline points="22 4 12 14.01 9 11.01" />
          </svg>
          <span>
            {changes.length} {changes.length === 1 ? 'file' : 'files'} changed
            {totalAdditions > 0 && <span style={{ color: '#10b981', marginLeft: '6px' }}>+{totalAdditions}</span>}
            {totalDeletions > 0 && <span style={{ color: '#ef4444', marginLeft: '4px' }}>-{totalDeletions}</span>}
          </span>
          <span aria-hidden="true">{isOpen ? '^' : '>'}</span>
        </button>

        {isOpen && (
          <div className="agent-activity-card" style={{ border: '1px solid rgba(16, 185, 129, 0.2)', borderRadius: '8px', padding: '10px', background: 'rgba(15, 23, 42, 0.6)', marginTop: '4px' }}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
              {changes.map((change) => {
                const fileKey = `${interactionId}-${change.path}`
                const isDiffOpen = Boolean(openFileDiffs[fileKey])
                const fileName = change.path.split('/').pop() || change.path
                const dirPath = change.path.includes('/') ? change.path.substring(0, change.path.lastIndexOf('/')) : ''

                return (
                  <div key={change.path} style={{ borderBottom: '1px solid rgba(255, 255, 255, 0.05)', paddingBottom: '4px' }}>
                    <div
                      onClick={() => setOpenFileDiffs((current) => ({ ...current, [fileKey]: !current[fileKey] }))}
                      style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', cursor: 'pointer', padding: '4px 6px', borderRadius: '4px', background: 'rgba(255, 255, 255, 0.02)' }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94a3b8" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
                          <polyline points="14 2 14 8 20 8" />
                        </svg>
                        <span style={{ fontSize: '0.82rem', fontWeight: 600, color: change.created ? '#10b981' : '#cbd5e1' }}>
                          {fileName}
                          {dirPath && <span style={{ fontSize: '0.72rem', color: '#64748b', fontWeight: 400, marginLeft: '6px' }}>{dirPath}</span>}
                          {change.created && <span style={{ fontSize: '0.7rem', color: '#10b981', marginLeft: '6px', padding: '1px 4px', border: '1px solid rgba(16, 185, 129, 0.3)', borderRadius: '3px', background: 'rgba(16, 185, 129, 0.1)' }}>new</span>}
                        </span>
                      </div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '0.76rem' }}>
                        {change.additions > 0 && <span style={{ color: '#10b981' }}>+{change.additions}</span>}
                        {change.deletions > 0 && <span style={{ color: '#ef4444' }}>-{change.deletions}</span>}
                        <span style={{ color: '#64748b', transform: isDiffOpen ? 'rotate(90deg)' : 'none', display: 'inline-block', transition: 'transform 0.15s' }}>&gt;</span>
                      </div>
                    </div>

                    {isDiffOpen && (
                      <div style={{ marginTop: '6px', background: '#0b0f19', borderRadius: '6px', padding: '8px', border: '1px solid rgba(255, 255, 255, 0.08)', overflowX: 'auto', maxHeight: '300px' }}>
                        {change.hunks.map((hunk, hIdx) => (
                          <div key={hIdx} style={{ fontSize: '0.74rem', fontFamily: 'monospace', lineHeight: '1.4', marginBottom: hIdx < change.hunks.length - 1 ? '10px' : 0 }}>
                            {hunk.oldText && (
                              <div style={{ background: 'rgba(239, 68, 68, 0.12)', borderLeft: '3px solid #ef4444', padding: '4px 6px', color: '#fca5a5', whiteSpace: 'pre-wrap' }}>
                                {hunk.oldText.split('\n').map((line, lIdx) => (
                                  <div key={lIdx}>- {line}</div>
                                ))}
                              </div>
                            )}
                            {hunk.newText && (
                              <div style={{ background: 'rgba(16, 185, 129, 0.12)', borderLeft: '3px solid #10b981', padding: '4px 6px', color: '#a7f3d0', whiteSpace: 'pre-wrap' }}>
                                {hunk.newText.split('\n').map((line, lIdx) => (
                                  <div key={lIdx}>+ {line}</div>
                                ))}
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          </div>
        )}
      </div>
    )
  }

  const renderActiveFileChanges = () => {
    const changes = parseFileChangesFromProgress(agentProgress)
    if (changes.length === 0) return null

    const totalAdditions = changes.reduce((sum, c) => sum + c.additions, 0)
    const totalDeletions = changes.reduce((sum, c) => sum + c.deletions, 0)
    const isOpen = Boolean(openReviewIds['active-run'])

    return (
      <div className="message ai-message agent-activity-message" style={{ marginTop: '4px', marginBottom: '8px' }}>
        <div className="agent-activity-card" style={{ border: '1px solid rgba(16, 185, 129, 0.25)', borderRadius: '8px', padding: '10px', background: 'rgba(15, 23, 42, 0.6)' }}>
          <button
            type="button"
            className="agent-activity-toggle"
            aria-expanded={isOpen}
            onClick={() => setOpenReviewIds((current) => ({ ...current, 'active-run': !current['active-run'] }))}
            style={{ display: 'flex', alignItems: 'center', gap: '6px', color: '#10b981', fontWeight: 500, border: 0, background: 'transparent', padding: 0 }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ marginRight: '2px' }}>
              <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
              <polyline points="22 4 12 14.01 9 11.01" />
            </svg>
            <span>
              {changes.length} {changes.length === 1 ? 'file' : 'files'} changed in this run
              {totalAdditions > 0 && <span style={{ color: '#10b981', marginLeft: '6px' }}>+{totalAdditions}</span>}
              {totalDeletions > 0 && <span style={{ color: '#ef4444', marginLeft: '4px' }}>-{totalDeletions}</span>}
            </span>
            <span aria-hidden="true">{isOpen ? '^' : '>'}</span>
          </button>

          {isOpen && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginTop: '8px' }}>
              {changes.map((change) => {
                const fileKey = `active-${change.path}`
                const isDiffOpen = Boolean(openFileDiffs[fileKey])
                const fileName = change.path.split('/').pop() || change.path
                const dirPath = change.path.includes('/') ? change.path.substring(0, change.path.lastIndexOf('/')) : ''

                return (
                  <div key={change.path} style={{ borderBottom: '1px solid rgba(255, 255, 255, 0.05)', paddingBottom: '4px' }}>
                    <div
                      onClick={() => setOpenFileDiffs((current) => ({ ...current, [fileKey]: !current[fileKey] }))}
                      style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', cursor: 'pointer', padding: '4px 6px', borderRadius: '4px', background: 'rgba(255, 255, 255, 0.02)' }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94a3b8" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
                          <polyline points="14 2 14 8 20 8" />
                        </svg>
                        <span style={{ fontSize: '0.82rem', fontWeight: 600, color: change.created ? '#10b981' : '#cbd5e1' }}>
                          {fileName}
                          {dirPath && <span style={{ fontSize: '0.72rem', color: '#64748b', fontWeight: 400, marginLeft: '6px' }}>{dirPath}</span>}
                          {change.created && <span style={{ fontSize: '0.7rem', color: '#10b981', marginLeft: '6px', padding: '1px 4px', border: '1px solid rgba(16, 185, 129, 0.3)', borderRadius: '3px', background: 'rgba(16, 185, 129, 0.1)' }}>new</span>}
                        </span>
                      </div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '0.76rem' }}>
                        {change.additions > 0 && <span style={{ color: '#10b981' }}>+{change.additions}</span>}
                        {change.deletions > 0 && <span style={{ color: '#ef4444' }}>-{change.deletions}</span>}
                        <span style={{ color: '#64748b', transform: isDiffOpen ? 'rotate(90deg)' : 'none', display: 'inline-block', transition: 'transform 0.15s' }}>&gt;</span>
                      </div>
                    </div>

                    {isDiffOpen && (
                      <div style={{ marginTop: '6px', background: '#0b0f19', borderRadius: '6px', padding: '8px', border: '1px solid rgba(255, 255, 255, 0.08)', overflowX: 'auto', maxHeight: '300px' }}>
                        {change.hunks.map((hunk, hunkIdx) => (
                          <div key={hunkIdx} style={{ fontSize: '0.74rem', fontFamily: 'monospace', lineHeight: '1.4', marginBottom: hunkIdx < change.hunks.length - 1 ? '10px' : 0 }}>
                            {hunk.oldText && (
                              <div style={{ background: 'rgba(239, 68, 68, 0.12)', borderLeft: '3px solid #ef4444', padding: '4px 6px', color: '#fca5a5', whiteSpace: 'pre-wrap' }}>
                                {hunk.oldText.split('\n').map((line, lIdx) => (
                                  <div key={lIdx}>- {line}</div>
                                ))}
                              </div>
                            )}
                            {hunk.newText && (
                              <div style={{ background: 'rgba(16, 185, 129, 0.12)', borderLeft: '3px solid #10b981', padding: '4px 6px', color: '#a7f3d0', whiteSpace: 'pre-wrap' }}>
                                {hunk.newText.split('\n').map((line, lIdx) => (
                                  <div key={lIdx}>+ {line}</div>
                                ))}
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          )}
        </div>
      </div>
    )
  }

  return (
    <section className={`conversation-panel ${isEmptyChat ? 'is-empty' : ''}`}>
      <div className="chat-header">
        <div className="chat-header-title">
          <img src="./assets/icon.png" alt="Logo" className="chat-header-logo" />
          <span>Mint Agent</span>
        </div>
      </div>
      <div className="chat-container">
        {interactions.map((interaction) => {
          const isSystemEvent = interaction.provider === 'system' && interaction.model === 'provider_change';
          if (isSystemEvent) {
            return (
              <div key={interaction.id} className="system-event-divider">
                <div className="system-event-line" />
                <div className="system-event-pill">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                    <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z" />
                  </svg>
                  <span>{interaction.userText}</span>
                </div>
                <div className="system-event-line" />
              </div>
            );
          }
          return (
            <div key={interaction.id} style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
              {interaction.userText && (
                <div className="message user-message">
                  <div className="bubble-wrapper">
                    <div className="message-bubble">{renderFormattedMessage(interaction.userText)}</div>
                    <div className="message-time"><span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span></div>
                  </div>
                </div>
              )}
              <div className="message ai-message">
              <div className="bubble-wrapper">
                {renderCompletedActivity(interaction)}
                {renderFileChanges(interaction)}
                {(() => {
                  const progress = agentActivitySnapshots[String(interaction.id)] ?? interaction.agentActivity ?? []
                  const thoughts = thoughtsFrom(progress)
                  return (
                    <ThinkingBlock
                      blockKey={String(interaction.id)}
                      thoughts={thoughts}
                      expanded={thinkingExpanded[String(interaction.id)] ?? false}
                      onExpandedChange={onThinkingExpandedChange}
                      showEmptyHint={hasAgentToolActivity(progress) && thoughts.length === 0}
                    />
                  )
                })()}
                <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>{renderFormattedMessage(interaction.aiText)}</div>
                <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <button className="provider-badge">{interaction.provider} • {interaction.model}</button>
                  {fallbackNotice(interaction) && <span className="provider-fallback-notice">{fallbackNotice(interaction)}</span>}
                  <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                  <button
                    type="button"
                    className={`tts-btn ${speakingText === interaction.aiText ? 'is-speaking' : ''}`}
                    onClick={() => speak(interaction.aiText)}
                    title={speakingText === interaction.aiText ? 'Stop reading' : 'Read aloud'}
                  >
                    {renderSpeakerIcon(speakingText === interaction.aiText)}
                  </button>
                </div>
              </div>
            </div>
          </div>
        );
      })}

        {sending && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
            <div className="message user-message"><div className="bubble-wrapper"><div className="message-bubble">{sendingImageMarkers ? renderFormattedMessage(`${sendingMessage} ${sendingImageMarkers}`) : renderFormattedMessage(sendingMessage)}</div></div></div>
            {agentMode && agentActivities.items.length > 0 && (
              <div className="message ai-message agent-activity-message">
                <div className="agent-activity-card">
                  <div className="agent-activity-header">
                    <span>{agentActivities.summary}</span>
                    <span className="agent-activity-status" data-state={pendingApproval ? 'approval' : 'active'}>
                      {pendingApproval ? 'Waiting for approval' : 'Working'}
                    </span>
                  </div>
                  {renderAgentActivityTable(agentActivities)}
                </div>
              </div>
            )}
            {renderActiveFileChanges()}
            <div className="message ai-message thinking-message">
              <div className="bubble-wrapper">
                <ThinkingBlock
                  blockKey="live"
                  thoughts={thoughtsFrom(agentProgress)}
                  isLive={true}
                  expanded={thinkingExpanded.live ?? true}
                  onExpandedChange={onThinkingExpandedChange}
                  showEmptyHint={
                    agentMode
                    && hasAgentToolActivity(agentProgress)
                    && thoughtsFrom(agentProgress).length === 0
                    && !streamedReply
                  }
                />
                <div className="message-bubble">
                  <span>
                    {streamedReply ? (
                      renderFormattedMessage(streamedReply)
                    ) : (
                      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: 'var(--text-soft, #94a3b8)' }}>
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'inline-block', flexShrink: 0 }}>
                          <circle cx="12" cy="12" r="10" stroke="rgba(255, 255, 255, 0.12)" />
                          <path d="M12 2a10 10 0 0 1 10 10" stroke="var(--accent)" strokeLinecap="round">
                            <animateTransform
                              attributeName="transform"
                              type="rotate"
                              from="0 12 12"
                              to="360 12 12"
                              dur="0.9s"
                              repeatCount="indefinite"
                            />
                          </path>
                        </svg>
                        <span>Thinking for {elapsedSeconds}s</span>
                      </div>
                    )}
                  </span>
                </div>
                {streamedResponse && (
                  <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <button className="provider-badge">{badge(streamedResponse.provider, streamedResponse.model)}</button>
                    {activeFallbackNotice && <span className="provider-fallback-notice">{activeFallbackNotice}</span>}
                    {streamedReply && (
                      <button
                        type="button"
                        className={`tts-btn ${speakingText === streamedReply ? 'is-speaking' : ''}`}
                        onClick={() => speak(streamedReply)}
                        title={speakingText === streamedReply ? 'Stop reading' : 'Read aloud'}
                      >
                        {renderSpeakerIcon(speakingText === streamedReply)}
                      </button>
                    )}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {pendingApproval && (() => {
          const details = renderApprovalDetails(pendingApproval.approval)
          const writeFile = pendingApproval.approval?.WriteFile
          const applyPatch = pendingApproval.approval?.ApplyPatch
          const diffText = writeFile?.diff || applyPatch?.diff

          return (
            <div className="message ai-message" style={{ width: '100%' }}>
              <div className="bubble-wrapper" style={{ width: '100%' }}>
                <div className="action-card approval-card" data-tier={details.isDangerous ? 'dangerous' : undefined} style={{ width: '100%' }}>
                  <div className="approval-card-content" style={{ width: '100%' }}>
                    <div className="approval-card-title">{details.title}</div>
                    <div className="approval-card-body" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>{details.body}</div>
                    {diffText ? (
                      <div className="approval-card-diff-container" style={{ marginTop: '8px', width: '100%' }}>
                        <div style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-soft, #94a3b8)', marginBottom: '4px' }}>Diff:</div>
                        {renderDiff(diffText)}
                      </div>
                    ) : (
                      details.reason && <div className="approval-card-reason" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>{details.reason}</div>
                    )}
                  </div>
                  <div className="approval-card-actions">
                    <button type="button" className="approval-btn approval-btn-approve" onClick={() => onApproval(true)}>Approve</button>
                    <button type="button" className="approval-btn" style={{ backgroundColor: 'rgba(16, 185, 129, 0.22)', borderColor: 'rgba(16, 185, 129, 0.4)', color: '#a7f3d0' }} onClick={() => onApproval(true, true)}>Approve this session</button>
                    <button type="button" className="approval-btn approval-btn-cancel" onClick={() => onApproval(false)}>Cancel</button>
                  </div>
                </div>
              </div>
            </div>
          )
        })()}
        <div ref={chatEnd} />
      </div>

      <div className="input-area">
        {isEmptyChat && <div className="empty-chat-prompt">Mint Agent is ready to work</div>}
        <button type="button" className="workspace-select-btn" onClick={onSelectWorkspace}>
          <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
              <path d="M3 6h7l2 2h9v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"></path>
            </svg>
          </span>
          <span>{workspaceName}</span>
          <span aria-hidden="true">⌄</span>
        </button>
        <div className="smart-context-bar">
          <div className="smart-context-label" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <label className="toggle-switch">
              <input type="checkbox" checked={smartContext} onChange={(event) => onSetSmartContext(event.target.checked)} />
              <span className="slider round" />
            </label>
            <span>Smart Context (Auto-Screen)</span>
          </div>
          <div className="smart-context-label" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <label className="toggle-switch">
              <input type="checkbox" checked={agentMode} onChange={(event) => onSetAgentMode(event.target.checked)} />
              <span className="slider round" />
            </label>
            <span>Agent Mode</span>
          </div>
        </div>
        {voiceMode && (
          <div className="voice-mode-bar" data-state={voiceStatus}>
            <span className="voice-mode-dot" />
            <span>{voiceStatusLabel}</span>
            {voiceTranscript && <span className="voice-mode-transcript">{voiceTranscript}</span>}
          </div>
        )}

        <form
          id="chat-form"
          onSubmit={onSubmit}
          onDragOver={(event) => {
            if (event.dataTransfer.types.includes('application/x-mint-workspace-path')) {
              event.preventDefault()
              event.dataTransfer.dropEffect = 'copy'
            }
          }}
          onDrop={handleWorkspaceDrop}
          onPaste={(event: ClipboardEvent<HTMLElement>) => {
            if (onPasteImage(event.clipboardData)) event.preventDefault()
          }}
        >
          {(imageAttachments.length > 0 || documentName) && (
            <div className="mint-attachment">
              {imageAttachments.map((attachment, idx) => (
                <div className="mint-image-attachment" key={idx}>
                  <img className="mint-image-preview" src={attachment.previewDataUri || attachment.dataUri} alt={attachment.name || 'Image attachment'} />
                  <button className="mint-attachment-remove" type="button" onClick={() => onRemoveImage(idx)} aria-label="Remove image">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </button>
                </div>
              ))}
              {documentName && (
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center', color: 'var(--text-soft)' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                      <polyline points="14 2 14 8 20 8"></polyline>
                    </svg>
                  </span>
                  <span style={{ fontSize: '0.76rem', color: 'var(--text-soft)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: '220px' }}>{documentName}</span>
                  <button type="button" onClick={onRemoveDocument} style={{ background: 'transparent', border: 0, color: '#ef4444', cursor: 'pointer', display: 'inline-flex', alignItems: 'center' }}>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </button>
                </div>
              )}
            </div>
          )}
          <textarea
            id="chat-input"
            value={message}
            onChange={(event) => {
              resizeInput(event.currentTarget)
              onSetMessage(event.target.value)
            }}
            onKeyDown={handleInputKeyDown}
            onDrop={handleWorkspaceDrop}
            placeholder="Ask anything, @ to mention, / for actions"
            rows={1}
          />
          <div className="chat-tool-menu-wrap" ref={toolMenuRef}>
            <button id="chat-tool-btn" type="button" aria-haspopup="menu" aria-expanded={toolMenuOpen} onClick={() => setToolMenuOpen((open) => !open)} style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <line x1="12" y1="5" x2="12" y2="19"></line>
                <line x1="5" y1="12" x2="19" y2="12"></line>
              </svg>
            </button>
            {toolMenuOpen && (
              <div className="chat-tool-menu" role="menu">
                <button type="button" role="menuitem" onClick={openImagePicker} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                      <circle cx="8.5" cy="8.5" r="1.5"></circle>
                      <polyline points="21 15 16 10 5 21"></polyline>
                    </svg>
                  </span>
                  <span>Add image</span>
                </button>
                <button type="button" role="menuitem" onClick={openDocumentPicker} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                      <polyline points="14 2 14 8 20 8"></polyline>
                    </svg>
                  </span>
                  <span>Add file</span>
                </button>
                <button type="button" role="menuitem" onClick={startWebSearch} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <circle cx="11" cy="11" r="8"></circle>
                      <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                    </svg>
                  </span>
                  <span>Search web</span>
                </button>
              </div>
            )}
          </div>
          <input id="vision-file-input" type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={onSelectImage} style={{ display: 'none' }} />
          <input id="document-file-input" type="file" accept="application/pdf,.pdf" onChange={onSelectDocument} style={{ display: 'none' }} />
          <button id="screen-capture-btn" type="button" onClick={onCaptureScreen} aria-label="Capture screen">
            <span className="screen-capture-eye" aria-hidden="true" />
          </button>
          <div className="chat-provider-select" style={{ display: 'flex', gap: '4px', padding: 0, background: 'transparent', border: 0, width: '100%', height: '32px' }}>
            <select 
              value={status?.activeProvider ?? ''} 
              onChange={(event) => onSetProvider(event.target.value)}
              style={{
                flex: 1,
                minWidth: '65px',
                height: '100%',
                padding: '0 20px 0 6px',
                background: 'transparent',
                border: 0,
                color: 'var(--text-soft)',
                fontSize: '0.78rem',
                outline: 'none',
                cursor: 'pointer',
                fontFamily: 'inherit',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap'
              }}
            >
              {status?.availableProviders.map((provider) => {
                let displayName = provider
                if (provider === 'gemini') displayName = 'Gemini'
                else if (provider === 'openai') displayName = 'OpenAI'
                else if (provider === 'openrouter') displayName = 'OpenRouter'
                else if (provider === 'deepseek') displayName = 'DeepSeek'
                else if (provider === 'anthropic') displayName = 'Claude'
                else if (provider === 'huggingface') displayName = 'HF'
                else if (provider === 'local_openai') displayName = 'Local'
                else if (provider === 'ollama') displayName = 'Ollama'
                return <option key={provider} value={provider}>{displayName}</option>
              })}
            </select>
            {(availableModels.length > 0 || activeModel) && (
              <select 
                value={activeModel} 
                onChange={(event) => onSetModel(event.target.value)}
                style={{
                  flex: 1.2,
                  minWidth: '85px',
                  height: '100%',
                  padding: '0 20px 0 6px',
                  background: 'transparent',
                  border: 0,
                  color: 'var(--text-soft)',
                  fontSize: '0.78rem',
                  outline: 'none',
                  cursor: 'pointer',
                  fontFamily: 'inherit',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap'
                }}
              >
                {availableModels.map((model) => (
                  <option key={model} value={model}>{model.split('/').pop()}</option>
                ))}
                {!availableModels.includes(activeModel) && activeModel && (
                  <option value={activeModel}>{activeModel.split('/').pop()}</option>
                )}
              </select>
            )}
          </div>
          <button
            id="mic-btn"
            className={`${isRecording ? 'is-recording' : ''} ${voiceMode ? 'voice-mode-active' : ''}`}
            type="button"
            onClick={toggleRecording}
            title={voiceMode ? 'Disable voice conversation' : 'Enable voice conversation'}
            style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}
          >
            {isRecording ? (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="4" y="4" width="16" height="16" rx="2" ry="2"></rect>
              </svg>
            ) : (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"></path>
                <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
                <line x1="12" y1="19" x2="12" y2="23"></line>
                <line x1="8" y1="23" x2="16" y2="23"></line>
              </svg>
            )}
          </button>
          <button id="send-btn" type="submit" disabled={sending || !canSubmit} style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="22" y1="2" x2="11" y2="13"></line>
              <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
            </svg>
          </button>
        </form>
      </div>
      <p className="input-disclaimer">
        Mint Agent is an AI gateway. Responses via third-party APIs. Verify critical info.
      </p>
    </section>
  )
}
