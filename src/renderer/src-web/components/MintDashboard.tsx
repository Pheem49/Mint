import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from 'react'
import { mergeActivitySnapshots, trimAgentProgress } from '../agentProgress'
import {
  clearChatHistory,
  deleteChatSession,
  renameChatSession,
  getRecentInteractions,
  saveSystemInteraction,
  getRuntimeStatus,
  setActiveModel,
  listChatSessions,
  listSavedPictures,
  saveInteractionAgentActivity,
  streamChatMessage,
  cancelChatMessage,
  submitToolApproval,
  listen,
  type AgentProgress,
  type ChatResponse,
  type ChatSession,
  type DocumentAttachment,
  type PictureEntry,
  type RuntimeStatus,
} from '../tauri'
import ChatPanel from './ChatPanel'
import DashboardSidebar, { type DashboardView } from './DashboardSidebar'
import PicturesLibrary from './PicturesLibrary'
import ImageStudioPanel from './ImageStudioPanel'
import VeoStudioPanel from './VeoStudioPanel'
import {
  errorMessage,
  readImage,
  readDocument,
  createTrimmedImagePreview,
  createObjectUrlPreview,
  applyThemeStyles,
} from '../../shared/utils/ui'
import { executeSlashCommand } from '../../shared/utils/slashCommandProcessor'


import { DEFAULT_CONFIG } from '../../shared/constants/config'

const ACTIVE_CONVERSATION_ID_KEY = 'mint:web-active-conversation-id'

function createConversationId() {
  const random = Math.random().toString(36).slice(2, 10)
  return `conversation-${Date.now().toString(36)}-${random}`
}

