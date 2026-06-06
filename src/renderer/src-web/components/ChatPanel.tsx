import { useEffect, useRef, useState, type ChangeEvent, type ClipboardEvent, type FormEvent, type KeyboardEvent, type RefObject, type DragEvent } from 'react'
import {
  type AgentProgress,
  type ChatResponse,
  type PictureEntry,
  type RuntimeStatus,
  convertFileSrc,
} from '../tauri'
import type { DashboardView } from './DashboardSidebar'

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

const OLLAMA_MODELS = [
  'llama3:latest',
  'llama3.1:latest',
  'llama3.2:latest',
  'gemma2:latest',
  'mistral:latest',
  'phi3:latest',
  'qwen2.5:latest'
]

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
  onToggleMobileSidebar: () => void
  settingsConfig: any
  onSetModel: (model: string) => void
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

function renderSpeakerIcon(isSpeaking: boolean) {
  if (isSpeaking) {
    return (
      <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
        <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
        <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
      </svg>
    )
  }
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
      <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
    </svg>
  )
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
  onToggleMobileSidebar,
  settingsConfig,
  onSetModel,
}: ChatPanelProps) {
  const agentActivities = activitiesFrom(agentProgress)
  const [toolMenuOpen, setToolMenuOpen] = useState(false)
  const toolMenuRef = useRef<HTMLDivElement | null>(null)

  const getAvailableModels = (provider: string) => {
    switch (provider) {
      case 'gemini':
        return GEMINI_MODELS
      case 'openai':
        return OPENAI_MODELS
      case 'anthropic':
        return ANTHROPIC_MODELS
      case 'huggingface':
        return HF_MODELS
      case 'local_openai':
        return LOCAL_MODELS
      case 'ollama':
        return OLLAMA_MODELS
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

  // Voice Input (Speech to Text)
  const [isRecording, setIsRecording] = useState(false)
  const recognitionRef = useRef<any>(null)

  const toggleRecording = () => {
    if (isRecording) {
      if (recognitionRef.current) {
        recognitionRef.current.stop()
      }
      setIsRecording(false)
    } else {
      const SpeechRecognition = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition
      if (!SpeechRecognition) {
        alert("ขออภัยค่ะ เบราว์เซอร์ของคุณไม่รองรับการพิมพ์ด้วยเสียง (Web Speech API)")
        return
      }

      try {
        const recognition = new SpeechRecognition()
        recognition.continuous = false
        recognition.interimResults = false
        recognition.lang = 'th-TH'

        recognition.onstart = () => {
          setIsRecording(true)
        }

        recognition.onresult = (event: any) => {
          const resultText = event.results[0][0].transcript
          if (resultText) {
            onSetMessage((message ? message + ' ' : '') + resultText)
          }
        }

        recognition.onerror = (event: any) => {
          console.error('Speech recognition error', event.error)
          setIsRecording(false)
        }

        recognition.onend = () => {
          setIsRecording(false)
        }

        recognitionRef.current = recognition
        recognition.start()
      } catch (err) {
        console.error('Failed to start speech recognition', err)
        setIsRecording(false)
      }
    }
  }

  // Text to Speech (TTS)
  const [speakingText, setSpeakingText] = useState<string | null>(null)

  const speak = (text: string) => {
    if (typeof window === 'undefined' || !window.speechSynthesis) return

    if (speakingText === text) {
      window.speechSynthesis.cancel()
      setSpeakingText(null)
      return
    }

    window.speechSynthesis.cancel()
    const cleanText = text
      .replace(/\*\*([\s\S]*?)\*\*/g, '$1')
      .replace(/[*_`#]/g, '')
      .trim()

    const utterance = new SpeechSynthesisUtterance(cleanText)
    const hasThai = /[\u0e00-\u0e7f]/.test(cleanText)
    utterance.lang = hasThai ? 'th-TH' : 'en-US'

    const voices = window.speechSynthesis.getVoices()
    const voice = voices.find(v => v.lang.startsWith(hasThai ? 'th' : 'en'))
    if (voice) {
      utterance.voice = voice
    }

    utterance.onend = () => {
      setSpeakingText(null)
    }
    utterance.onerror = () => {
      setSpeakingText(null)
    }

    setSpeakingText(text)
    window.speechSynthesis.speak(utterance)
  }

  useEffect(() => {
    return () => {
      if (typeof window !== 'undefined' && window.speechSynthesis) {
        window.speechSynthesis.cancel()
      }
      if (recognitionRef.current) {
        recognitionRef.current.stop()
      }
    }
  }, [])

  // Drag and Drop Zone Overlay
  const [isDragging, setIsDragging] = useState(false)
  const dragCounter = useRef(0)

  const handleDragEnter = (e: DragEvent<HTMLElement>) => {
    e.preventDefault()
    if (e.dataTransfer?.types?.includes('Files')) {
      dragCounter.current++
      setIsDragging(true)
    }
  }

  const handleDragLeave = (e: DragEvent<HTMLElement>) => {
    e.preventDefault()
    if (e.dataTransfer?.types?.includes('Files')) {
      dragCounter.current--
      if (dragCounter.current === 0) {
        setIsDragging(false)
      }
    }
  }

  const handleDragOver = (e: DragEvent<HTMLElement>) => {
    e.preventDefault()
  }

  const handleDrop = (e: DragEvent<HTMLElement>) => {
    e.preventDefault()
    dragCounter.current = 0
    setIsDragging(false)

    const files = e.dataTransfer?.files
    if (files && files.length > 0) {
      const file = files[0]
      if (file.type.startsWith('image/')) {
        const input = document.getElementById('vision-file-input') as HTMLInputElement | null
        if (input) {
          const dt = new DataTransfer()
          dt.items.add(file)
          input.files = dt.files
          const event = { target: input } as ChangeEvent<HTMLInputElement>
          onSelectImage(event)
        }
      } else if (file.type === 'application/pdf' || file.name.toLowerCase().endsWith('.pdf')) {
        const input = document.getElementById('document-file-input') as HTMLInputElement | null
        if (input) {
          const dt = new DataTransfer()
          dt.items.add(file)
          input.files = dt.files
          const event = { target: input } as ChangeEvent<HTMLInputElement>
          onSelectDocument(event)
        }
      }
    }
  }
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
    <section
      className="conversation-panel"
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      style={{ position: 'relative' }}
    >
      <div className="chat-header">
        <button
          className="mobile-menu-btn"
          type="button"
          onClick={onToggleMobileSidebar}
          aria-label="Toggle menu"
        >
          ☰
        </button>
        <div className="chat-header-title">
          <img src="./assets/icon.png" alt="Logo" className="chat-header-logo" />
          <span>Agent Mint</span>
        </div>
      </div>

      {isDragging && (
        <div
          className="drag-drop-overlay"
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            background: 'rgba(15, 23, 42, 0.82)',
            backdropFilter: 'blur(8px)',
            border: '2px dashed var(--accent)',
            borderRadius: '16px',
            margin: '12px',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            color: 'white',
            zIndex: 1000,
            pointerEvents: 'auto',
          }}
        >
          <div style={{ fontSize: '3.5rem', marginBottom: '16px' }}>🖼️</div>
          <div style={{ fontSize: '1.25rem', fontWeight: 'bold', letterSpacing: '0.5px' }}>วางไฟล์เพื่อแนบข้อมูล</div>
          <div style={{ fontSize: '0.85rem', color: '#94a3b8', marginTop: '8px' }}>รองรับรูปภาพ (PNG, JPEG, WebP, GIF) และไฟล์ PDF</div>
        </div>
      )}

      <div className="chat-container">
        {interactions.length === 0 && !sending && (
          <div className="message ai-message" style={{ marginBottom: '16px' }}>
            <div className="bubble-wrapper">
              <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>{renderFormattedMessage(welcomeInteraction.aiText)}</div>
              <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <button className="provider-badge">{welcomeInteraction.provider} • {welcomeInteraction.model}</button>
                <span>14:44</span>
                <button
                  type="button"
                  className={`tts-btn ${speakingText === welcomeInteraction.aiText ? 'is-speaking' : ''}`}
                  onClick={() => speak(welcomeInteraction.aiText)}
                  style={{
                    background: 'transparent',
                    border: 'none',
                    color: speakingText === welcomeInteraction.aiText ? 'var(--accent)' : 'var(--text-soft)',
                    cursor: 'pointer',
                    fontSize: '0.85rem',
                    padding: '2px',
                    display: 'inline-flex',
                    alignItems: 'center',
                    opacity: 0.7,
                    transition: 'all 0.2s',
                  }}
                  title={speakingText === welcomeInteraction.aiText ? "หยุดอ่านออกเสียง" : "อ่านออกเสียง"}
                >
                  {renderSpeakerIcon(speakingText === welcomeInteraction.aiText)}
                </button>
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
                <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <button className="provider-badge">{interaction.provider} • {interaction.model}</button>
                  <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                  <button
                    type="button"
                    className={`tts-btn ${speakingText === interaction.aiText ? 'is-speaking' : ''}`}
                    onClick={() => speak(interaction.aiText)}
                    style={{
                      background: 'transparent',
                      border: 'none',
                      color: speakingText === interaction.aiText ? 'var(--accent)' : 'var(--text-soft)',
                      cursor: 'pointer',
                      fontSize: '0.85rem',
                      padding: '2px',
                      display: 'inline-flex',
                      alignItems: 'center',
                      opacity: 0.7,
                      transition: 'all 0.2s',
                    }}
                    title={speakingText === interaction.aiText ? "หยุดอ่านออกเสียง" : "อ่านออกเสียง"}
                  >
                    {renderSpeakerIcon(speakingText === interaction.aiText)}
                  </button>
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
                {streamedResponse && (
                  <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <button className="provider-badge">{badge(streamedResponse.provider, streamedResponse.model)}</button>
                    {streamedReply && (
                      <button
                        type="button"
                        className={`tts-btn ${speakingText === streamedReply ? 'is-speaking' : ''}`}
                        onClick={() => speak(streamedReply)}
                        style={{
                          background: 'transparent',
                          border: 'none',
                          color: speakingText === streamedReply ? 'var(--accent)' : 'var(--text-soft)',
                          cursor: 'pointer',
                          fontSize: '0.85rem',
                          padding: '2px',
                          display: 'inline-flex',
                          alignItems: 'center',
                          opacity: 0.7,
                          transition: 'all 0.2s',
                        }}
                        title={speakingText === streamedReply ? "หยุดอ่านออกเสียง" : "อ่านออกเสียง"}
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

        <div ref={chatEnd} />
      </div>

      <div className="input-area">
        <div className="smart-context-bar">
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
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center', color: 'var(--accent)' }}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                      <polyline points="14 2 14 8 20 8"></polyline>
                      <line x1="16" y1="13" x2="8" y2="13"></line>
                      <line x1="16" y1="17" x2="8" y2="17"></line>
                    </svg>
                  </span>
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
            <button id="chat-tool-btn" type="button" aria-haspopup="menu" aria-expanded={toolMenuOpen} onClick={() => setToolMenuOpen((open) => !open)} style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                <line x1="12" y1="5" x2="12" y2="19"></line>
                <line x1="5" y1="12" x2="19" y2="12"></line>
              </svg>
            </button>
            {toolMenuOpen && (
              <div className="chat-tool-menu" role="menu">
                <button type="button" role="menuitem" onClick={openImagePicker}>
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                      <circle cx="8.5" cy="8.5" r="1.5"></circle>
                      <polyline points="21 15 16 10 5 21"></polyline>
                    </svg>
                  </span>
                  <span>Add image</span>
                </button>
                <button type="button" role="menuitem" onClick={openDocumentPicker}>
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                      <polyline points="14 2 14 8 20 8"></polyline>
                    </svg>
                  </span>
                  <span>Add file</span>
                </button>
                <button type="button" role="menuitem" onClick={startWebSearch}>
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <circle cx="12" cy="12" r="10"></circle>
                      <line x1="2" y1="12" x2="22" y2="12"></line>
                      <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
                    </svg>
                  </span>
                  <span>Search web</span>
                </button>
              </div>
            )}
          </div>
          <input id="vision-file-input" type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={onSelectImage} style={{ display: 'none' }} />
          <input id="document-file-input" type="file" accept="application/pdf,.pdf" onChange={onSelectDocument} style={{ display: 'none' }} />
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
                else if (provider === 'anthropic') displayName = 'Claude'
                else if (provider === 'huggingface') displayName = 'HF'
                else if (provider === 'local_openai') displayName = 'Local'
                else if (provider === 'ollama') displayName = 'Ollama'
                return <option key={provider} value={provider}>{displayName}</option>
              })}
            </select>
            {availableModels.length > 0 && (
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
            className={isRecording ? 'is-recording' : ''}
            type="button"
            onClick={toggleRecording}
            title={isRecording ? "หยุดบันทึกเสียง" : "สั่งการด้วยเสียง"}
            style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}
          >
            {isRecording ? (
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="4" y="4" width="16" height="16" rx="2" ry="2"></rect>
              </svg>
            ) : (
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"></path>
                <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
                <line x1="12" y1="19" x2="12" y2="23"></line>
                <line x1="8" y1="23" x2="16" y2="23"></line>
              </svg>
            )}
          </button>
          <button id="send-btn" type="submit" disabled={sending || !message.trim()} style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="22" y1="2" x2="11" y2="13"></line>
              <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
            </svg>
          </button>
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
