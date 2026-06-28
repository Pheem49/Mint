import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from 'react'
import { mergeActivitySnapshots, trimAgentProgress } from '../agentProgress'
import {
  clearChatHistory,
  deleteChatSession,
  renameChatSession,
  getRecentInteractions,
  saveSystemInteraction,
  getRuntimeStatus,
  listChatSessions,
  listSavedPictures,
  selectWorkspaceDirectory,
  saveInteractionAgentActivity,
  streamChatMessage,
  submitToolApproval,
  listen,
  readClipboardImage as readTauriClipboardImage,
  type AgentProgress,
  type ChatResponse,
  type ChatSession,
  type DocumentAttachment,
  type PictureEntry,
  type RuntimeStatus,
} from '../tauri'
import ChatPanel from './ChatPanel'
import DashboardSidebar, { type DashboardView } from './DashboardSidebar'
import ImageStudioPanel from './ImageStudioPanel'
import ModelPanel from './ModelPanel'
import type { ModelInteraction } from './ModelPanel'
import PicturesLibrary from './PicturesLibrary'
import WorkspacePanel from './WorkspacePanel'
import WorkflowBuilderPanel from './WorkflowBuilderPanel'

const EXPRESSIONS = [
  "Default",
  "Dumb Cat",
  "Dumb Cat Eye Roll",
  "Take Photo",
  "Poke",
  "Cat Filter",
]

const ACCESSORIES = [
  "None",
  "Apron",
  "Glasses",
  "Hold Pen",
]

const DEFAULT_CONFIG = {
  theme: 'dark',
  accentColor: '#10b981',
  systemTextColor: '#f8fafc',
  customBgStart: '#0f172a',
  customBgEnd: '#1e1b4b',
  customPanelBg: '#1e293b',
  glassBlur: 'blur(16px)',
  fontFamily: "'Outfit', sans-serif",
  fontSize: '18px',
}

const LAST_WORKSPACE_PATH_KEY = 'mint:last-workspace-path'
const ACTIVE_CONVERSATION_ID_KEY = 'mint:active-conversation-id'

function createConversationId() {
  const random = Math.random().toString(36).slice(2, 10)
  return `conversation-${Date.now().toString(36)}-${random}`
}

function activeConversationId() {
  const existing = window.localStorage.getItem(ACTIVE_CONVERSATION_ID_KEY)
  if (existing === 'conversation-default') {
    window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, 'cli')
    return 'cli'
  }
  if (existing) return existing
  const next = createConversationId()
  window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, next)
  return next
}

const MOCK_WELCOME_INTERACTION = {
  id: -1,
  userText: '',
  aiText: `Hi there! I'm Mint, your AI assistant! 🎯✨ I'm here and ready to help you with whatever you need. I've been organizing some background data to make things smoother for you. 💖\n\nBut the moment you start chatting with me, I'll put everything aside and focus on you! Is there something I can help you with today, or would you like to chat about something special? Let's do this! 🚀💪`,
  provider: 'gemini',
  model: 'gemini-3-flash-preview',
  createdAt: new Date().toISOString(),
}

function errorMessage(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason)
}

function readImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error ?? new Error('Unable to read image'))
    reader.readAsDataURL(file)
  })
}

function readDocument(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error ?? new Error('Unable to read document'))
    reader.readAsDataURL(file)
  })
}

async function createTrimmedImagePreview(dataUri: string): Promise<string> {
  const image = await new Promise<HTMLImageElement>((resolve, reject) => {
    const nextImage = new Image()
    nextImage.onload = () => resolve(nextImage)
    nextImage.onerror = () => reject(new Error('Unable to prepare image preview'))
    nextImage.src = dataUri
  })

  const canvas = document.createElement('canvas')
  canvas.width = image.naturalWidth || image.width
  canvas.height = image.naturalHeight || image.height
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context || canvas.width === 0 || canvas.height === 0) return dataUri

  context.drawImage(image, 0, 0)
  const pixels = context.getImageData(0, 0, canvas.width, canvas.height)
  let minX = canvas.width
  let minY = canvas.height
  let maxX = -1
  let maxY = -1

  for (let y = 0; y < canvas.height; y += 1) {
    for (let x = 0; x < canvas.width; x += 1) {
      const alpha = pixels.data[(y * canvas.width + x) * 4 + 3]
      if (alpha > 12) {
        minX = Math.min(minX, x)
        minY = Math.min(minY, y)
        maxX = Math.max(maxX, x)
        maxY = Math.max(maxY, y)
      }
    }
  }

  if (maxX < minX || maxY < minY) return dataUri

  const padding = 8
  const sx = Math.max(0, minX - padding)
  const sy = Math.max(0, minY - padding)
  const sw = Math.min(canvas.width - sx, maxX - minX + 1 + padding * 2)
  const sh = Math.min(canvas.height - sy, maxY - minY + 1 + padding * 2)

  if (sw >= canvas.width * 0.92 && sh >= canvas.height * 0.92) return dataUri

  const previewCanvas = document.createElement('canvas')
  previewCanvas.width = sw
  previewCanvas.height = sh
  const previewContext = previewCanvas.getContext('2d')
  if (!previewContext) return dataUri
  previewContext.drawImage(canvas, sx, sy, sw, sh, 0, 0, sw, sh)
  return previewCanvas.toDataURL('image/png')
}