function getConversationIdFromUrl(): string | null {
  if (typeof window === 'undefined') return null
  const pathname = window.location.pathname || ''
  const hash = (window.location.hash || '').replace(/^#/, '')
  const target = pathname || hash

  const match = target.match(/^\/chat\/(.+)$/i) || target.match(/^\/c\/(.+)$/i)
  if (match && match[1]) {
    return decodeURIComponent(match[1])
  }

  const searchParams = new URLSearchParams(window.location.search)
  const queryId = searchParams.get('id')
  if (queryId) return queryId

  return null
}

function activeConversationId() {
  const fromUrl = getConversationIdFromUrl()
  if (fromUrl) {
    window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, fromUrl)
    return fromUrl
  }
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


import SkillsView from '../../shared/components/SkillsView'
import ScheduledTasksView from '../../shared/components/ScheduledTasksView'
import LinkedFoldersView from '../../shared/components/LinkedFoldersView'
import McpServersView from '../../shared/components/McpServersView'
import PluginsView from '../../shared/components/PluginsView'
import { isSupportedDocument } from '../../shared/utils/documentTypes'
import {
  listLearnedSkills,
  addLearnedSkill,
  deleteLearnedSkill,
  detectSystemTools,
  setProfileValue,
  listCronJobs,
  addCronJob,
  removeCronJob,
  setCronJobEnabled,
  listLinkedFolders,
  addLinkedFolder,
  removeLinkedFolder,
} from '../tauri'

function getInitialViewFromUrl(): DashboardView {
  if (typeof window === 'undefined') return 'chat'
  const hash = (window.location.hash || '').toLowerCase().replace(/^#/, '')
  const pathname = (window.location.pathname || '').toLowerCase()
  const target = hash || pathname

  if (target.includes('skills')) return 'skills'
  if (target.includes('mcp')) return 'mcp'
  if (target.includes('plugins')) return 'plugins'
  if (target.includes('picture')) return 'pictures'
  if (target.includes('image-studio') || target.includes('imagine')) return 'imagine'
  if (target.includes('veo-studio') || target.includes('veo')) return 'veo'
  return 'chat'
}

function getCleanPathForView(v: DashboardView, activeId?: string): string {
  if (v === 'skills') return '/skills'
  if (v === 'mcp') return '/mcp'
  if (v === 'plugins') return '/plugins'
  if (v === 'pictures') return '/pictures'
  if (v === 'imagine') return '/image-studio'
  if (v === 'veo') return '/veo-studio'
  if (activeId) return `/chat/${encodeURIComponent(activeId)}`
  return '/chat'
}

export default function MintDashboard() {
  const [view, setView] = useState<DashboardView>(getInitialViewFromUrl)
  const [conversationId, setConversationId] = useState(activeConversationId)

  useEffect(() => {
    const handleUrlChange = () => {
      if (window.location.hash === '#' || window.location.hash === '#/') {
        window.history.replaceState({}, '', window.location.pathname + window.location.search)
      }
      const pathname = (window.location.pathname || '').toLowerCase()
      const hash = (window.location.hash || '').toLowerCase()
      if (pathname.includes('/settings') || hash.includes('/settings')) return
      const nextView = getInitialViewFromUrl()
      setView(nextView)

      const urlSessionId = getConversationIdFromUrl()
      if (urlSessionId && urlSessionId !== conversationId) {
        window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, urlSessionId)
        setConversationId(urlSessionId)
        getRecentInteractions(50, urlSessionId).then((history) => {
          const reversed = history.reverse()
          setInteractions(reversed)
          setAgentActivitySnapshots((current) => mergeActivitySnapshots(current, reversed))
        })
      }
    }
    window.addEventListener('popstate', handleUrlChange)
    window.addEventListener('hashchange', handleUrlChange)
    return () => {
      window.removeEventListener('popstate', handleUrlChange)
      window.removeEventListener('hashchange', handleUrlChange)
    }
  }, [conversationId])
  const [status, setStatus] = useState<RuntimeStatus | null>(null)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const [interactions, setInteractions] = useState<any[]>([])
  const [pictures, setPictures] = useState<PictureEntry[]>([])
  const [sending, setSending] = useState(false)
  const [sendingMessage, setSendingMessage] = useState('')
  const [sendingImageCount, setSendingImageCount] = useState(0)
  const [sendingVideoCount, setSendingVideoCount] = useState(0)
  const [streamedReply, setStreamedReply] = useState('')
  const [streamedResponse, setStreamedResponse] = useState<ChatResponse | null>(null)
  const [streamingConversationId, setStreamingConversationId] = useState<string | null>(null)
  const [agentProgress, setAgentProgress] = useState<AgentProgress[]>([])
  const [agentActivitySnapshots, setAgentActivitySnapshots] = useState<Record<string, AgentProgress[]>>({})
  const [thinkingExpanded, setThinkingExpanded] = useState<Record<string, boolean>>({})
  const liveThinkingOpenRef = useRef(true)
  const [imageAttachments, setImageAttachments] = useState<Array<{ dataUri: string; name: string; previewDataUri?: string }>>([])
  const [videoAttachments, setVideoAttachments] = useState<Array<{ dataUri: string; name: string }>>([])
  const [documentAttachment, setDocumentAttachment] = useState<DocumentAttachment | null>(null)
  const [pendingApproval, setPendingApproval] = useState<any | null>(null)
  const sessionAutoApprovedRef = useRef(false)
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => window.localStorage.getItem('mint:sidebar-collapsed') === 'true')
  const [smartContext, setSmartContext] = useState(() => window.localStorage.getItem('mint:smart-context') !== 'false')
  const [agentMode, setAgentMode] = useState(() => window.localStorage.getItem('mint:agent-mode') === 'true')
  const [toastMessage, setToastMessage] = useState('')
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false)
  const [dashboardDataReady, setDashboardDataReady] = useState(false)
  const [startupTimedOut, setStartupTimedOut] = useState(false)
  const [settingsConfig, setSettingsConfig] = useState<any>(null)
  const [chatSessions, setChatSessions] = useState<ChatSession[]>([])
  const chatEnd = useRef<HTMLDivElement | null>(null)
  const startupReady = dashboardDataReady || startupTimedOut

  const [mcpName, setMcpName] = useState('')
  const [mcpCmd, setMcpCmd] = useState('')
  const [mcpArgs, setMcpArgs] = useState('')
  const [mcpEnv, setMcpEnv] = useState('')
  const [mcpIcon, setMcpIcon] = useState('')

  const handleUpdateSettingsField = async (field: string, value: any) => {
    const currentConfig = settingsConfig || DEFAULT_CONFIG
    const updatedConfig = { ...currentConfig, [field]: value }
    setSettingsConfig(updatedConfig)

    if ((window as any).settingsApi) {
      await (window as any).settingsApi.saveSettings(updatedConfig)
    } else {
      try {
        await setProfileValue('user-settings', JSON.stringify(updatedConfig))
      } catch (e) {
        console.error('Failed to save settings field:', e)
      }
    }
  }

  const handleAddMcpServer = async () => {
    if (!mcpName.trim() || !mcpCmd.trim()) {
      alert('Please provide at least a server name and command.')
      return
    }

    let parsedEnv = {}
    if (mcpEnv.trim()) {
      try {
        parsedEnv = JSON.parse(mcpEnv)
      } catch {
        alert('Invalid JSON in Environment variable field.')
        return
      }
    }

    const argList = mcpArgs.split(/\s+/).filter(Boolean)
    const currentConfig = settingsConfig || DEFAULT_CONFIG
    const updatedMcp = {
      ...currentConfig?.mcpServers,
      [mcpName.trim()]: {
        command: mcpCmd.trim(),
        args: argList,
        env: parsedEnv,
        icon: mcpIcon.trim() || undefined,
      },
    }

    await handleUpdateSettingsField('mcpServers', updatedMcp)
    setMcpName('')
    setMcpCmd('')
    setMcpArgs('')
    setMcpEnv('')
    setMcpIcon('')
  }

  const handleRemoveMcpServer = async (name: string) => {
    const currentConfig = settingsConfig || DEFAULT_CONFIG
    const updated = { ...(currentConfig?.mcpServers || {}) }
    delete updated[name]
    await handleUpdateSettingsField('mcpServers', updated)
  }

  const handleConnectPlugin = (_plugin: string) => {}

  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key === 'k') {
        event.preventDefault()
        setIsSearchOpen((prev) => !prev)
      } else if (event.key === 'Escape') {
        setIsSearchOpen(false)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  useEffect(() => {
    if (!isSearchOpen) {
      setSearchQuery('')
    }
  }, [isSearchOpen])

  const filteredSessions = chatSessions.filter((session) => {
    if (session.kind === 'cli' || session.id === 'conversation-default') return false
    return session.title.toLowerCase().includes(searchQuery.toLowerCase())
  })

  const groupSessionsByDate = (sessions: ChatSession[]) => {
    const groups: { [key: string]: ChatSession[] } = {}
    
    sessions.forEach((session) => {
      const dateStr = session.updatedAt || session.createdAt
      if (!dateStr) {
        const groupName = 'Other'
        if (!groups[groupName]) groups[groupName] = []
        groups[groupName].push(session)
        return
      }

      const date = new Date(dateStr)
      const today = new Date()
      const yesterday = new Date()
      yesterday.setDate(today.getDate() - 1)

      let groupName = ''
      if (date.toDateString() === today.toDateString()) {
        groupName = 'Today'
      } else if (date.toDateString() === yesterday.toDateString()) {
        groupName = 'Yesterday'
      } else {
        const diffTime = Math.abs(today.getTime() - date.getTime())
        const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24))
        if (diffDays <= 7) {
          groupName = 'Previous 7 days'
        } else if (diffDays <= 30) {
          groupName = 'Previous 30 days'
        } else {
          groupName = date.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })
        }
      }

      if (!groups[groupName]) {
        groups[groupName] = []
      }
      groups[groupName].push(session)
    })

    return groups
  }

  const groupedSearchSessions = groupSessionsByDate(filteredSessions)


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

  async function refreshPictures() {
    setPictures(await listSavedPictures())
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
    const handleWindowFocus = () => {
      getRuntimeStatus().then(setStatus).catch(() => {})
      window.settingsApi?.getSettings?.().then((loaded: any) => {
        if (loaded) {
          setSettingsConfig(loaded)
          applyThemeStyles(loaded)
        }
      }).catch(() => {})
    }
    window.addEventListener('focus', handleWindowFocus)

    window.api?.onSettingsChanged?.((loaded: any) => {
      setSettingsConfig(loaded)
      applyThemeStyles(loaded)
      getRuntimeStatus().then(setStatus).catch(() => {})
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
      window.removeEventListener('focus', handleWindowFocus)
      unlistenPromise?.then?.((unlisten) => unlisten?.())
    }
  }, [])

  useEffect(() => {
    const timer = window.setTimeout(() => setStartupTimedOut(true), 10000)
    return () => window.clearTimeout(timer)
  }, [])

  useEffect(() => {
    if (view === 'pictures') refreshPictures().catch((reason: unknown) => setError(errorMessage(reason)))
  }, [view])

  useEffect(() => {
    chatEnd.current?.scrollIntoView({ behavior: 'smooth' })
  }, [interactions, sending, streamedReply, pendingApproval, agentProgress])

  const showToast = (nextMessage: string) => {
    setToastMessage(nextMessage)
    setTimeout(() => setToastMessage((current) => current === nextMessage ? '' : current), 3000)
  }

  const changeView = (newView: DashboardView, targetConversationId?: string) => {
    setView(newView)
    setMobileSidebarOpen(false)
    const targetPath = getCleanPathForView(newView, targetConversationId || conversationId)
    if (window.location.pathname !== targetPath) {
      window.history.pushState({}, '', targetPath)
    }
  }

  const toggleSidebar = () => {
    if (window.innerWidth <= 760) {
      setMobileSidebarOpen(false)
      return
    }
    const next = !sidebarCollapsed
    window.localStorage.setItem('mint:sidebar-collapsed', String(next))
    setSidebarCollapsed(next)
    setMobileSidebarOpen(false)
  }

  const updateSmartContext = (enabled: boolean) => {
    window.localStorage.setItem('mint:smart-context', String(enabled))
    setSmartContext(enabled)
  }

  const updateAgentMode = (enabled: boolean) => {
    window.localStorage.setItem('mint:agent-mode', String(enabled))
    setAgentMode(enabled)
  }

  async function handleApproval(approved: boolean, _autoApproveSession = false, answer?: string) {
    if (!pendingApproval) return
    try {
      await submitToolApproval(pendingApproval.token, approved, answer)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setPendingApproval(null)
    }
  }

  async function handleCancelMessage() {
    if (!sending || !streamingConversationId) return
    try {
      await cancelChatMessage(streamingConversationId)
    } catch (e) {
      console.error("Failed to cancel message stream:", e)
    } finally {
      setSending(false)
      setStreamingConversationId(null)
      setSendingMessage('')
      setSendingImageCount(0)
    }
  }

  async function sendPrompt(
    promptText: string,
    options: {
      imageAttachments?: Array<{ dataUri: string; name: string; previewDataUri?: string }>
      videoAttachments?: Array<{ dataUri: string; name: string }>
      audioDataUri?: string | null
      documentAttachment?: DocumentAttachment | null
      systemInstruction?: string
      clearComposer?: boolean
    } = {},
  ) {
    if (sending) return
    const outgoingImages = options.imageAttachments ?? []
    const outgoingVideos = options.videoAttachments ?? []
    const outgoingDocument = options.documentAttachment ?? null
    const shouldUseAgentMode = agentMode || promptText.toLowerCase().startsWith('search web:')
    const outgoingImage = outgoingImages.map((img) => img.dataUri).join(' ')
    const outgoingVideo = outgoingVideos.map((vid) => vid.dataUri).join(' ')
    const outgoingImageCount = outgoingImages.length
    setSending(true)
    setStreamingConversationId(conversationId)
    setSendingMessage(promptText)
    setSendingImageCount(outgoingImageCount)
    setSendingVideoCount(outgoingVideos.length)
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
      setVideoAttachments([])
      setDocumentAttachment(null)
    }

    try {
      const response = await streamChatMessage(
        shouldUseAgentMode ? promptText : `/chat ${promptText}`,
        (chunk) => setStreamedReply((current) => `${current}${chunk}`),
        outgoingImage,
        options.audioDataUri ?? null,
        outgoingVideo,
        options.systemInstruction ?? '',
        (progress) => {
          progressSnapshot.push(progress)
          setAgentProgress((current) => trimAgentProgress([...current, progress]))
        },
        outgoingDocument,
        null,
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
      getRuntimeStatus().then(setStatus).catch(() => {})
      setStreamedReply('')
      setStreamedResponse(null)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSending(false)
      setStreamingConversationId(null)
      setSendingMessage('')
      setSendingImageCount(0)
      setSendingVideoCount(0)
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const trimmed = message.trim()
    const currentImages = imageAttachments
    const currentVideos = videoAttachments
    const currentDocument = documentAttachment
    const hasAttachments = currentImages.length > 0 || currentVideos.length > 0 || Boolean(currentDocument)
    if ((!trimmed && !hasAttachments) || sending) return

    if (trimmed.startsWith('/')) {
      const activeP = status?.activeProvider || ''
      const activeM = settingsConfig?.model || ''
      const slashResult = executeSlashCommand(trimmed, {
        activeProvider: activeP,
        activeModel: activeM,
        availableProviders: status?.availableProviders || [],
        interactionsCount: interactions.length,
        fastMode: settingsConfig?.fastMode ?? false,
        multiAgent: settingsConfig?.multiAgent ?? false,
      })

      if (slashResult.handled) {
        setMessage('')
        setImageAttachments([])
        setVideoAttachments([])
        setDocumentAttachment(null)

        if (slashResult.action === 'set_agent_mode') {
          setAgentMode(true)
          if (slashResult.payload?.prompt) {
            await sendPrompt(slashResult.payload.prompt, { clearComposer: true })
          }
          return
        }

        if (slashResult.action === 'open_image_picker') {
          document.getElementById('vision-file-input')?.click()
          return
        } else if (slashResult.action === 'open_plugins') {
          setView('plugins')
          return
        } else if (slashResult.action === 'generate_veo') {
          setView('veo')
          return
        } else if (slashResult.action === 'set_provider_model' && slashResult.payload?.target) {
          const target = slashResult.payload.target
          if (status?.availableProviders.includes(target)) {
            setActiveModel(target).then(() => getRuntimeStatus().then(setStatus)).catch(() => {})
          }
        }

        if (slashResult.systemText) {
          const systemMsg = {
            id: Date.now(),
            userText: trimmed,
            aiText: slashResult.systemText,
            createdAt: new Date().toISOString(),
            provider: 'system',
            model: 'mint-cli',
          }
          setInteractions((prev) => [...prev, systemMsg])
          saveSystemInteraction(conversationId, trimmed, slashResult.systemText || '', 'system', 'mint-cli').catch(() => {})
        }
        return
      }
    }

    const promptText = trimmed || (
      currentImages.length > 0 ? (currentImages.length > 1 ? 'Describe these images.' : 'Describe this image.') :
      currentVideos.length > 0 ? (currentVideos.length > 1 ? 'Describe these videos.' : 'Describe this video.') :
      'Summarize this document.'
    )
    await sendPrompt(promptText, {
      imageAttachments: currentImages,
      videoAttachments: currentVideos,
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
      const objectUrl = createObjectUrlPreview(file).objectUrl
      const dataUri = await readImage(file)
      const previewDataUri = await createTrimmedImagePreview(dataUri).catch(() => dataUri)
      setImageAttachments((current) => [...current, { dataUri, previewDataUri, objectUrl, name: file.name }])
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      event.target.value = ''
    }
  }
  const MAX_VIDEO_BYTES = 25 * 1024 * 1024 // 25 MB
  async function selectVideo(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    if (!file) return
    try {
      if (file.size > MAX_VIDEO_BYTES) {
        setError(`Video is too large. Maximum allowed is ${Math.round(MAX_VIDEO_BYTES / (1024 * 1024))} MB.`)
        return
      }
      const dataUri = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(reader.result as string)
        reader.onerror = reject
        reader.readAsDataURL(file)
      })
      setVideoAttachments((current) => [...current, { dataUri, name: file.name }])
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


  async function selectDocument(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    if (!file) return
    try {
      if (!isSupportedDocument(file.name)) {
        throw new Error('Unsupported document type')
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
        selectConversation(next)
        return
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
    window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, id)
    setConversationId(id)
    changeView('chat', id)
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
    else if (provider.startsWith('custom:')) {
      const id = provider.replace(/^custom:/, '')
      const cp = (settingsConfig?.customProviders ?? []).find((p: any) => p.id === id)
      providerName = cp?.displayName || id
    }

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
      default: {
        if (provider.startsWith('custom:')) {
          const id = provider.replace(/^custom:/, '')
          const cp = (config.customProviders ?? []).find((p: any) => p.id === id)
          return (config.customModelSelections ?? {})[id] ?? cp?.models[0]?.modelId ?? ''
        }
        return ''
      }
    }
  }

  async function changeProvider(provider: string) {
    try {
      const config = await window.settingsApi.getSettings()
      if (config.aiProvider === provider) return
      config.aiProvider = provider
      await window.settingsApi.saveSettings(config)
      setSettingsConfig(config)
      setStatus(await getRuntimeStatus())

      // Record system event in chat history
      const activeModel = getActiveModelName(config, provider)
      const displayName = formatProviderChangeText(provider, activeModel)
      await saveSystemInteraction(conversationId, displayName, '', 'system', 'provider_change')
      await refreshHistory()
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  async function changeModel(modelName: string) {
    try {
      const config = await window.settingsApi.getSettings()
      const provider = config.aiProvider
      const currentModel = getActiveModelName(config, provider)
      if (currentModel === modelName) return
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
      } else if (provider.startsWith('custom:')) {
        const id = provider.replace(/^custom:/, '')
        config.customModelSelections = {
          ...(config.customModelSelections ?? {}),
          [id]: modelName
        }
      }
      await window.settingsApi.saveSettings(config)
      setSettingsConfig(config)
      setStatus(await getRuntimeStatus())

      // Record system event in chat history
      const displayName = formatProviderChangeText(provider, modelName)
      await saveSystemInteraction(conversationId, displayName, '', 'system', 'provider_change')
      await refreshHistory()
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  async function changeGeminiLiveVoice(voiceName: string) {
    try {
      const config = await window.settingsApi.getSettings()
      if (config.geminiLiveVoice === voiceName) return
      config.geminiLiveVoice = voiceName
      await window.settingsApi.saveSettings(config)
      setSettingsConfig(config)
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  return (
    <div className={`app-container ${startupReady ? '' : 'is-loading'}`}>
      <div className={`app-body ${(sidebarCollapsed && window.innerWidth > 760) ? 'sidebar-collapsed' : ''} ${view === 'pictures' ? 'pictures-open' : ''} ${mobileSidebarOpen ? 'mobile-sidebar-open' : ''}`}>
        {mobileSidebarOpen && (
          <div
            className="sidebar-backdrop"
            onClick={() => setMobileSidebarOpen(false)}
            style={{
              position: 'fixed',
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              background: 'rgba(0, 0, 0, 0.5)',
              backdropFilter: 'blur(4px)',
              zIndex: 9998,
            }}
          />
        )}
        <DashboardSidebar
          view={view}
          sidebarCollapsed={sidebarCollapsed}
          sending={sending}
          chatSessions={chatSessions}
          activeConversationId={conversationId}
          onToggleSidebar={toggleSidebar}
          onClearHistory={clearHistory}
          onSelectConversation={selectConversation}
          onDeleteConversation={deleteConversation}
          onRenameConversation={renameConversation}
          onSetView={changeView}
          isSearchOpen={isSearchOpen}
          onSetSearchOpen={setIsSearchOpen}
        />
        <main className="assistant-workspace model-hidden" style={view !== 'chat' ? { display: 'none' } : undefined}>
          <ChatPanel
            interactions={interactions}
            sending={sending && streamingConversationId === conversationId}
            sendingMessage={streamingConversationId === conversationId ? sendingMessage : ''}
            sendingImageCount={streamingConversationId === conversationId ? sendingImageCount : 0}
            streamedReply={streamingConversationId === conversationId ? streamedReply : ''}
            streamedResponse={streamingConversationId === conversationId ? streamedResponse : null}
            agentProgress={streamingConversationId === conversationId ? agentProgress : []}
            sendingVideoCount={streamingConversationId === conversationId ? sendingVideoCount : 0}
            agentActivitySnapshots={agentActivitySnapshots}
            thinkingExpanded={thinkingExpanded}
            onThinkingExpandedChange={handleThinkingExpandedChange}
            message={message}
            imageAttachments={imageAttachments}
            videoAttachments={videoAttachments}
            documentName={documentAttachment?.filename ?? ''}
            pendingApproval={streamingConversationId === conversationId ? pendingApproval : null}
            smartContext={smartContext}
            agentMode={agentMode}
            status={status}
            chatEnd={chatEnd}
            welcomeInteraction={MOCK_WELCOME_INTERACTION}
            onSubmit={handleSubmit}
            onSelectImage={selectImage}
            onSelectVideo={selectVideo}
            onSelectDocument={selectDocument}
            onPasteImage={pasteImage}
            onSetMessage={setMessage}
            onSendVoiceMessage={sendVoiceMessage}
            onRemoveImage={(idx: number) => {
              setImageAttachments((current) => current.filter((_, i) => i !== idx))
            }}
            onRemoveVideo={(idx: number) => {
              setVideoAttachments((current) => current.filter((_, i) => i !== idx))
            }}
            onRemoveDocument={() => setDocumentAttachment(null)}
            onStartWebSearch={startWebSearch}
            onCaptureScreen={captureScreen}
            onSetSmartContext={updateSmartContext}
            onSetAgentMode={updateAgentMode}
            onSetProvider={changeProvider}
            settingsConfig={settingsConfig}
            onSetModel={changeModel}
            onSetGeminiLiveVoice={changeGeminiLiveVoice}
            onApproval={handleApproval}
            onCancelMessage={handleCancelMessage}
            onToggleMobileSidebar={() => setMobileSidebarOpen(!mobileSidebarOpen)}
            onClearMessages={() => clearHistory('Clear history')}
          />
        </main>
        {view === 'skills' && (
          <div style={{ flex: 1, overflowY: 'auto', background: 'transparent' }}>
            <SkillsView
              listSkills={listLearnedSkills}
              addSkill={addLearnedSkill}
              deleteSkill={deleteLearnedSkill}
              workspacePath={window.localStorage.getItem('mint:last-workspace-path') || undefined}
            />
          </div>
        )}
        {view === 'mcp' && (
          <div style={{ flex: 1, overflowY: 'auto', background: 'transparent' }}>
            <McpServersView
              config={settingsConfig || DEFAULT_CONFIG}
              updateField={handleUpdateSettingsField}
              mcpName={mcpName}
              setMcpName={setMcpName}
              mcpCmd={mcpCmd}
              setMcpCmd={setMcpCmd}
              mcpArgs={mcpArgs}
              setMcpArgs={setMcpArgs}
              mcpEnv={mcpEnv}
              setMcpEnv={setMcpEnv}
              mcpIcon={mcpIcon}
              setMcpIcon={setMcpIcon}
              handleAddMcpServer={handleAddMcpServer}
              handleRemoveMcpServer={handleRemoveMcpServer}
              detectTools={detectSystemTools}
            />
          </div>
        )}
        {view === 'plugins' && (
          <div style={{ flex: 1, overflowY: 'auto', background: 'transparent' }}>
            <PluginsView
              config={settingsConfig || DEFAULT_CONFIG}
              updateField={handleUpdateSettingsField}
              handleConnectPlugin={handleConnectPlugin}
            />
          </div>
        )}
        {view === 'cron' && (
          <div style={{ flex: 1, overflowY: 'auto', background: 'transparent' }}>
            <ScheduledTasksView
              listCronJobs={listCronJobs}
              addCronJob={addCronJob}
              removeCronJob={removeCronJob}
              setCronJobEnabled={setCronJobEnabled}
              workspacePath={window.localStorage.getItem('mint:last-workspace-path') || undefined}
            />
          </div>
        )}
        {view === 'link' && (
          <div style={{ flex: 1, overflowY: 'auto', background: 'transparent' }}>
            <LinkedFoldersView
              listLinkedFolders={listLinkedFolders}
              addLinkedFolder={addLinkedFolder}
              removeLinkedFolder={removeLinkedFolder}
            />
          </div>
        )}
        <PicturesLibrary view={view} pictures={pictures} onSetView={changeView} onRefreshPictures={refreshPictures} />
        <ImageStudioPanel
          view={view}
          onRefreshPictures={refreshPictures}
          onSendToChat={(_url, imgPrompt) => {
            setView('chat')
            setMessage(imgPrompt)
          }}
          onToggleMobileSidebar={() => setMobileSidebarOpen(!mobileSidebarOpen)}
        />
        <VeoStudioPanel
          view={view}
          onSendToChat={(imgPrompt) => {
            setView('chat')
            setMessage(imgPrompt)
          }}
          onToggleMobileSidebar={() => setMobileSidebarOpen(!mobileSidebarOpen)}
        />
      </div>
      <div className={`startup-loading ${startupReady ? 'is-hidden' : ''}`} aria-live="polite" aria-busy={!startupReady}>
        <div className="startup-loading-content">
          <div className="startup-loading-dots" aria-hidden="true"><span /><span /><span /></div>
          <div className="startup-loading-text">Loading Mint Agent</div>
        </div>
      </div>
      {error && (
        <div className="mint-error" style={{ position: 'absolute', bottom: '20px', right: '20px', zIndex: 100, margin: 0, boxShadow: '0 8px 24px rgba(0,0,0,0.3)' }}>
          {error}
          <button onClick={() => setError('')} style={{ marginLeft: '12px', background: 'transparent', border: 0, color: 'white', cursor: 'pointer' }}>✕</button>
        </div>
      )}

      {isSearchOpen && (
        <div className="sidebar-search-modal-backdrop" onClick={() => setIsSearchOpen(false)}>
          <div className="sidebar-search-modal" onClick={(e) => e.stopPropagation()}>
            <div className="search-modal-header">
              <span className="search-icon-wrapper">
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="11" cy="11" r="8"></circle>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
              </span>
              <input
                type="text"
                placeholder="Search chats..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                autoFocus
              />
              <button className="search-modal-close" onClick={() => setIsSearchOpen(false)}>
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            
            <div className="search-modal-body">
              <button
                className="search-new-chat-btn"
                onClick={() => {
                  clearHistory('New chat')
                  setIsSearchOpen(false)
                }}
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
                <span>New Chat</span>
              </button>

              <div className="search-modal-results">
                {Object.keys(groupedSearchSessions).length > 0 ? (
                  Object.entries(groupedSearchSessions).map(([groupName, sessions]) => (
                    <div key={groupName} className="search-results-group">
                      <div className="search-group-title">{groupName}</div>
                      {sessions.map((session) => (
                        <button
                          key={session.id}
                          className={`search-result-item ${session.id === conversationId ? 'active' : ''}`}
                          onClick={() => {
                            selectConversation(session.id)
                            setIsSearchOpen(false)
                          }}
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
                          </svg>
                          <span className="search-result-title">{session.title || 'New chat'}</span>
                        </button>
                      ))}
                    </div>
                  ))
                ) : (
                  <div className="search-no-results">No matching chats found</div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
