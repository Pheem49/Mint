import { useEffect, useRef, useState, Fragment, type ChangeEvent, type ClipboardEvent, type FormEvent, type KeyboardEvent, type RefObject, type DragEvent } from 'react'
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
} from '../../shared/constants/models'
import { badge, providerLabel, fallbackNotice } from '../../shared/utils/providers'
import { activitiesFrom, parseWebSearchSources, type AgentActivity, type AgentActivityView } from '../../shared/utils/agentActivity'
import { AgentActivityTable } from '../../shared/components/AgentActivityTable'
import { ChatCodeBlock } from '../../shared/components/ChatCodeBlock'
import { renderApprovalDetails, renderDiff, type ApprovalDetails } from '../../shared/utils/approval'
import { ApprovalCard } from '../../shared/components/ApprovalCard'
import { renderFormattedMessage, readableAssistantText, cleanSpeechText, renderSpeakerIcon } from '../../shared/utils/markdown'
import { ThinkingBlock } from '../../shared/components/ThinkingBlock'
import { AgentActivityDrawer } from '../../shared/components/AgentActivityDrawer'
import type { DiffHunk, FileChange } from '../../shared/types'
import { numericSetting } from '../../shared/utils/ui'
import { useSpeechToText } from '../../shared/utils/speech'