const lightenColor = (hex: string, amount: number) => {
  const clean = hex.replace('#', '')
  if (clean.length !== 6) return hex
  const num = parseInt(clean, 16)
  const r = Math.min(255, (num >> 16) + amount)
  const g = Math.min(255, ((num >> 8) & 0x00FF) + amount)
  const b = Math.min(255, (num & 0x0000FF) + amount)
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`
}

const hexToRgb = (hex: string) => {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
  return result ? {
    r: parseInt(result[1], 16),
    g: parseInt(result[2], 16),
    b: parseInt(result[3], 16),
  } : { r: 15, g: 23, b: 42 }
}

const applyThemeStyles = (cfg: any) => {
  const theme = cfg.theme || 'dark'
  const accentColor = cfg.accentColor || '#4f83e6'
  const systemTextColor = cfg.systemTextColor || '#f8fafc'

  document.documentElement.setAttribute('data-theme', theme)
  document.documentElement.style.setProperty('--accent', accentColor)
  document.documentElement.style.setProperty('--accent-hover', lightenColor(accentColor, 20))
  document.documentElement.style.setProperty('--text-main', systemTextColor)
  document.documentElement.style.setProperty('--glass-blur', cfg.glassBlur || 'blur(16px)')
  document.body.style.fontFamily = cfg.fontFamily || "'Outfit', sans-serif"
  document.documentElement.style.fontSize = cfg.fontSize || '18px'

  if (theme === 'custom') {
    if (cfg.customBgStart && cfg.customBgEnd) {
      document.documentElement.style.setProperty('--bg-color', cfg.customBgStart)
      document.documentElement.style.setProperty('--bg-gradient', `linear-gradient(135deg, ${cfg.customBgStart} 0%, ${cfg.customBgEnd} 100%)`)
    }
    if (cfg.customPanelBg) {
      const rgb = hexToRgb(cfg.customPanelBg)
      document.documentElement.style.setProperty('--panel-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.75)`)
      document.documentElement.style.setProperty('--panel-raised', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.82)`)
      document.documentElement.style.setProperty('--panel-soft', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.46)`)
      document.documentElement.style.setProperty('--chrome-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.88)`)
      document.documentElement.style.setProperty('--surface-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.62)`)
      document.documentElement.style.setProperty('--surface-strong', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.86)`)
      document.documentElement.style.setProperty('--input-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.72)`)
    }
    return
  }

  ;[
    '--bg-color',
    '--bg-gradient',
    '--panel-bg',
    '--panel-raised',
    '--panel-soft',
    '--chrome-bg',
    '--surface-bg',
    '--surface-strong',
    '--input-bg',
  ].forEach((name) => document.documentElement.style.removeProperty(name))
}

