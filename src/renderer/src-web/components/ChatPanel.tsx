import { useEffect, useRef, useState, type ChangeEvent, type ClipboardEvent, type FormEvent, type KeyboardEvent, type RefObject } from 'react'
import {
  type AgentProgress,
  type ChatResponse,
  type PictureEntry,
  type RuntimeStatus,
  convertFileSrc,
} from '../tauri'
import type { DashboardView } from './DashboardSidebar'

interface ApprovalDetails {
  title: string
  body: string
  reason?: string
  isDangerous: boolean
}

function badge(provider: string, model: string) {
  return [provider, model].filter(Boolean).join(' / ')
}

interface AgentActivity {
  text: string
  state: 'active' | 'done' | 'error'
}

function activityDetail(input: Record<string, unknown>, key: string) {
  const value = input[key]
  return typeof value === 'string' && value.trim() ? value : ''
}

function describeTool(action: string, input: Record<string, unknown>) {
  const path = activityDetail(input, 'path')
  const query = activityDetail(input, 'query')
  const command = activityDetail(input, 'command')
  const labels: Record<string, string> = {
    list_files: path ? `List ${path}` : 'List workspace files',
    read_file: path ? `Read ${path}` : 'Read file',
    search_code: query ? `Search code for "${query}"` : 'Search code',
    symbols: path ? `Inspect symbols in ${path}` : 'Inspect symbols',
    semantic_index: path ? `Index semantic code in ${path}` : 'Index semantic code',
    semantic_search: query ? `Semantic search for "${query}"` : 'Semantic code search',
    knowledge_search: query ? `Search knowledge for "${query}"` : 'Search local knowledge',
    web_search: query ? `Search the web for "${query}"` : 'Search the web',
    memory_recall: query ? `Recall memory for "${query}"` : 'Recall memory',
    git_status: 'Read git status',
    git_diff: path ? `Read git diff for ${path}` : 'Read git diff',
    git_log: 'Read git log',
    git_branch: 'Read git branch',
    create_plan: 'Create plan',
    update_plan: 'Update plan',
    request_user_approval: 'Request approval',
    ask_user: query ? `Ask: ${query}` : 'Ask user',
    detect_project: path ? `Detect project in ${path}` : 'Detect project',
    list_tests: path ? `List tests in ${path}` : 'List tests',
    read_diagnostics: path ? `Read diagnostics in ${path}` : 'Read diagnostics',
    view_image: path ? `View image ${path}` : 'View image',
    note_write: path ? `Write note ${path}` : 'Write note',
    run_plugin: activityDetail(input, 'name') ? `Run plugin ${activityDetail(input, 'name')}` : 'Run plugin',
    mcp_tool: activityDetail(input, 'tool') ? `Call MCP tool ${activityDetail(input, 'tool')}` : 'Call MCP tool',
    run_shell: command ? `Run \`${command}\`` : 'Run shell command',
    verify: command ? `Verify with \`${command}\`` : 'Verify changes',
    apply_patch: path ? `Patch ${path}` : 'Apply code patch',
    write_file: path ? `Write ${path}` : 'Write file',
  }
  return labels[action] ?? `Use ${action.replaceAll('_', ' ')}`
}

