import { useEffect, useMemo, useRef, useState, useCallback, Fragment, type ChangeEvent, type ClipboardEvent, type DragEvent, type FormEvent, type KeyboardEvent, type RefObject } from 'react'
import { hasAgentToolActivity, thoughtsFrom, parseFileChangesFromProgress } from '../agentProgress'
import {
  GEMINI_MODELS,
  OPENAI_MODELS,
  ANTHROPIC_MODELS,
  OPENROUTER_MODELS,
  DEEPSEEK_MODELS,
  HF_MODELS,
  LOCAL_MODELS,
  OLLAMA_MODELS,
  GEMINI_LIVE_VOICES,
} from '../constants/models'
import { badge, providerLabel, fallbackNotice } from '../utils/providers'
import { activitiesFrom, parseWebSearchSources, type AgentActivity, type AgentActivityView } from '../utils/agentActivity'
import { SLASH_COMMANDS } from '../constants/slashCommands'
import { AgentActivityTable } from './AgentActivityTable'
import { ChatCodeBlock } from './ChatCodeBlock'
import { renderApprovalDetails, renderDiff, type ApprovalDetails } from '../utils/approval'
import { ApprovalCard } from './ApprovalCard'
import { renderFormattedMessage, readableAssistantText, cleanSpeechText, renderSpeakerIcon, renderCopyIcon } from '../utils/markdown'
import { ThinkingBlock } from './ThinkingBlock'
import SourcesBlock from './SourcesBlock'
import ChatMessageItem from './ChatMessageItem'
import { AgentActivityDrawer } from './AgentActivityDrawer'
import type { DiffHunk, FileChange } from '../types'
import { numericSetting, shouldShowSessionDivider, formatSessionDividerLabel } from '../utils/ui'
import { useVoiceInput } from '@/voiceInput'
import { useGeminiLiveVoice } from '../utils/useGeminiLiveVoice'
import GeminiLiveOverlay from './GeminiLiveOverlay'
import { isSupportedDocument, SUPPORTED_DOCUMENT_ACCEPT } from '../utils/documentTypes'

import {
  APP_ICON_PATH,
  listLearnedSkills,
  type LearnedSkill,
  type AgentProgress,
  type ChatResponse,
  type RuntimeStatus,
  getTtsUrls,
  startGeminiLiveSession,
  sendGeminiLiveAudioChunk,
  stopGeminiLiveSession,
} from '@/tauri'


interface ChatPanelProps {
  interactions: any[]
  sending: boolean
  sendingMessage: string
  sendingImageCount: number
  sendingVideoCount?: number
  streamedReply: string
  streamedResponse: ChatResponse | null
  agentProgress: AgentProgress[]
  agentActivitySnapshots: Record<string, AgentProgress[]>
  thinkingExpanded: Record<string, boolean>
  onThinkingExpandedChange: (key: string, open: boolean) => void
  message: string
  imageAttachments: Array<{ dataUri: string; name: string; previewDataUri?: string; objectUrl?: string }>
  videoAttachments: Array<{ dataUri: string; name: string }>
  documentName: string
  pendingApproval: any | null
  smartContext: boolean
  agentMode: boolean
  /** Desktop only — plan-mode-approval UI isn't available on web. */
  planMode?: boolean
  status: RuntimeStatus | null
  /** Desktop only — web has no local workspace-folder concept to select from. */
  workspacePath?: string
  chatEnd: RefObject<HTMLDivElement | null>
  welcomeInteraction: any
  onSubmit: (event: FormEvent<HTMLFormElement>) => void
  onSelectImage: (event: ChangeEvent<HTMLInputElement>) => void
  onSelectVideo: (event: ChangeEvent<HTMLInputElement>) => void
  onSelectDocument: (event: ChangeEvent<HTMLInputElement>) => void
  onPasteImage: (clipboardData: DataTransfer) => boolean
  onSetMessage: (message: string) => void
  onSendVoiceMessage: (message: string, audioDataUri?: string | null) => Promise<void>
  onRemoveImage: (idx: number) => void
  onRemoveVideo: (idx: number) => void
  onRemoveDocument: () => void
  onStartWebSearch: () => void
  onCaptureScreen: () => void
  onSetSmartContext: (enabled: boolean) => void
  onSetAgentMode: (enabled: boolean) => void
  /** Desktop only — see `planMode`. */
  onSetPlanMode?: (enabled: boolean) => void
  onSetProvider: (provider: string) => void
  /** Desktop only — see `workspacePath`. */
  onSelectWorkspace?: () => void
  onApproval: (approved: boolean, autoApproveSession?: boolean, answer?: string) => void
  settingsConfig: any
  onSetModel: (model: string) => void
  onCancelMessage: () => void
  onClearMessages: () => void
  onSetGeminiLiveVoice: (voice: string) => Promise<void>
  /** Web only — desktop's window has no mobile-width sidebar to toggle. */
  onToggleMobileSidebar?: () => void
}


