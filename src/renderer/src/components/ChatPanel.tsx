import { useEffect, useRef, useState, type ChangeEvent, type ClipboardEvent, type DragEvent, type FormEvent, type KeyboardEvent, type RefObject } from 'react'
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
  sendingImageCount: number
  streamedReply: string
  streamedResponse: ChatResponse | null
  agentProgress: AgentProgress[]
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
  onApproval: (approved: boolean) => void
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

export default function ChatPanel({
  interactions,
  sending,
  sendingMessage,
  sendingImageCount,
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
  const [toolMenuOpen, setToolMenuOpen] = useState(false)
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
  const voiceStatusLabel = voiceStatus === 'speaking' ? 'กำลังตอบ' : voiceStatus === 'thinking' ? 'กำลังคิด' : voiceStatus === 'listening' ? 'กำลังฟัง' : 'พร้อมฟัง'

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
      alert('ขออภัย ระบบนี้ไม่สามารถเข้าถึงไมค์หรืออัดเสียงใน WebView นี้ได้')
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
      alert('เปิดไมค์ไม่สำเร็จ กรุณาตรวจสิทธิ์ microphone ของแอป')
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
    onSetMessage(message.trim() ? `${message.trimEnd()} ${trimmed}` : trimmed)
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

  return (
    <section className={`conversation-panel ${isEmptyChat ? 'is-empty' : ''}`}>
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
                <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>{renderFormattedMessage(interaction.aiText)}</div>
                <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <button className="provider-badge">{interaction.provider} • {interaction.model}</button>
                  <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                  <button
                    type="button"
                    className={`tts-btn ${speakingText === interaction.aiText ? 'is-speaking' : ''}`}
                    onClick={() => speak(interaction.aiText)}
                    title={speakingText === interaction.aiText ? 'หยุดอ่านออกเสียง' : 'อ่านออกเสียง'}
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
                        title={speakingText === streamedReply ? 'หยุดอ่านออกเสียง' : 'อ่านออกเสียง'}
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
    </section>
  )
}