function activitiesFrom(progress: AgentProgress[]) {
  const activities: AgentActivity[] = []
  for (const event of progress) {
    if (event.type === 'Thought' && event.data.thought.trim()) {
      activities.push({ text: event.data.thought, state: 'done' })
    } else if (event.type === 'ToolStart') {
      activities.push({ text: describeTool(event.data.action, event.data.input), state: 'active' })
    } else if (event.type === 'ToolEnd') {
      for (let index = activities.length - 1; index >= 0; index -= 1) {
        if (activities[index].state !== 'active') continue
        activities[index].state = event.data.result.startsWith('Error:') ? 'error' : 'done'
        break
      }
    }
  }
  return activities.slice(-8)
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

interface ChatPanelProps {
  interactions: any[]
  sending: boolean
  sendingMessage: string
  sendingHasImage: boolean
  streamedReply: string
  streamedResponse: ChatResponse | null
  agentProgress: AgentProgress[]
  message: string
  imageAttachments: Array<{ dataUri: string; name: string }>
  documentName: string
  pendingApproval: any | null
  smartContext: boolean
  agentMode: boolean
  status: RuntimeStatus | null
  chatEnd: RefObject<HTMLDivElement | null>
  welcomeInteraction: any
  onSubmit: (event: FormEvent<HTMLFormElement>) => void
  onSelectImage: (event: ChangeEvent<HTMLInputElement>) => void
  onSelectDocument: (event: ChangeEvent<HTMLInputElement>) => void
  onPasteImage: (clipboardData: DataTransfer) => boolean
  onReadClipboardImage: () => Promise<boolean>
  onSetMessage: (message: string) => void
  onRemoveImage: (idx: number) => void
  onRemoveDocument: () => void
  onStartWebSearch: () => void
  onCaptureScreen: () => void
  onSetSmartContext: (enabled: boolean) => void
  onSetAgentMode: (enabled: boolean) => void
  onSetProvider: (provider: string) => void
  onApproval: (approved: boolean) => void
}

function renderFormattedMessage(text: string) {
  const displayText = readableAssistantText(text)
  if (!displayText) return null
  const parts = displayText.split(/\*\*([\s\S]*?)\*\*/g)
  return parts.map((part, index) => {
    if (index % 2 === 1) {
      return (
        <strong key={index} className="chat-bold-highlight">
          {part}
        </strong>
      )
    }
    return part
  })
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

export default function ChatPanel({
  interactions,
  sending,
  sendingMessage,
  sendingHasImage,
  streamedReply,
  streamedResponse,
  agentProgress,
  message,
  imageAttachments,
  documentName,
  pendingApproval,
  smartContext,
  agentMode,
  status,
  chatEnd,
  welcomeInteraction,
  onSubmit,
  onSelectImage,
  onSelectDocument,
  onPasteImage,
  onReadClipboardImage,
  onSetMessage,
  onRemoveImage,
  onRemoveDocument,
  onStartWebSearch,
  onCaptureScreen,
  onSetSmartContext,
  onSetAgentMode,
  onSetProvider,
  onApproval,
}: ChatPanelProps) {
  const agentActivities = activitiesFrom(agentProgress)
  const [toolMenuOpen, setToolMenuOpen] = useState(false)
  const toolMenuRef = useRef<HTMLDivElement | null>(null)
  const submitOnEnter = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return
    event.preventDefault()
    event.currentTarget.form?.requestSubmit()
  }
  const handleInputKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    submitOnEnter(event)
  }
  const resizeInput = (element: HTMLTextAreaElement) => {
    element.style.height = 'auto'
    element.style.height = `${Math.min(element.scrollHeight, 72)}px`
  }
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

  return (
    <section className="conversation-panel">
      <div className="chat-container">
        {interactions.length === 0 && !sending && (
          <div className="message ai-message" style={{ marginBottom: '16px' }}>
            <div className="bubble-wrapper">
              <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>{renderFormattedMessage(welcomeInteraction.aiText)}</div>
              <div className="message-time">
                <button className="provider-badge">{welcomeInteraction.provider} • {welcomeInteraction.model}</button>
                <span>14:44</span>
              </div>
            </div>
          </div>
        )}

        {interactions.map((interaction) => (
          <div key={interaction.id} style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
            {interaction.isSystemEvent ? (
              <div className="system-event" style={{ background: 'rgba(16, 185, 129, 0.06)', border: '1px solid rgba(16, 185, 129, 0.2)', borderRadius: '8px', padding: '10px 14px', color: '#a7f3d0', fontSize: '0.82rem', lineHeight: '1.45', alignSelf: 'stretch' }}>
                {interaction.userText}
              </div>
            ) : interaction.userText && (
              <div className="message user-message">
                <div className="bubble-wrapper">
                  <div className="message-bubble">{renderFormattedMessage(interaction.userText)}</div>
                  <div className="message-time"><span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span></div>
                </div>
              </div>
            )}
            <div className="message ai-message">
              <div className="bubble-wrapper">
                <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>{renderFormattedMessage(interaction.aiText)}</div>
                <div className="message-time">
                  <button className="provider-badge">{interaction.provider} • {interaction.model}</button>
                  <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                </div>
              </div>
            </div>
          </div>
        ))}

        {sending && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
            <div className="message user-message"><div className="bubble-wrapper"><div className="message-bubble">{sendingHasImage ? renderFormattedMessage(`${sendingMessage} [Image #1]`) : renderFormattedMessage(sendingMessage)}</div></div></div>
            {agentMode && agentActivities.length > 0 && (
              <div className="message ai-message agent-activity-message">
                <div className="agent-activity-card">
                  <div className="agent-activity-header">
                    <span>Agent activity</span>
                    <span className="agent-activity-status" data-state={pendingApproval ? 'approval' : 'active'}>
                      {pendingApproval ? 'Waiting for approval' : 'Working'}
                    </span>
                  </div>
                  <div className="agent-activity-list">
                    {agentActivities.map((activity, index) => (
                      <div className="agent-activity-item" data-state={activity.state} key={`${index}-${activity.text}`}>
                        <span className="agent-activity-dot" />
                        <span className="agent-activity-text">{activity.text}</span>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            )}
            <div className="message ai-message thinking-message">
              <div className="bubble-wrapper">
                <div className="message-bubble"><span>{streamedReply ? renderFormattedMessage(streamedReply) : 'Thinking...'}</span></div>
                {streamedResponse && <div className="message-time"><button className="provider-badge">{badge(streamedResponse.provider, streamedResponse.model)}</button></div>}
              </div>
            </div>
          </div>
        )}

        {pendingApproval && (() => {
          const details = renderApprovalDetails(pendingApproval.approval)
          return (
            <div className="message ai-message" style={{ width: '100%' }}>
              <div className="bubble-wrapper" style={{ width: '100%' }}>
                <div className="action-card approval-card" data-tier={details.isDangerous ? 'dangerous' : undefined} style={{ width: '100%' }}>
                  <div className="approval-card-content">
                    <div className="approval-card-title">{details.title}</div>
                    <div className="approval-card-body" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>{details.body}</div>
                    {details.reason && <div className="approval-card-reason" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>{details.reason}</div>}
                  </div>
                  <div className="approval-card-actions">
                    <button type="button" className="approval-btn approval-btn-approve" onClick={() => onApproval(true)}>Approve</button>
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

        <form
          id="chat-form"
          onSubmit={onSubmit}
          onPaste={(event: ClipboardEvent<HTMLElement>) => {
            if (onPasteImage(event.clipboardData)) event.preventDefault()
          }}
        >
          {(imageAttachments.length > 0 || documentName) && (
            <div className="mint-attachment">
              {imageAttachments.map((attachment, idx) => (
                <div className="mint-image-attachment" key={idx}>
                  <img className="mint-image-preview" src={attachment.dataUri} alt={attachment.name || 'Image attachment'} />
                  <button className="mint-attachment-remove" type="button" onClick={() => onRemoveImage(idx)} aria-label="Remove image">×</button>
                </div>
              ))}
              {documentName && (
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
                  <span aria-hidden="true" style={{ fontSize: '1rem' }}>📄</span>
                  <span style={{ fontSize: '0.76rem', color: 'var(--text-soft)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: '220px' }}>{documentName}</span>
                  <button type="button" onClick={onRemoveDocument} style={{ background: 'transparent', border: 0, color: '#ef4444', cursor: 'pointer' }}>✕</button>
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
            placeholder="Ask anything, @ to mention, / for actions"
            rows={1}
          />
          <div className="chat-tool-menu-wrap" ref={toolMenuRef}>
            <button id="chat-tool-btn" type="button" aria-haspopup="menu" aria-expanded={toolMenuOpen} onClick={() => setToolMenuOpen((open) => !open)}>+</button>
            {toolMenuOpen && (
              <div className="chat-tool-menu" role="menu">
                <button type="button" role="menuitem" onClick={openImagePicker}>
                  <span aria-hidden="true">⌕</span>
                  <span>Add image</span>
                </button>
                <button type="button" role="menuitem" onClick={openDocumentPicker}>
                  <span aria-hidden="true">□</span>
                  <span>Add file</span>
                </button>
                <button type="button" role="menuitem" onClick={startWebSearch}>
                  <span aria-hidden="true">○</span>
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
          <select className="chat-provider-select" value={status?.activeProvider ?? ''} onChange={(event) => onSetProvider(event.target.value)}>
            {status?.availableProviders.map((provider) => <option key={provider} value={provider}>{provider}</option>)}
          </select>
          <button id="mic-btn" type="button" onClick={() => alert("Voice transcription coming soon!")}>🎙</button>
          <button id="send-btn" type="submit" disabled={sending || !message.trim()}>➤</button>
        </form>
      </div>
    </section>
  )
}

interface PicturesLibraryProps {
  view: DashboardView
  pictures: PictureEntry[]
  onSetView: (view: DashboardView) => void
}

export function PicturesLibrary({ view, pictures, onSetView }: PicturesLibraryProps) {
  if (view !== 'pictures') return null

  return (
    <section className="pictures-library">
      <header className="pictures-header">
        <div><span className="pictures-kicker">Gallery</span><h2>Saved Pictures</h2></div>
        <button className="pictures-close-btn" onClick={() => onSetView('chat')}>Close Gallery</button>
      </header>
      {pictures.length === 0 ? (
        <div className="pictures-empty">
          <div className="pictures-empty-icon" style={{ display: 'flex', justifyContent: 'center', marginBottom: '12px', opacity: 0.3 }}>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline>
            </svg>
          </div>
          <p>No saved pictures yet</p>
          <span>Images appear here after a message with an attachment is sent successfully.</span>
        </div>
      ) : (
        <div className="pictures-grid">
          {pictures.map((picture) => (
            <a className="picture-card" href={picture.url} target="_blank" rel="noreferrer" key={picture.id} onClick={(event) => { event.preventDefault(); window.settingsApi?.openExternal(picture.url || '') }}>
              <img src={convertFileSrc(picture.path)} alt={picture.message || picture.filename} loading="lazy" decoding="async" />
              <div className="picture-card-meta">{picture.message || picture.filename}</div>
            </a>
          ))}
        </div>
      )}
    </section>
  )
}
