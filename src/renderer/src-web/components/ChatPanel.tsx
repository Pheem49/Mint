import { useEffect, useRef, useState, type ChangeEvent, type ClipboardEvent, type FormEvent, type KeyboardEvent, type RefObject, type DragEvent } from 'react'
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
  const target = path || query || command || name || tool || action.replaceAll('_', ' ')
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

interface ChatPanelProps {
  interactions: any[]
  sending: boolean
  sendingMessage: string
  sendingImageCount: number
  streamedReply: string
  streamedResponse: ChatResponse | null
  agentProgress: AgentProgress[]
  agentActivitySnapshots: Record<string, AgentProgress[]>
  message: string
  imageAttachments: Array<{ dataUri: string; name: string; previewDataUri?: string }>
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
  onSendVoiceMessage: (message: string, audioDataUri?: string | null) => Promise<void>
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

export default function ChatPanel({
  interactions,
  sending,
  sendingMessage,
  sendingImageCount,
  streamedReply,
  streamedResponse,
  agentProgress,
  agentActivitySnapshots,
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
  onSendVoiceMessage,
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
  const [openActivityIds, setOpenActivityIds] = useState<Record<string, boolean>>({})
  const [toolMenuOpen, setToolMenuOpen] = useState(false)
  const toolMenuRef = useRef<HTMLDivElement | null>(null)
  const canSubmit = Boolean(message.trim() || imageAttachments.length > 0 || documentName)
  const sendingImageMarkers = Array.from({ length: sendingImageCount }, (_, index) => `[Image #${index + 1}]`).join(' ')

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

  const [isRecording, setIsRecording] = useState(false)
  const [voiceMode, setVoiceMode] = useState(false)
  const [voiceTranscript, setVoiceTranscript] = useState('')
  const [voiceAwaitingResponse, setVoiceAwaitingResponse] = useState(false)
  const [speakingText, setSpeakingText] = useState<string | null>(null)
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
  const voiceStatus = speakingText ? 'speaking' : (sending || voiceAwaitingResponse) ? 'thinking' : isRecording ? 'listening' : voiceMode ? 'ready' : 'off'
  const voiceStatusLabel = voiceStatus === 'speaking' ? 'กำลังตอบ' : voiceStatus === 'thinking' ? 'กำลังคิด' : voiceStatus === 'listening' ? 'กำลังฟัง' : 'พร้อมฟัง'

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
    if (mediaRecorderRef.current?.state === 'recording') mediaRecorderRef.current.stop()
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
    if (typeof window !== 'undefined' && window.speechSynthesis) window.speechSynthesis.cancel()
    setSpeakingText(null)
  }
  const speakNative = (text: string, displayText: string) => {
    if (typeof window === 'undefined' || !window.speechSynthesis) {
      setSpeakingText(null)
      speakingRef.current = null
      scheduleVoiceListen(900)
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
    if (chunks.length === 0) throw new Error('No TTS URLs available')
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
      scheduleVoiceListen()
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
      alert('ขออภัย ระบบนี้ไม่สามารถเข้าถึงไมค์หรืออัดเสียงในเบราว์เซอร์นี้ได้')
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
      setVoiceTranscript('กำลังฟังเสียงจากไมค์')

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
          setVoiceTranscript('ยังไม่ได้ยินเสียงพูด')
          scheduleVoiceListen(700)
          return
        }
        const blob = new Blob(chunks, { type: recorder.mimeType || 'audio/webm' })
        const audioDataUri = await blobToDataUri(blob)
        setVoiceTranscript('ส่งเสียงให้ AI แล้ว')
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
          setVoiceTranscript('กำลังฟัง...')
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
      alert('เปิดไมค์ไม่สำเร็จ กรุณาตรวจสิทธิ์ microphone ของเบราว์เซอร์')
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
    element.style.height = `${Math.min(element.scrollHeight, 120)}px`
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

  return (
    <section
      className={`conversation-panel ${isEmptyChat ? 'is-empty' : ''}`}
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
                {renderCompletedActivity(interaction)}
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
        {isEmptyChat && <div className="empty-chat-prompt">Mint Agent is ready to work</div>}
        <div className="smart-context-bar">
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
          onPaste={(event: ClipboardEvent<HTMLElement>) => {
            if (onPasteImage(event.clipboardData)) event.preventDefault()
          }}
        >
          {(imageAttachments.length > 0 || documentName) && (
            <div className="mint-attachment">
              {imageAttachments.map((attachment, idx) => (
                <div className="mint-image-attachment" key={idx}>
                  <img className="mint-image-preview" src={attachment.previewDataUri || attachment.dataUri} alt={attachment.name || 'Image attachment'} />
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
                else if (provider === 'openrouter') displayName = 'OpenRouter'
                else if (provider === 'deepseek') displayName = 'DeepSeek'
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
            className={`${isRecording ? 'is-recording' : ''} ${voiceMode ? 'voice-mode-active' : ''}`}
            type="button"
            onClick={toggleRecording}
            title={voiceMode ? 'ปิดโหมดสนทนาเสียง' : 'เปิดโหมดสนทนาเสียง'}
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
          <button id="send-btn" type="submit" disabled={sending || !canSubmit} style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
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