export default function ChatPanel({
  interactions,
  sending,
  sendingMessage,
  sendingImageCount,
  sendingVideoCount,
  streamedReply,
  streamedResponse,
  agentProgress,
  agentActivitySnapshots,
  thinkingExpanded,
  onThinkingExpandedChange,
  message,
  imageAttachments,
  videoAttachments,
  documentName,
  pendingApproval,
  smartContext,
  agentMode,
  planMode,
  status,
  workspacePath,
  chatEnd,
  welcomeInteraction,
  onSubmit,
  onSelectImage,
  onSelectVideo,
  onSelectDocument,
  onPasteImage,
  onSetMessage,
  onSendVoiceMessage,
  onRemoveImage,
  onRemoveVideo,
  onRemoveDocument,
  onStartWebSearch,
  onCaptureScreen,
  onSetSmartContext,
  onSetAgentMode,
  onSetPlanMode,
  onSetProvider,
  onSelectWorkspace,
  onApproval,
  settingsConfig,
  onSetModel,
  onCancelMessage,
  onClearMessages,
  onSetGeminiLiveVoice,
  onToggleMobileSidebar,
}: ChatPanelProps) {
  const agentActivities = activitiesFrom(agentProgress)
  const activeFallbackNotice = fallbackNotice(streamedResponse)
  const lastThinkingProgress = [...agentProgress].reverse().find(p => p.type === 'Thinking')
  let activeAgentName: string | null = null
  let activeModelName: string | null = null
  if (lastThinkingProgress && lastThinkingProgress.type === 'Thinking') {
    activeAgentName = (lastThinkingProgress.data as any).agent_name || null
    activeModelName = (lastThinkingProgress.data as any).model_name || null
  }
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
  const [speakingText, setSpeakingText] = useState<string | null>(null)
  const [copiedId, setCopiedId] = useState<string | number | null>(null)

  const chatContainerRef = useRef<HTMLDivElement | null>(null)
  const [showScrollToBottom, setShowScrollToBottom] = useState(false)

  const handleChatScroll = useCallback(() => {
    const el = chatContainerRef.current
    if (!el) return
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
    setShowScrollToBottom(distanceFromBottom > 240)
  }, [])

  const scrollToBottom = useCallback(() => {
    chatEnd.current?.scrollIntoView({ behavior: 'smooth' })
  }, [chatEnd])

  const handleCopyMessage = useCallback(async (id: string | number, text: string) => {
    try {
      const cleanText = readableAssistantText(text) || text
      await navigator.clipboard.writeText(cleanText)
      setCopiedId(id)
      setTimeout(() => {
        setCopiedId((current) => (current === id ? null : current))
      }, 2000)
    } catch (err) {
      console.error('Failed to copy message:', err)
    }
  }, [])


  const toolMenuRef = useRef<HTMLDivElement | null>(null)
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const speechRunRef = useRef(0)
  const historyReadyRef = useRef(false)
  const submittedDuringSessionRef = useRef(false)
  const lastAutoSpokenIdRef = useRef<number | string | null>(null)
  const speakingRef = useRef<string | null>(null)

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
      } else if (file.type.startsWith('video/')) {
        const input = document.getElementById('video-file-input') as HTMLInputElement | null
        if (input) {
          const dt = new DataTransfer()
          dt.items.add(file)
          input.files = dt.files
          const event = { target: input } as ChangeEvent<HTMLInputElement>
          onSelectVideo(event)
        }
      } else if (isSupportedDocument(file.name)) {
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


  // The mic button always drives native push-to-talk transcription; Gemini Live is a
  // separate "Live" button + overlay (see below), not a swap-in replacement for the
  // same button.
  const {
    isRecording,
    voiceMode,
    setVoiceMode,
    voiceTranscript,
    setVoiceTranscript,
    voiceAwaitingResponse,
    voiceAwaitingResponseRef,
    voiceModeRef,
    startRecognition,
    stopRecognition,
    scheduleVoiceListen,
    clearRestartTimer
  } = useVoiceInput({
    onSendVoiceMessage,
    language: settingsConfig?.language,
    message,
    sending,
    isSpeaking: Boolean(speakingText),
    onSetMessage,
  })

  const geminiLiveEnabled = settingsConfig?.voiceMode === 'geminiLive'
  const geminiLive = useGeminiLiveVoice({
    workspacePath,
    startSession: startGeminiLiveSession,
    sendAudioChunk: sendGeminiLiveAudioChunk,
    stopSession: stopGeminiLiveSession
  })

  const canSubmit = Boolean(message.trim() || imageAttachments.length > 0 || videoAttachments.length > 0 || documentName)
  const sendingImageMarkers = Array.from({ length: sendingImageCount }, (_, index) => `[Image #${index + 1}]`).join(' ')
  const sendingVideoMarkers = Array.from({ length: sendingVideoCount || 0 }, (_, index) => `[Video #${index + 1}]`).join(' ')
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
        if (provider.startsWith('custom:')) {
          const id = provider.replace(/^custom:/, '')
          const cp = (settingsConfig?.customProviders ?? []).find(p => p.id === id)
          return cp?.models.map(m => m.modelId) ?? []
        }
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
        if (provider.startsWith('custom:')) {
          const id = provider.replace(/^custom:/, '')
          const cp = (settingsConfig?.customProviders ?? []).find(p => p.id === id)
          return (settingsConfig.customModelSelections ?? {})[id] ?? cp?.models[0]?.modelId ?? ''
        }
        return ''
    }
  }
  const activeModel = getActiveModel(activeProvider)
  const workspaceName = workspacePath
    ? workspacePath.split(/[\\/]/).filter(Boolean).pop() || workspacePath
    : 'Select Project'
  const [skillsList, setSkillsList] = useState<LearnedSkill[]>([])
  const [slashMenuOpen, setSlashMenuOpen] = useState(true)
  const [slashSelectedIndex, setSlashSelectedIndex] = useState(0)
  const slashMenuRef = useRef<HTMLDivElement>(null)
  const slashListRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // Up/Down input history: recall previously submitted prompts (parity with the CLI).
  // `historyIndex` counts back from the newest entry — 0 is the most recent, null
  // means "editing the live draft". The draft is stashed the moment browsing starts.
  const promptHistory = useMemo(() => {
    const out: string[] = []
    for (const it of interactions) {
      if (it?.provider === 'system') continue
      const text = typeof it?.userText === 'string' ? it.userText.trim() : ''
      if (!text) continue
      if (out[out.length - 1] === text) continue
      out.push(text)
    }
    return out
  }, [interactions])
  const [historyIndex, setHistoryIndex] = useState<number | null>(null)
  const historyDraftRef = useRef('')

  const applyHistoryValue = (value: string) => {
    onSetMessage(value)
    // Collapse any open slash/@ menu; the reopen effect brings it back once
    // browsing ends (historyIndex null) if the text still warrants it.
    setSlashMenuOpen(false)
    setTimeout(() => {
      const el = textareaRef.current
      if (!el) return
      resizeInput(el)
      const end = el.value.length
      el.setSelectionRange(end, end)
      el.focus()
    }, 0)
  }
  const navigateHistoryPrev = () => {
    if (promptHistory.length === 0) return false
    if (historyIndex === null) {
      historyDraftRef.current = message
      setHistoryIndex(0)
      applyHistoryValue(promptHistory[promptHistory.length - 1])
      return true
    }
    if (historyIndex + 1 < promptHistory.length) {
      const next = historyIndex + 1
      setHistoryIndex(next)
      applyHistoryValue(promptHistory[promptHistory.length - 1 - next])
    }
    return true
  }
  // "Edit" on a past user message: drop its text back into the composer so it
  // can be tweaked and sent again as a new message (nothing is deleted).
  const handleEditMessage = (text: string) => {
    setHistoryIndex(null)
    applyHistoryValue(text)
  }
  const navigateHistoryNext = () => {
    if (historyIndex === null) return false
    if (historyIndex === 0) {
      setHistoryIndex(null)
      applyHistoryValue(historyDraftRef.current)
      return true
    }
    const next = historyIndex - 1
    setHistoryIndex(next)
    applyHistoryValue(promptHistory[promptHistory.length - 1 - next])
    return true
  }

  const isSlashInput = message.trimStart().startsWith('/')
  const isSkillInput = message.trimStart().startsWith('$')
  const atMatch = message.match(/@([\w\-\.\/]*)$/)
  const isAtInput = Boolean(atMatch)
  const atQuery = atMatch ? atMatch[1].toLowerCase() : ''

  const CONTEXT_SUGGESTIONS = [
    { label: '@workspace', desc: 'Include workspace path & context', type: 'context' as const },
    { label: '@file', desc: 'Reference workspace file', type: 'context' as const },
    { label: '@docs', desc: 'Include documentation context', type: 'context' as const },
    { label: '@memory', desc: 'Include long-term memory store', type: 'context' as const },
  ]

  const mcpSuggestions = Object.entries(settingsConfig?.mcpServers || {})
    .filter(([, srv]: [string, any]) => srv?.disabled !== true)
    .map(([name]) => ({
      label: `@${name}`,
      desc: 'Restrict this message to this MCP server/plugin',
      type: 'plugin' as const,
    }))

  const filteredContexts = isAtInput
    ? [...CONTEXT_SUGGESTIONS, ...mcpSuggestions].filter((item) => item.label.toLowerCase().includes(atQuery))
    : []

  useEffect(() => {
    let isMounted = true
    listLearnedSkills(workspacePath)
      .then((res) => {
        if (isMounted && Array.isArray(res)) setSkillsList(res)
      })
      .catch(() => {})
    return () => { isMounted = false }
  }, [workspacePath, isSkillInput])

  const slashQuery = isSlashInput ? message.trimStart().toLowerCase() : ''
  const filteredSlashCommands = isSlashInput
    ? SLASH_COMMANDS.filter((c) => c.command.toLowerCase().startsWith(slashQuery))
    : []

  const skillQuery = isSkillInput ? message.trimStart().slice(1).toLowerCase() : ''
  const filteredSkills = isSkillInput
    ? skillsList.filter((s) => s.name.toLowerCase().startsWith(skillQuery))
    : []

  const showSuggestionMenu = slashMenuOpen && (
    (isSlashInput && filteredSlashCommands.length > 0) ||
    (isSkillInput && filteredSkills.length > 0) ||
    (isAtInput && filteredContexts.length > 0)
  )

  const suggestionCount = isSlashInput
    ? filteredSlashCommands.length
    : isSkillInput
    ? filteredSkills.length
    : filteredContexts.length

  // Set right before a menu pick rewrites `message`, so the effect below skips
  // one auto-reopen — a completed `/cmd ` stays dismissed and the next Enter
  // runs it (↑/↓ still bring the menu back for subcommands). CLI Tab parity.
  const suppressSlashReopenRef = useRef(false)

  useEffect(() => {
    if (suppressSlashReopenRef.current) {
      suppressSlashReopenRef.current = false
      return
    }
    // Don't pop the suggestion menu while stepping through history — a recalled
    // slash/@ command would otherwise reopen it on every ↑/↓.
    if (historyIndex === null && (isSlashInput || isSkillInput || isAtInput)) {
      setSlashMenuOpen(true)
      setSlashSelectedIndex(0)
    }
  }, [message])

  useEffect(() => {
    if (showSuggestionMenu && slashListRef.current) {
      const activeItem = slashListRef.current.children[slashSelectedIndex] as HTMLElement
      if (activeItem) {
        activeItem.scrollIntoView({ block: 'nearest' })
      }
    }
  }, [slashSelectedIndex, showSuggestionMenu])

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        slashMenuRef.current &&
        !slashMenuRef.current.contains(event.target as Node) &&
        textareaRef.current &&
        !textareaRef.current.contains(event.target as Node)
      ) {
        setSlashMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const selectSlashCommand = (cmd: string) => {
    suppressSlashReopenRef.current = true
    onSetMessage(cmd + ' ')
    setSlashMenuOpen(false)
    setSlashSelectedIndex(0)
    setTimeout(() => textareaRef.current?.focus(), 0)
  }

  const selectSkillCommand = (name: string) => {
    suppressSlashReopenRef.current = true
    onSetMessage('$' + name + ' ')
    setSlashMenuOpen(false)
    setSlashSelectedIndex(0)
    setTimeout(() => textareaRef.current?.focus(), 0)
  }

  const selectAtCommand = (label: string) => {
    suppressSlashReopenRef.current = true
    const nextMsg = message.replace(/@[\w\-\.\/]*$/, label + ' ')
    onSetMessage(nextMsg)
    setSlashMenuOpen(false)
    setSlashSelectedIndex(0)
    setTimeout(() => textareaRef.current?.focus(), 0)
  }

  const submitOnEnter = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return
    event.preventDefault()
    event.currentTarget.form?.requestSubmit()
  }
  const handleInputKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    const isPlainArrowNav =
      (event.key === 'ArrowUp' || event.key === 'ArrowDown') &&
      !event.shiftKey && !event.altKey && !event.metaKey && !event.ctrlKey &&
      !event.nativeEvent.isComposing

    // Once history browsing has started, ↑/↓ keep stepping through history even
    // if a recalled entry is a slash/@ command that reopened the suggestion
    // menu — otherwise the menu would swallow the arrows and you'd be stuck.
    if (
      isPlainArrowNav && historyIndex !== null &&
      event.currentTarget.selectionStart === event.currentTarget.selectionEnd
    ) {
      if (event.key === 'ArrowUp' ? navigateHistoryPrev() : navigateHistoryNext()) {
        event.preventDefault()
        return
      }
    }

    if (showSuggestionMenu) {
      if (event.key === 'ArrowDown') {
        event.preventDefault()
        setSlashSelectedIndex((prev) => (prev + 1) % suggestionCount)
        return
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault()
        setSlashSelectedIndex((prev) => (prev - 1 + suggestionCount) % suggestionCount)
        return
      }
      if (event.key === 'Enter' || event.key === 'Tab') {
        if (!event.shiftKey && !event.nativeEvent.isComposing) {
          event.preventDefault()
          if (isSlashInput) {
            const selected = filteredSlashCommands[slashSelectedIndex] || filteredSlashCommands[0]
            if (selected) selectSlashCommand(selected.command)
          } else if (isSkillInput) {
            const selected = filteredSkills[slashSelectedIndex] || filteredSkills[0]
            if (selected) selectSkillCommand(selected.name)
          } else if (isAtInput) {
            const selected = filteredContexts[slashSelectedIndex] || filteredContexts[0]
            if (selected) selectAtCommand(selected.label)
          }
          return
        }
      }
      if (event.key === 'Escape') {
        event.preventDefault()
        setSlashMenuOpen(false)
        return
      }
    }

    // Menu dismissed (e.g. just after a completion) but the text is still a
    // slash/$/@ prefix with matches — ↑/↓ bring it back rather than starting
    // history browsing, so subcommands stay one keypress away.
    if (
      isPlainArrowNav && !slashMenuOpen && historyIndex === null &&
      (isSlashInput || isSkillInput || isAtInput) && suggestionCount > 0
    ) {
      event.preventDefault()
      setSlashMenuOpen(true)
      setSlashSelectedIndex(0)
      return
    }

    if (isPlainArrowNav) {
      const el = event.currentTarget
      if (el.selectionStart === el.selectionEnd) {
        if (event.key === 'ArrowUp') {
          const onFirstLine = !el.value.slice(0, el.selectionStart).includes('\n')
          if (onFirstLine && navigateHistoryPrev()) {
            event.preventDefault()
            return
          }
        } else {
          const onLastLine = !el.value.slice(el.selectionEnd).includes('\n')
          if (onLastLine && navigateHistoryNext()) {
            event.preventDefault()
            return
          }
        }
      }
    }

    submitOnEnter(event)
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
    const voice = window.speechSynthesis.getVoices().find((item) => {
      const lang = item.lang.toLowerCase()
      const target = hasThai ? 'th' : 'en'
      return lang.startsWith(target) || lang.includes(target)
    })
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

  const toggleRecording = () => {
    if (isRecording) {
      stopRecognition()
      cancelSpeech()
      return
    }

    cancelSpeech()
    startRecognition()
  }
  const resizeInput = (element: HTMLTextAreaElement) => {
    element.style.height = 'auto'
    element.style.height = `${Math.min(element.scrollHeight, 120)}px`
  }
  useEffect(() => {
    return () => {
      cancelSpeech()
    }
  }, [])
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
    if (!latest?.aiText || latest.id === lastAutoSpokenIdRef.current) return

    // Gemini Live speaks its own replies over the realtime audio stream — skip the
    // separate browser/Google TTS pass while a live voice session is active.
    if (!settingsConfig?.enableVoiceReply || geminiLive.voiceMode) {
      lastAutoSpokenIdRef.current = latest.id
      if (voiceMode) {
        scheduleVoiceListen(350)
      }
      return
    }

    lastAutoSpokenIdRef.current = latest.id
    speak(latest.aiText)
  }, [interactions, sending, settingsConfig?.enableVoiceReply, geminiLive.voiceMode, voiceMode])
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
      }
    }
    window.addEventListener('paste', handleWindowPaste, true)
    return () => window.removeEventListener('paste', handleWindowPaste, true)
  }, [onPasteImage])
  useEffect(() => {
    const handleEscapeKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape' && sending) {
        onCancelMessage()
      }
    }
    window.addEventListener('keydown', handleEscapeKeyDown, true)
    return () => window.removeEventListener('keydown', handleEscapeKeyDown, true)
  }, [sending, onCancelMessage])
  useEffect(() => {
    if (message) return
    const input = document.getElementById('chat-input') as HTMLTextAreaElement | null
    if (input) input.style.height = ''
  }, [message])

  const openImagePicker = () => {
    setToolMenuOpen(false)
    document.getElementById('vision-file-input')?.click()
  }
  const openVideoPicker = () => {
    setToolMenuOpen(false)
    document.getElementById('video-file-input')?.click()
  }
  const openDocumentPicker = () => {
    setToolMenuOpen(false)
    document.getElementById('document-file-input')?.click()
  }
  const startWebSearch = () => {
    setToolMenuOpen(false)
    onStartWebSearch()
  }
  const startGenerateImagePrompt = () => {
    setToolMenuOpen(false)
    if (!agentMode) onSetAgentMode(true)
    onSetMessage('Generate an image of ')
    textareaRef.current?.focus()
  }
  const startGenerateVideoPrompt = () => {
    setToolMenuOpen(false)
    if (!agentMode) onSetAgentMode(true)
    onSetMessage('Generate a video of ')
    textareaRef.current?.focus()
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
  const renderCompletedActivity = useCallback((interaction: any) => {
    const interactionId = String(interaction.id)
    const activityView = activitiesFrom(agentActivitySnapshots[interactionId] ?? interaction.agentActivity ?? [])
    const isOpen = Boolean(openActivityIds[interactionId])
    return (
      <AgentActivityDrawer
        activityView={activityView}
        isOpen={isOpen}
        onToggle={() => setOpenActivityIds((current) => ({ ...current, [interactionId]: !current[interactionId] }))}
        isHistorical={true}
      />
    )
  }, [agentActivitySnapshots, openActivityIds])

  const renderWebSearchSources = useCallback((interaction: any) => {
    const interactionId = String(interaction.id)
    const progress = agentActivitySnapshots[interactionId] ?? interaction.agentActivity ?? []
    const sources = parseWebSearchSources(progress)
    if (sources.length === 0) return null

    return <SourcesBlock sources={sources} />
  }, [agentActivitySnapshots])

  const renderFileChanges = useCallback((interaction: any) => {
    const interactionId = String(interaction.id)
    const progress = agentActivitySnapshots[interactionId] ?? interaction.agentActivity ?? []
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
  }, [agentActivitySnapshots, openReviewIds, openFileDiffs])

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
    <section
      className={`conversation-panel ${isEmptyChat ? 'is-empty' : ''}`}
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      style={{ position: 'relative' }}
    >
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
          <div style={{ fontSize: '1.25rem', fontWeight: 'bold', letterSpacing: '0.5px' }}>Drag files to attach data</div>
          <div style={{ fontSize: '0.85rem', color: '#94a3b8', marginTop: '8px' }}>Supports images (PNG, JPEG, WebP, GIF), videos (MP4, WebM, MOV, MKV), and PDF files</div>
        </div>
      )}
      <div className="chat-header">
        {onToggleMobileSidebar && (
          <button
            className="mobile-menu-btn"
            type="button"
            onClick={onToggleMobileSidebar}
            aria-label="Toggle menu"
          >
            ☰
          </button>
        )}
        <div className="chat-header-title">
          <img src={APP_ICON_PATH} alt="Logo" className="chat-header-logo" />
          <span>Mint Agent</span>
        </div>
        <button className="chat-header-clear-btn" title="Clear Messages" onClick={onClearMessages}>
          <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="3 6 5 6 21 6"></polyline>
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            <line x1="10" y1="11" x2="10" y2="17"></line>
            <line x1="14" y1="11" x2="14" y2="17"></line>
          </svg>
        </button>
      </div>
      <div className="chat-container" ref={chatContainerRef} onScroll={handleChatScroll}>
        {interactions.map((interaction, index) => (
          <Fragment key={interaction.id}>
            {index > 0 && shouldShowSessionDivider(interactions[index - 1].createdAt, interaction.createdAt) && (
              <div className="system-event-divider">
                <div className="system-event-line" />
                <div className="system-event-pill">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <circle cx="12" cy="12" r="9" />
                    <polyline points="12 7 12 12 15.5 14" />
                  </svg>
                  <span>{formatSessionDividerLabel(interaction.createdAt)}</span>
                </div>
                <div className="system-event-line" />
              </div>
            )}
            <ChatMessageItem
              interaction={interaction}
              copiedId={copiedId}
              onEditMessage={handleEditMessage}
              speakingText={speakingText}
              agentActivitySnapshots={agentActivitySnapshots}
              thinkingExpanded={thinkingExpanded}
              openActivityIds={openActivityIds}
              openReviewIds={openReviewIds}
              openFileDiffs={openFileDiffs}
              onThinkingExpandedChange={onThinkingExpandedChange}
              handleCopyMessage={handleCopyMessage}
              speak={speak}
              renderCompletedActivity={renderCompletedActivity}
              renderFileChanges={renderFileChanges}
              renderWebSearchSources={renderWebSearchSources}
            />
          </Fragment>
        ))}

        {sending && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
            <div className="message user-message"><div className="bubble-wrapper"><div className="message-bubble">{renderFormattedMessage([sendingMessage, sendingImageMarkers, sendingVideoMarkers].filter(Boolean).join(' '))}</div></div></div>
            {agentMode && (
              <AgentActivityDrawer
                activityView={agentActivities}
                isOpen={openActivityIds['live'] ?? true}
                onToggle={() => setOpenActivityIds((current) => ({ ...current, live: !(current['live'] ?? true) }))}
                pendingApproval={!!pendingApproval}
              />
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
                         <span>
                           {activeAgentName && activeModelName 
                             ? `${activeAgentName} (${activeModelName}) is thinking... (${elapsedSeconds}s)`
                             : `Thinking for ${elapsedSeconds}s (Esc to cancel)`
                           }
                         </span>
                      </div>
                    )}
                  </span>
                </div>
                {streamedResponse && (
                  <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <button className="provider-badge">{badge(streamedResponse.provider, streamedResponse.model)}</button>
                    {activeFallbackNotice && <span className="provider-fallback-notice">{activeFallbackNotice}</span>}
                    {streamedReply && (
                      <div className="message-action-buttons" style={{ display: 'flex', alignItems: 'center', gap: '4px', marginLeft: 'auto' }}>
                        <button
                          type="button"
                          className={`msg-action-btn copy-btn ${copiedId === 'live' ? 'is-copied' : ''}`}
                          onClick={() => handleCopyMessage('live', streamedReply)}
                          title={copiedId === 'live' ? 'คัดลอกแล้ว (Copied!)' : 'คัดลอกข้อความ (Copy message)'}
                        >
                          {renderCopyIcon(copiedId === 'live')}
                        </button>
                        <button
                          type="button"
                          className={`msg-action-btn tts-btn ${speakingText === streamedReply ? 'is-speaking' : ''}`}
                          onClick={() => speak(streamedReply)}
                          title={speakingText === streamedReply ? 'Stop reading' : 'Read aloud'}
                        >
                          {renderSpeakerIcon(speakingText === streamedReply)}
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {pendingApproval && (
          <ApprovalCard
            key={pendingApproval.id || JSON.stringify(pendingApproval)}
            pendingApproval={pendingApproval}
            onApproval={onApproval}
          />
        )}
        {showScrollToBottom && (
          <button
            type="button"
            className="scroll-to-bottom-btn"
            onClick={scrollToBottom}
            title="Scroll to latest message"
            aria-label="Scroll to latest message"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 5v14"></path>
              <path d="M6 13l6 6 6-6"></path>
            </svg>
          </button>
        )}
        <div ref={chatEnd} />
      </div>

      <div className={`input-area ${voiceMode ? 'voice-active' : ''}`}>
        {isEmptyChat && <div className="empty-chat-prompt">Mint Agent is ready to work</div>}
        {onSelectWorkspace && (
          <button type="button" className="workspace-select-btn" onClick={onSelectWorkspace}>
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                <path d="M3 6h7l2 2h9v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"></path>
              </svg>
            </span>
            <span>{workspaceName}</span>
            <span aria-hidden="true">⌄</span>
          </button>
        )}
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
          {onSetPlanMode && (
            <div className="smart-context-label" style={{ display: 'flex', alignItems: 'center', gap: '8px' }} title="Investigate read-only and require plan approval before editing files or running commands">
              <label className="toggle-switch">
                <input type="checkbox" checked={Boolean(planMode)} onChange={(event) => onSetPlanMode(event.target.checked)} />
                <span className="slider round" />
              </label>
              <span>Plan Mode</span>
            </div>
          )}
        </div>
        {voiceMode && (
          <div className="voice-mode-bar" data-state={voiceStatus}>
            <span className="voice-mode-dot" />
            {voiceTranscript ? (
              voiceTranscript === 'Listening to microphone...' ||
              voiceTranscript === 'Listening...' ||
              voiceTranscript === 'No speech detected' ||
              voiceTranscript === 'Sent audio to AI' ? (
                <span>{voiceTranscript}</span>
              ) : (
                <span style={{ display: 'flex', alignItems: 'center', gap: '4px', minWidth: 0, overflow: 'hidden' }}>
                  <span>{voiceStatusLabel}:</span>
                  <span className="voice-mode-transcript">"{voiceTranscript}"</span>
                </span>
              )
            ) : (
              <span>{voiceStatusLabel}</span>
            )}
          </div>
        )}

        {showSuggestionMenu && (
          <div className="slash-suggestions-popup" ref={slashMenuRef}>
            <div className="slash-suggestions-header">
              {isSlashInput ? 'Slash Commands' : isSkillInput ? 'Learned Skills' : 'Context Mentions'} ({slashSelectedIndex + 1}/{suggestionCount})
            </div>
            <div className="slash-suggestions-list" ref={slashListRef}>
              {isSlashInput
                ? filteredSlashCommands.map((item, idx) => (
                    <button
                      key={item.command}
                      type="button"
                      className={`slash-suggestion-item ${idx === slashSelectedIndex ? 'active' : ''}`}
                      onMouseDown={(e) => {
                        e.preventDefault()
                        selectSlashCommand(item.command)
                      }}
                      onMouseEnter={() => setSlashSelectedIndex(idx)}
                    >
                      <span className="slash-cmd-name">{item.command}</span>
                      <span className="slash-cmd-desc">{item.description}</span>
                    </button>
                  ))
                : isSkillInput
                ? filteredSkills.map((item, idx) => (
                    <button
                      key={item.id || item.name}
                      type="button"
                      className={`slash-suggestion-item ${idx === slashSelectedIndex ? 'active' : ''}`}
                      onMouseDown={(e) => {
                        e.preventDefault()
                        selectSkillCommand(item.name)
                      }}
                      onMouseEnter={() => setSlashSelectedIndex(idx)}
                    >
                      <span className="slash-cmd-name">${item.name}</span>
                      <span className="skill-badge">[Skill]</span>
                      <span className="slash-cmd-desc">{item.description || item.content?.slice(0, 60)}</span>
                    </button>
                  ))
                : filteredContexts.map((item, idx) => (
                    <button
                      key={item.label}
                      type="button"
                      className={`slash-suggestion-item ${idx === slashSelectedIndex ? 'active' : ''}`}
                      onMouseDown={(e) => {
                        e.preventDefault()
                        selectAtCommand(item.label)
                      }}
                      onMouseEnter={() => setSlashSelectedIndex(idx)}
                    >
                      <span className="slash-cmd-name">{item.label}</span>
                      <span className="skill-badge">{item.type === 'plugin' ? '[Plugin]' : '[Context]'}</span>
                      <span className="slash-cmd-desc">{item.desc}</span>
                    </button>
                  ))}
            </div>
          </div>
        )}

        <form
          id="chat-form"
          className={geminiLiveEnabled ? 'has-live-btn' : ''}
          style={{ position: 'relative' }}
          onSubmit={(event) => {
            setHistoryIndex(null)
            historyDraftRef.current = ''
            onSubmit(event)
          }}
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
          {(imageAttachments.length > 0 || videoAttachments.length > 0 || documentName) && (
            <div className="mint-attachment">
              {imageAttachments.map((attachment, idx) => (
                <div className="mint-image-attachment" key={idx}>
                  <img className="mint-image-preview" src={attachment.objectUrl || attachment.previewDataUri || attachment.dataUri} alt={attachment.name || 'Image attachment'} />
                  <button className="mint-attachment-remove" type="button" onClick={() => onRemoveImage(idx)} aria-label="Remove image">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </button>
                </div>
              ))}
              {videoAttachments.map((attachment, idx) => (
                <div className="mint-image-attachment" key={idx}>
                  <video className="mint-image-preview" src={attachment.dataUri} muted playsInline preload="metadata" />
                  <div className="mint-video-play-indicator" style={{ position: 'absolute', top: '50%', left: '50%', transform: 'translate(-50%, -50%)', pointerEvents: 'none', background: 'rgba(0,0,0,0.6)', borderRadius: '50%', display: 'flex', alignItems: 'center', justifyContent: 'center', width: '22px', height: '22px', border: '1.5px solid white' }}>
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="white" stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <polygon points="5 3 19 12 5 21 5 3"></polygon>
                    </svg>
                  </div>
                  <button className="mint-attachment-remove" type="button" onClick={() => onRemoveVideo(idx)} aria-label="Remove video">
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
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </button>
                </div>
              )}
            </div>
          )}
          {historyIndex !== null && promptHistory.length > 0 && (
            <div
              className="chat-history-indicator"
              aria-live="polite"
              style={{
                position: 'absolute',
                top: '-9px',
                right: '14px',
                padding: '1px 8px',
                fontSize: '0.68rem',
                fontWeight: 600,
                letterSpacing: '0.02em',
                color: 'var(--text-soft, #94a3b8)',
                background: 'var(--input-bg, #1e293b)',
                border: '1px solid var(--border, rgba(148,163,184,0.25))',
                borderRadius: '999px',
                pointerEvents: 'none',
                zIndex: 2,
              }}
            >
              History {historyIndex + 1}/{promptHistory.length}
            </div>
          )}
          <textarea
            id="chat-input"
            ref={textareaRef}
            value={message}
            onChange={(event) => {
              resizeInput(event.currentTarget)
              onSetMessage(event.target.value)
              // A real keystroke always re-enables the suggestion menu and drops
              // out of history browsing (the recalled text is kept).
              suppressSlashReopenRef.current = false
              if (historyIndex !== null) setHistoryIndex(null)
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
                  <button type="button" role="menuitem" onClick={openImagePicker}>
                    <span aria-hidden="true">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                        <circle cx="8.5" cy="8.5" r="1.5"></circle>
                        <polyline points="21 15 16 10 5 21"></polyline>
                      </svg>
                    </span>
                    <span>Add image</span>
                  </button>
                  <button type="button" role="menuitem" onClick={openVideoPicker}>
                    <span aria-hidden="true">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <polygon points="23 7 16 12 23 17 23 7"></polygon>
                        <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
                      </svg>
                    </span>
                    <span>Add video</span>
                  </button>
                  <button type="button" role="menuitem" onClick={openDocumentPicker}>
                    <span aria-hidden="true">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                        <polyline points="14 2 14 8 20 8"></polyline>
                      </svg>
                    </span>
                    <span>Add file</span>
                  </button>
                  <button type="button" role="menuitem" onClick={startWebSearch}>
                    <span aria-hidden="true">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <circle cx="11" cy="11" r="8"></circle>
                        <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                      </svg>
                    </span>
                    <span>Search web</span>
                  </button>
                  <button type="button" role="menuitem" onClick={startGenerateImagePrompt}>
                    <span aria-hidden="true">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"></path>
                      </svg>
                    </span>
                    <span>Generate image</span>
                  </button>
                  <button type="button" role="menuitem" onClick={startGenerateVideoPrompt}>
                    <span aria-hidden="true">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <polygon points="23 7 16 12 23 17 23 7"></polygon>
                        <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
                      </svg>
                    </span>
                    <span>Generate video</span>
                  </button>
                </div>
              )}
            </div>
          <input id="vision-file-input" type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={onSelectImage} style={{ display: 'none' }} />
          <input id="video-file-input" type="file" accept="video/mp4,video/webm,video/quicktime,video/x-matroska" onChange={onSelectVideo} style={{ display: 'none' }} />
          <input id="document-file-input" type="file" accept={SUPPORTED_DOCUMENT_ACCEPT} onChange={onSelectDocument} style={{ display: 'none' }} />
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
                else if (provider.startsWith('custom:')) {
                  const id = provider.replace(/^custom:/, '')
                  const cp = (settingsConfig?.customProviders ?? []).find(p => p.id === id)
                  displayName = cp?.displayName || id
                }
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
          {geminiLiveEnabled && (
            <button
              id="gemini-live-btn"
              type="button"
              onClick={() => geminiLive.setVoiceMode(true)}
              title="Start Gemini Live conversation"
              style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="2"></circle>
                <path d="M8.5 8.5a5 5 0 0 0 0 7"></path>
                <path d="M15.5 8.5a5 5 0 0 1 0 7"></path>
                <path d="M5.5 5.5a9 9 0 0 0 0 13"></path>
                <path d="M18.5 5.5a9 9 0 0 1 0 13"></path>
              </svg>
            </button>
          )}
          {sending ? (
            <button
              id="send-btn"
              className="stop-btn"
              type="button"
              onClick={onCancelMessage}
              title="Stop generating"
              style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <rect x="4" y="4" width="16" height="16" rx="2" ry="2"></rect>
              </svg>
            </button>
          ) : (
            <button id="send-btn" type="submit" disabled={!canSubmit} style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <line x1="22" y1="2" x2="11" y2="13"></line>
                <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
              </svg>
            </button>
          )}
        </form>
      </div>
      <p className="input-disclaimer">
        Mint Agent is an AI gateway. Responses via third-party APIs. Verify critical info.
      </p>
      {geminiLive.voiceMode && (
        <GeminiLiveOverlay
          status={
            geminiLive.isPaused
              ? 'paused'
              : geminiLive.isSpeaking
              ? 'speaking'
              : geminiLive.voiceAwaitingResponse
              ? 'thinking'
              : 'listening'
          }
          userTranscript={geminiLive.userTranscript}
          assistantTranscript={geminiLive.assistantTranscript}
          isPaused={geminiLive.isPaused}
          onTogglePause={geminiLive.togglePause}
          onEndCall={() => geminiLive.setVoiceMode(false)}
          voice={settingsConfig?.geminiLiveVoice || 'Puck'}
          voices={GEMINI_LIVE_VOICES}
          onChangeVoice={async (voiceName) => {
            await onSetGeminiLiveVoice(voiceName)
            geminiLive.restart()
          }}
        />
      )}
    </section>
  )
}