export default function MintDashboard() {
  const [view, setView] = useState<DashboardView>('chat')
  const [status, setStatus] = useState<RuntimeStatus | null>(null)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const [interactions, setInteractions] = useState<any[]>([])
  const [pictures, setPictures] = useState<PictureEntry[]>([])
  const [sending, setSending] = useState(false)
  const [sendingMessage, setSendingMessage] = useState('')
  const [sendingImageCount, setSendingImageCount] = useState(0)
  const [streamedReply, setStreamedReply] = useState('')
  const [streamedResponse, setStreamedResponse] = useState<ChatResponse | null>(null)
  const [streamingConversationId, setStreamingConversationId] = useState<string | null>(null)
  const [agentProgress, setAgentProgress] = useState<AgentProgress[]>([])
  const [agentActivitySnapshots, setAgentActivitySnapshots] = useState<Record<string, AgentProgress[]>>({})
  const [thinkingExpanded, setThinkingExpanded] = useState<Record<string, boolean>>({})
  const liveThinkingOpenRef = useRef(true)
  const [imageAttachments, setImageAttachments] = useState<Array<{ dataUri: string; name: string; previewDataUri?: string }>>([])
  const [documentAttachment, setDocumentAttachment] = useState<DocumentAttachment | null>(null)
  const [pendingApproval, setPendingApproval] = useState<any | null>(null)
  const [sessionAutoApproved, setSessionAutoApproved] = useState(false)
  const sessionAutoApprovedRef = useRef(false)
  const [modelVisible, setModelVisible] = useState(() => window.localStorage.getItem('mint:model-visible') !== 'false')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => window.localStorage.getItem('mint:sidebar-collapsed') === 'true')
  const [smartContext, setSmartContext] = useState(() => window.localStorage.getItem('mint:smart-context') !== 'false')
  const [agentMode, setAgentMode] = useState(() => window.localStorage.getItem('mint:agent-mode') === 'true')
  const [scale, setScale] = useState(1.00)
  const [interactionEnabled, setInteractionEnabled] = useState(() => window.localStorage.getItem('mint:interaction-enabled') !== 'false')
  const [showInteractionGuide, setShowInteractionGuide] = useState(() => window.localStorage.getItem('mint:interaction-guide-visible') !== 'false')
  const [isLocked, setIsLocked] = useState(false)
  const [layoutPreset, setLayoutPreset] = useState<'chat-wide' | 'model-wide'>(() => (window.localStorage.getItem('mint:layout-preset') as 'chat-wide' | 'model-wide') || 'chat-wide')
  const [toastMessage, setToastMessage] = useState('')
  const [expressionIndex, setExpressionIndex] = useState(0)
  const [accessoryIndex, setAccessoryIndex] = useState(0)
  const [dashboardDataReady, setDashboardDataReady] = useState(false)
  const [modelReady, setModelReady] = useState(false)
  const [startupTimedOut, setStartupTimedOut] = useState(false)
  const [settingsConfig, setSettingsConfig] = useState<any>(null)
  const [workspacePath, setWorkspacePath] = useState(() => window.localStorage.getItem(LAST_WORKSPACE_PATH_KEY) || '')
  const [conversationId, setConversationId] = useState(activeConversationId)
  const [chatSessions, setChatSessions] = useState<ChatSession[]>([])
  const chatEnd = useRef<HTMLDivElement | null>(null)
  const startupReady = (dashboardDataReady && modelReady) || startupTimedOut

  async function refreshHistory() {
    const history = await getRecentInteractions(50, conversationId)
    const reversed = history.reverse()
    setInteractions(reversed)
    setAgentActivitySnapshots((current) => mergeActivitySnapshots(current, reversed))
  }

  async function refreshChatSessions(nextActiveId = conversationId) {
    const sessions = await listChatSessions()
    const isKnown = sessions.some((session) => session.id === nextActiveId)
    setChatSessions(
      isKnown || nextActiveId === 'cli'
        ? sessions
        : [
            {
              id: nextActiveId,
              title: 'New chat',
              kind: 'conversation',
              createdAt: new Date().toISOString(),
              updatedAt: new Date().toISOString(),
            },
            ...sessions,
          ],
    )
  }

  const [picturesRefreshing, setPicturesRefreshing] = useState(false)

  async function refreshPictures() {
    setPicturesRefreshing(true)
    try {
      setPictures(await listSavedPictures())
    } finally {
      setPicturesRefreshing(false)
    }
  }

  useEffect(() => {
    Promise.allSettled([
      getRuntimeStatus().then(setStatus),
      refreshHistory(),
      refreshChatSessions(),
      window.settingsApi?.getSettings()
        .then((loaded: any) => {
          setSettingsConfig(loaded)
          applyThemeStyles({ ...DEFAULT_CONFIG, ...loaded })
        }),
    ]).then((results) => {
      const failure = results.find((result) => result.status === 'rejected')
      if (failure?.status === 'rejected') setError(errorMessage(failure.reason))
      setDashboardDataReady(true)
    })
    const unlistenSpotlight = window.api.onSpotlightToChat((query) => {
      setView('chat')
      setMessage(query)
    })
    const unlistenVision = window.api.onVisionReady((image) => {
      createTrimmedImagePreview(image)
        .catch(() => image)
        .then((previewDataUri) => {
          setImageAttachments((current) => [...current, { dataUri: image, previewDataUri, name: 'Screen capture' }])
        })
    })
    window.api?.onSettingsChanged?.((loaded: any) => {
      setSettingsConfig(loaded)
      applyThemeStyles(loaded)
    })

    const unlistenPromise = listen<any>('tool-approval-requested', (event) => {
      if (sessionAutoApprovedRef.current) {
        submitToolApproval(event.payload.token, true).catch((err) => {
          console.error("Auto approval failed:", err)
        })
      } else {
        setPendingApproval(event.payload)
      }
    })
    return () => {
      unlistenPromise?.then?.((unlisten) => unlisten?.())
      unlistenSpotlight?.then?.((unlisten) => unlisten?.())
      unlistenVision?.then?.((unlisten) => unlisten?.())
    }
  }, [])

  useEffect(() => {
    const timer = window.setTimeout(() => setStartupTimedOut(true), 10000)
    return () => window.clearTimeout(timer)
  }, [])

  useEffect(() => {
    if (view === 'pictures' || view === 'imagine') refreshPictures().catch((reason: unknown) => setError(errorMessage(reason)))
  }, [view])

  useEffect(() => {
    if (view === 'workspeac' && !agentMode) updateAgentMode(true)
  }, [view, agentMode])

  useEffect(() => {
    chatEnd.current?.scrollIntoView({ behavior: 'smooth' })
  }, [interactions, sending, streamedReply, pendingApproval, agentProgress])

  const showToast = (nextMessage: string) => {
    setToastMessage(nextMessage)
    setTimeout(() => setToastMessage((current) => current === nextMessage ? '' : current), 3000)
  }

  const changeLayoutPreset = (preset: 'chat-wide' | 'model-wide') => {
    window.localStorage.setItem('mint:layout-preset', preset)
    setLayoutPreset(preset)
  }

  const toggleModel = () => {
    const next = !modelVisible
    window.localStorage.setItem('mint:model-visible', String(next))
    setModelVisible(next)
  }

  const toggleSidebar = () => {
    const next = !sidebarCollapsed
    window.localStorage.setItem('mint:sidebar-collapsed', String(next))
    setSidebarCollapsed(next)
  }

  const updateInteractionEnabled = (enabled: boolean) => {
    window.localStorage.setItem('mint:interaction-enabled', String(enabled))
    setInteractionEnabled(enabled)
  }

  const updateInteractionGuide = (visible: boolean) => {
    window.localStorage.setItem('mint:interaction-guide-visible', String(visible))
    setShowInteractionGuide(visible)
  }

  const updateSmartContext = (enabled: boolean) => {
    window.localStorage.setItem('mint:smart-context', String(enabled))
    setSmartContext(enabled)
  }

  const updateAgentMode = (enabled: boolean) => {
    window.localStorage.setItem('mint:agent-mode', String(enabled))
    setAgentMode(enabled)
  }

  const updateWorkspacePath = (path: string) => {
    const next = path.trim()
    if (next) {
      window.localStorage.setItem(LAST_WORKSPACE_PATH_KEY, next)
    } else {
      window.localStorage.removeItem(LAST_WORKSPACE_PATH_KEY)
    }
    setWorkspacePath(next)
  }

  useEffect(() => {
    if (!sending) {
      sessionAutoApprovedRef.current = false
      setSessionAutoApproved(false)
    }
  }, [sending])

  async function handleApproval(approved: boolean, autoApproveSession = false) {
    if (!pendingApproval) return
    try {
      if (autoApproveSession) {
        sessionAutoApprovedRef.current = true
        setSessionAutoApproved(true)
      }
      await submitToolApproval(pendingApproval.token, approved)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setPendingApproval(null)
    }
  }

  async function sendPrompt(
    promptText: string,
    options: {
      imageAttachments?: Array<{ dataUri: string; name: string; previewDataUri?: string }>
      audioDataUri?: string | null
      documentAttachment?: DocumentAttachment | null
      systemInstruction?: string
      clearComposer?: boolean
    } = {},
  ) {
    if (sending) return
    const outgoingImages = options.imageAttachments ?? []
    const outgoingDocument = options.documentAttachment ?? null
    const shouldUseAgentMode = agentMode || promptText.toLowerCase().startsWith('search web:')
    const outgoingImage = outgoingImages.map((img) => img.dataUri).join(' ')
    const outgoingImageCount = outgoingImages.length
    setSending(true)
    setStreamingConversationId(conversationId)
    setSendingMessage(promptText)
    setSendingImageCount(outgoingImageCount)
    setError('')
    setStreamedReply('')
    setStreamedResponse(null)
    setAgentProgress([])
    liveThinkingOpenRef.current = true
    setThinkingExpanded((current) => ({ ...current, live: true }))
    const progressSnapshot: AgentProgress[] = []
    if (options.clearComposer) {
      setMessage('')
      setImageAttachments([])
      setDocumentAttachment(null)
    }

    try {
      const response = await streamChatMessage(
        shouldUseAgentMode ? promptText : `/chat ${promptText}`,
        (chunk) => setStreamedReply((current) => `${current}${chunk}`),
        outgoingImage,
        options.audioDataUri ?? null,
        options.systemInstruction ?? '',
        (progress) => {
          progressSnapshot.push(progress)
          setAgentProgress((current) => trimAgentProgress([...current, progress]))
        },
        outgoingDocument,
        workspacePath || null,
        conversationId,
      )
      setStreamedResponse(response)
      const history = (await getRecentInteractions(50, conversationId)).reverse()
      let enrichedHistory = history
      if (progressSnapshot.length > 0) {
        const newestInteraction = [...history]
          .reverse()
          .find((interaction) => interaction.aiText === response.text || interaction.userText === promptText) ?? history[history.length - 1]
        if (newestInteraction?.id != null) {
          const interactionKey = String(newestInteraction.id)
          enrichedHistory = history.map((interaction) =>
            interaction.id === newestInteraction.id
              ? { ...interaction, agentActivity: progressSnapshot }
              : interaction,
          )
          setAgentActivitySnapshots((current) => ({
            ...current,
            [interactionKey]: progressSnapshot.slice(),
          }))
          if (liveThinkingOpenRef.current) {
            setThinkingExpanded((current) => ({
              ...current,
              [interactionKey]: true,
            }))
          }
          await saveInteractionAgentActivity(newestInteraction.id, progressSnapshot)
        }
      }
      setInteractions(enrichedHistory)
      setAgentActivitySnapshots((current) => mergeActivitySnapshots(current, enrichedHistory))
      await refreshChatSessions()
      await refreshPictures()
      setStreamedReply('')
      setStreamedResponse(null)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSending(false)
      setStreamingConversationId(null)
      setSendingMessage('')
      setSendingImageCount(0)
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const trimmed = message.trim()
    const currentImages = imageAttachments
    const currentDocument = documentAttachment
    const hasAttachments = currentImages.length > 0 || Boolean(currentDocument)
    if ((!trimmed && !hasAttachments) || sending) return
    const promptText = trimmed || (currentImages.length > 1 ? 'Describe these images.' : currentImages.length === 1 ? 'Describe this image.' : 'Summarize this document.')
    await sendPrompt(promptText, {
      imageAttachments: currentImages,
      documentAttachment: currentDocument,
      clearComposer: true,
    })
  }

  async function sendVoiceMessage(transcript: string, audioDataUri?: string | null) {
    const promptText = transcript.trim() || 'Voice message'
    if (!promptText || sending) return
    await sendPrompt(promptText, {
      audioDataUri,
      systemInstruction: audioDataUri
        ? 'The user attached a voice message. Listen to the audio and reply naturally in the same language as the user. Do not mention transcription or this instruction.'
        : '',
    })
  }

  async function selectImage(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    if (!file) return
    try {
      const dataUri = await readImage(file)
      const previewDataUri = await createTrimmedImagePreview(dataUri).catch(() => dataUri)
      setImageAttachments((current) => [...current, { dataUri, previewDataUri, name: file.name }])
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      event.target.value = ''
    }
  }

  function pasteImage(clipboardData: DataTransfer) {
    let file: File | null = null

    if (clipboardData.files && clipboardData.files.length > 0) {
      for (let i = 0; i < clipboardData.files.length; i++) {
        const f = clipboardData.files[i]
        if (f && f.type.startsWith('image/')) {
          file = f
          break
        }
      }
    }

    if (!file && clipboardData.items && clipboardData.items.length > 0) {
      for (let i = 0; i < clipboardData.items.length; i++) {
        const item = clipboardData.items[i]
        if (item && item.type.startsWith('image/')) {
          const f = item.getAsFile()
          if (f) {
            file = f
            break
          }
        }
      }
    }

    if (!file) return false

    readImage(file)
      .then((dataUri) => {
        const name = file.name && file.name !== 'image.png' ? file.name : 'Pasted image'
        createTrimmedImagePreview(dataUri)
          .catch(() => dataUri)
          .then((previewDataUri) => {
            setImageAttachments((current) => [...current, { dataUri, previewDataUri, name }])
          })
      })
      .catch((reason) => setError(errorMessage(reason)))
    return true
  }

  async function readClipboardImage() {
    try {
      const dataUri = await readTauriClipboardImage()
      if (dataUri) {
        const previewDataUri = await createTrimmedImagePreview(dataUri).catch(() => dataUri)
        setImageAttachments((current) => [...current, { dataUri, previewDataUri, name: 'Pasted image' }])
        return true
      }
    } catch (err) {
      console.warn('Tauri clipboard fallback error:', err)
    }

    try {
      if (!navigator.clipboard?.read) return false
      const items = await navigator.clipboard.read()
      for (const item of items) {
        const imageType = item.types.find((type) => type.startsWith('image/'))
        if (!imageType) continue
        const blob = await item.getType(imageType)
        const file = new File([blob], 'Pasted image', { type: imageType })
        const dataUri = await readImage(file)
        const previewDataUri = await createTrimmedImagePreview(dataUri).catch(() => dataUri)
        setImageAttachments((current) => [...current, { dataUri, previewDataUri, name: 'Pasted image' }])
        return true
      }
      return false
    } catch {
      // Some environments expose pasted images only through ClipboardEvent.
      return false
    }
  }

  async function selectDocument(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    if (!file) return
    try {
      if (file.type !== 'application/pdf' && !file.name.toLowerCase().endsWith('.pdf')) {
        throw new Error('Only PDF files are supported')
      }
      setDocumentAttachment({
        filename: file.name,
        dataUri: await readDocument(file),
      })
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      event.target.value = ''
    }
  }

  function startWebSearch() {
    updateAgentMode(true)
    setMessage((current) => current.trim() ? `Search web: ${current.trim()}` : 'Search web: ')
  }

  async function selectWorkspace() {
    try {
      const selected = await selectWorkspaceDirectory()
      if (selected) {
        updateWorkspacePath(selected)
        setView('workspeac')
      }
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  async function captureScreen() {
    try {
      await window.api.startVision()
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  async function clearHistory(action: 'New chat' | 'Clear history') {
    try {
      if (action === 'New chat') {
        const next = createConversationId()
        window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, next)
        setConversationId(next)
        await refreshChatSessions(next)
      } else {
        if (!window.confirm(`${action} will clear the current conversation history. Continue?`)) return
        await clearChatHistory(conversationId)
      }
      setInteractions([])
      setAgentActivitySnapshots({})
      setStreamedReply('')
      setStreamedResponse(null)
      setMessage('')
      setImageAttachments([])
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  function handleThinkingExpandedChange(key: string, open: boolean) {
    if (key === 'live') liveThinkingOpenRef.current = open
    setThinkingExpanded((current) => ({ ...current, [key]: open }))
  }

  async function selectConversation(id: string) {
    if (id === conversationId) {
      setView('chat')
      return
    }
    window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, id)
    setConversationId(id)
    setView('chat')
    setStreamedReply('')
    setStreamedResponse(null)
    setMessage('')
    setImageAttachments([])
    setDocumentAttachment(null)
    setAgentProgress([])
    const history = await getRecentInteractions(50, id)
    const reversed = history.reverse()
    setInteractions(reversed)
    setAgentActivitySnapshots((current) => mergeActivitySnapshots(current, reversed))
  }

  async function deleteConversation(id: string) {
    if (id === 'cli') return
    const session = chatSessions.find((item) => item.id === id)
    const title = session?.title || 'this chat'
    if (!window.confirm(`Delete "${title}"? This will remove the conversation and its messages.`)) return

    try {
      await deleteChatSession(id)
      const remaining = chatSessions.filter((item) => item.id !== id && item.kind !== 'cli' && item.id !== 'conversation-default')
      const nextActive = id === conversationId
        ? (remaining[0]?.id ?? createConversationId())
        : conversationId

      if (nextActive !== conversationId) {
        window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, nextActive)
        setConversationId(nextActive)
        setAgentProgress([])
        const history = await getRecentInteractions(50, nextActive)
        const reversed = history.reverse()
        setInteractions(reversed)
        setAgentActivitySnapshots((current) => mergeActivitySnapshots(current, reversed))
      }

      await refreshChatSessions(nextActive)
      if (id !== conversationId) return
      setStreamedReply('')
      setStreamedResponse(null)
      setMessage('')
      setImageAttachments([])
      setDocumentAttachment(null)
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  async function renameConversation(id: string, newTitle: string) {
    if (!newTitle.trim()) return
    try {
      await renameChatSession(id, newTitle.trim())
      await refreshChatSessions(conversationId)
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  function formatProviderChangeText(provider: string, model: string) {
    let providerName = provider
    if (provider === 'gemini') providerName = 'Gemini'
    else if (provider === 'openai') providerName = 'OpenAI'
    else if (provider === 'openrouter') providerName = 'OpenRouter'
    else if (provider === 'deepseek') providerName = 'DeepSeek'
    else if (provider === 'anthropic') providerName = 'Claude'
    else if (provider === 'huggingface') providerName = 'HF'
    else if (provider === 'local_openai') providerName = 'Local'
    else if (provider === 'ollama') providerName = 'Ollama'

    if (providerName && providerName === provider) {
      providerName = providerName.charAt(0).toUpperCase() + providerName.slice(1)
    }
    return [providerName, model].filter(Boolean).join(' • ')
  }

  function getActiveModelName(config: any, provider: string) {
    switch (provider) {
      case 'gemini': return config.geminiModel || 'gemini-1.5-flash'
      case 'openai': return config.openaiModel || 'gpt-4o'
      case 'openrouter': return config.openrouterModel || 'anthropic/claude-3.5-sonnet'
      case 'deepseek': return config.deepseekModel || 'deepseek-chat'
      case 'anthropic': return config.anthropicModel || 'claude-3-5-sonnet-20240620'
      case 'huggingface': return config.hfModel || 'meta-llama/Meta-Llama-3-8B-Instruct'
      case 'local_openai': return config.localModelName || 'llama3'
      case 'ollama': return config.ollamaModel || 'llama3:latest'
      default: return ''
    }
  }

  async function changeProvider(provider: string) {
    try {
      const config = await window.settingsApi.getSettings()
      config.aiProvider = provider
      await window.settingsApi.saveSettings(config)
      setSettingsConfig(config)
      setStatus(await getRuntimeStatus())

      // Record system event in chat history
      const activeModel = getActiveModelName(config, provider)
      const displayName = formatProviderChangeText(provider, activeModel)
      await saveSystemInteraction(conversationId, displayName, 'system', 'provider_change')
      await refreshHistory()
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  async function changeModel(modelName: string) {
    try {
      const config = await window.settingsApi.getSettings()
      const provider = config.aiProvider
      if (provider === 'gemini') {
        config.geminiModel = modelName
      } else if (provider === 'openai') {
        config.openaiModel = modelName
      } else if (provider === 'openrouter') {
        config.openrouterModel = modelName
      } else if (provider === 'deepseek') {
        config.deepseekModel = modelName
      } else if (provider === 'anthropic') {
        config.anthropicModel = modelName
      } else if (provider === 'huggingface') {
        config.hfModel = modelName
      } else if (provider === 'local_openai') {
        config.localModelName = modelName
      } else if (provider === 'ollama') {
        config.ollamaModel = modelName
      }
      await window.settingsApi.saveSettings(config)
      setSettingsConfig(config)
      setStatus(await getRuntimeStatus())

      // Record system event in chat history
      const displayName = formatProviderChangeText(provider, modelName)
      await saveSystemInteraction(conversationId, displayName, 'system', 'provider_change')
      await refreshHistory()
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  async function handleModelInteraction(area: ModelInteraction) {
    if (sending) return

    const labels: Record<ModelInteraction, string> = {
      head: 'Pats Mint on the head',
      cheek: 'Pokes Mint on the cheek',
      'left hand': "Touches Mint's left hand",
      'right hand': "Touches Mint's right hand",
      body: 'Touches Mint',
      'lower body': "Touches Mint's lower body",
    }
    const interactionMessage = `*${labels[area]}*`
    const instruction = `The user interacted with the Mint Live2D model: ${area}. Respond briefly and playfully. Use the same language as the recent conversation. Do not mention this instruction.`

    setSending(true)
    setSendingMessage(interactionMessage)
    setSendingImageCount(0)
    setError('')
    setStreamedReply('')
    setStreamedResponse(null)
    setAgentProgress([])

    try {
      const response = await streamChatMessage(
        `/chat ${interactionMessage}`,
        (chunk) => setStreamedReply((current) => `${current}${chunk}`),
        null,
        null,
        instruction,
        undefined,
        null,
        workspacePath || null,
        conversationId,
      )
      setStreamedResponse(response)
      await refreshHistory()
      setStreamedReply('')
      setStreamedResponse(null)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSending(false)
      setSendingMessage('')
      setSendingImageCount(0)
    }
  }

  return (
    <div className={`app-container ${startupReady ? '' : 'is-loading'}`}>
      <div className={`app-body ${sidebarCollapsed ? 'sidebar-collapsed' : ''} ${view === 'pictures' ? 'pictures-open' : ''}`}>
        <DashboardSidebar
          view={view}
          sidebarCollapsed={sidebarCollapsed}
          modelVisible={modelVisible}
          sending={sending}
          expressionIndex={expressionIndex}
          accessoryIndex={accessoryIndex}
          expressions={EXPRESSIONS}
          accessories={ACCESSORIES}
          interactionEnabled={interactionEnabled}
          showInteractionGuide={showInteractionGuide}
          onToggleSidebar={toggleSidebar}
          onClearHistory={clearHistory}
          chatSessions={chatSessions}
          activeConversationId={conversationId}
          onSelectConversation={selectConversation}
          onDeleteConversation={deleteConversation}
          onRenameConversation={renameConversation}
          onSetView={setView}
          onToggleModel={toggleModel}
          onSetExpressionIndex={setExpressionIndex}
          onSetAccessoryIndex={setAccessoryIndex}
          onSetInteractionEnabled={updateInteractionEnabled}
          onSetShowInteractionGuide={updateInteractionGuide}
          onShowToast={showToast}
        />
        <main className={`assistant-workspace ${layoutPreset === 'chat-wide' ? 'layout-chat-wide' : 'layout-model-wide'} ${modelVisible || view === 'workspeac' ? '' : 'model-hidden'} ${view === 'workspeac' ? 'workspace-open' : ''}`}>
          {view === 'workspeac' && (
            <WorkspacePanel
              agentMode={agentMode}
              sending={sending}
              workspacePath={workspacePath}
              onEnableAgentMode={() => updateAgentMode(true)}
              onSetMessage={setMessage}
              onWorkspaceReady={updateWorkspacePath}
            />
          )}
          <ModelPanel
            scale={scale}
            expressionIndex={expressionIndex}
            accessoryIndex={accessoryIndex}
            isLocked={isLocked}
            isActive={modelVisible && view !== 'pictures' && view !== 'workspeac' && view !== 'workflows'}
            layoutPreset={layoutPreset}
            sending={sending}
            interactionEnabled={interactionEnabled}
            showInteractionGuide={showInteractionGuide}
            toastMessage={toastMessage}
            onSetScale={setScale}
            onSetLocked={setIsLocked}
            onSetView={setView}
            onChangeLayoutPreset={changeLayoutPreset}
            onDismissToast={() => setToastMessage('')}
            onInteract={handleModelInteraction}
            onModelLoadComplete={() => setModelReady(true)}
          />
          <ChatPanel
            interactions={interactions}
            sending={sending && streamingConversationId === conversationId}
            sendingMessage={streamingConversationId === conversationId ? sendingMessage : ''}
            sendingImageCount={streamingConversationId === conversationId ? sendingImageCount : 0}
            streamedReply={streamingConversationId === conversationId ? streamedReply : ''}
            streamedResponse={streamingConversationId === conversationId ? streamedResponse : null}
            agentProgress={streamingConversationId === conversationId ? agentProgress : []}
            agentActivitySnapshots={agentActivitySnapshots}
            thinkingExpanded={thinkingExpanded}
            onThinkingExpandedChange={handleThinkingExpandedChange}
            message={message}
            imageAttachments={imageAttachments}
            documentName={documentAttachment?.filename ?? ''}
            pendingApproval={streamingConversationId === conversationId ? pendingApproval : null}
            smartContext={smartContext}
            agentMode={agentMode}
            status={status}
            workspacePath={workspacePath}
            chatEnd={chatEnd}
            welcomeInteraction={MOCK_WELCOME_INTERACTION}
            onSubmit={handleSubmit}
            onSelectImage={selectImage}
            onSelectDocument={selectDocument}
            onPasteImage={pasteImage}
            onReadClipboardImage={readClipboardImage}
            onSetMessage={setMessage}
            onSendVoiceMessage={sendVoiceMessage}
            onRemoveImage={(idx: number) => {
              setImageAttachments((current) => current.filter((_, i) => i !== idx))
            }}
            onRemoveDocument={() => setDocumentAttachment(null)}
            onStartWebSearch={startWebSearch}
            onCaptureScreen={captureScreen}
            onSetSmartContext={updateSmartContext}
            onSetAgentMode={updateAgentMode}
            onSetProvider={changeProvider}
            onSelectWorkspace={selectWorkspace}
            settingsConfig={settingsConfig}
            onSetModel={changeModel}
            onApproval={handleApproval}
          />
        </main>
        <PicturesLibrary view={view} pictures={pictures} onSetView={setView} onRefreshPictures={refreshPictures} />
        <ImageStudioPanel
          view={view}
          onRefreshPictures={refreshPictures}
          onSendToChat={(_url, imgPrompt) => {
            setView('chat')
            setMessage(imgPrompt)
          }}
        />
        <WorkflowBuilderPanel
          view={view}
          onShowToast={showToast}
        />
      </div>
      <div className={`startup-loading ${startupReady ? 'is-hidden' : ''}`} aria-live="polite" aria-busy={!startupReady}>
        <div className="startup-loading-content">
          <div className="startup-loading-dots" aria-hidden="true"><span /><span /><span /></div>
          <div className="startup-loading-text">Loading Mint Agent</div>
        </div>
      </div>
      {error && (
        <div className="mint-error" style={{ position: 'absolute', bottom: '20px', right: '20px', zIndex: 100, margin: 0, boxShadow: '0 8px 24px rgba(0,0,0,0.3)', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span>{error}</span>
          <button onClick={() => setError('')} style={{ background: 'transparent', border: 0, color: 'white', cursor: 'pointer', display: 'inline-flex', alignItems: 'center', padding: 0 }}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      )}
    </div>
  )
}