import {
  type AgentProgress,
  type ChatResponse,
  type RuntimeStatus,
  getTtsUrls,
} from '../tauri'


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
  chatEnd: RefObject<HTMLDivElement | null>
  welcomeInteraction: any
  onSubmit: (event: FormEvent<HTMLFormElement>) => void
  onSelectImage: (event: ChangeEvent<HTMLInputElement>) => void
  onSelectDocument: (event: ChangeEvent<HTMLInputElement>) => void
  onPasteImage: (clipboardData: DataTransfer) => boolean
  onSetMessage: (message: string) => void
  onSendVoiceMessage: (message: string, audioDataUri?: string | null) => Promise<void>
  onRemoveImage: (idx: number) => void
  onRemoveDocument: () => void
  onStartWebSearch: () => void
  onCaptureScreen: () => void
  onSetSmartContext: (enabled: boolean) => void
  onSetAgentMode: (enabled: boolean) => void
  onSetProvider: (provider: string) => void
  onApproval: (approved: boolean, autoApproveSession?: boolean, answer?: string) => void
  onToggleMobileSidebar: () => void
  settingsConfig: any
  onSetModel: (model: string) => void
  onCancelMessage: () => void
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
  chatEnd,
  welcomeInteraction,
  onSubmit,
  onSelectImage,
  onSelectDocument,
  onPasteImage,
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
  onCancelMessage,
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
  const toolMenuRef = useRef<HTMLDivElement | null>(null)
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

  const [elapsedSeconds, setElapsedSeconds] = useState(0)

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
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const speechRunRef = useRef(0)
  const historyReadyRef = useRef(false)
  const submittedDuringSessionRef = useRef(false)
  const lastAutoSpokenIdRef = useRef<number | string | null>(null)
  const speakingRef = useRef<string | null>(null)

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
  } = useSpeechToText({
    language: settingsConfig?.language,
    message,
    sending,
    isSpeaking: Boolean(speakingText),
    onSendVoiceMessage,
    onSetMessage: (val) => onSetMessage(val)
  })

  const voiceStatus = speakingText ? 'speaking' : (sending || voiceAwaitingResponse) ? 'thinking' : isRecording ? 'listening' : voiceMode ? 'ready' : 'off'
  const voiceStatusLabel = voiceStatus === 'speaking' ? 'Speaking' : voiceStatus === 'thinking' ? 'Thinking' : voiceStatus === 'listening' ? 'Listening' : 'Ready'


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

  const toggleRecording = () => {
    if (voiceMode) {
      setVoiceMode(false)
      stopRecognition()
      cancelSpeech()
      return
    }

    setVoiceMode(true)
    startRecognition(true)
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
    const isOpen = Boolean(openActivityIds[interactionId])
    return (
      <AgentActivityDrawer
        activityView={activityView}
        isOpen={isOpen}
        onToggle={() => setOpenActivityIds((current) => ({ ...current, [interactionId]: !current[interactionId] }))}
        isHistorical={true}
      />
    )
  }

  const renderWebSearchSources = (interaction: any) => {
    const interactionId = String(interaction.id)
    const progress = agentActivitySnapshots[interactionId] ?? interaction.agentActivity ?? []
    const sources = parseWebSearchSources(progress)
    if (sources.length === 0) return null
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '8px' }}>
        <span style={{ fontSize: '0.72rem', color: '#64748b', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.05em' }}>Sources</span>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
          {sources.map((src, i) => (
            <a
              key={i}
              href={src.url}
              target="_blank"
              rel="noopener noreferrer"
              title={src.snippet || src.title}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                padding: '5px 10px',
                background: 'rgba(255,255,255,0.04)',
                border: '1px solid rgba(255,255,255,0.08)',
                borderRadius: '8px',
                textDecoration: 'none',
                color: '#cbd5e1',
                fontSize: '0.78rem',
                maxWidth: '220px',
                overflow: 'hidden',
                cursor: 'pointer',
                transition: 'background 0.15s, border-color 0.15s',
              }}
              onMouseEnter={e => {
                (e.currentTarget as HTMLAnchorElement).style.background = 'rgba(255,255,255,0.08)'
                ;(e.currentTarget as HTMLAnchorElement).style.borderColor = 'rgba(255,255,255,0.18)'
              }}
              onMouseLeave={e => {
                (e.currentTarget as HTMLAnchorElement).style.background = 'rgba(255,255,255,0.04)'
                ;(e.currentTarget as HTMLAnchorElement).style.borderColor = 'rgba(255,255,255,0.08)'
              }}
            >
              <img
                src={src.faviconUrl}
                alt=""
                width={14}
                height={14}
                style={{ borderRadius: '2px', flexShrink: 0 }}
                onError={e => { (e.currentTarget as HTMLImageElement).style.display = 'none' }}
              />
              <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {src.domain}
              </span>
            </a>
          ))}
        </div>
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
          <span>Mint Agent</span>
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
          <div style={{ fontSize: '1.25rem', fontWeight: 'bold', letterSpacing: '0.5px' }}>Drag files to attach data</div>
          <div style={{ fontSize: '0.85rem', color: '#94a3b8', marginTop: '8px' }}>Supports images (PNG, JPEG, WebP, GIF) and PDF files</div>
        </div>
      )}

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
                {renderWebSearchSources(interaction)}
                <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <button className="provider-badge">{interaction.provider} • {interaction.model}</button>
                  {fallbackNotice(interaction) && <span className="provider-fallback-notice">{fallbackNotice(interaction)}</span>}
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
                    title={speakingText === interaction.aiText ? "Stop reading" : "Read aloud"}
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
                        title={speakingText === streamedReply ? "Stop reading" : "Read aloud"}
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

        {pendingApproval && (
          <ApprovalCard
            key={pendingApproval.id || JSON.stringify(pendingApproval)}
            pendingApproval={pendingApproval}
            onApproval={onApproval}
          />
        )}
        <div ref={chatEnd} />
      </div>

      <div className={`input-area ${voiceMode ? 'voice-active' : ''}`}>
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
          {sending ? (
            <button
              id="send-btn"
              className="stop-btn"
              type="button"
              onClick={onCancelMessage}
              title="Stop generating"
              style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
                <rect x="4" y="4" width="16" height="16" rx="2" ry="2"></rect>
              </svg>
            </button>
          ) : (
            <button id="send-btn" type="submit" disabled={!canSubmit} style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
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
    </section>
  )
}
