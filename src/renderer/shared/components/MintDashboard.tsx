import { type ChangeEvent, type CSSProperties, type FormEvent, useEffect, useRef, useState } from 'react'
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
  selectWorkspaceDirectory,
  selectLinkedFolderPath,
  saveInteractionAgentActivity,
  streamChatMessage,
  cancelChatMessage,
  submitToolApproval,
  listen,
  readClipboardImage,
  isTauriRuntime,
  type AgentProgress,
  type ChatResponse,
  type ChatSession,
  type DocumentAttachment,
  type PictureEntry,
  type RuntimeStatus,
} from '@/tauri'

import ChatPanel from './ChatPanel'
import DashboardSidebar, { type DashboardView } from './DashboardSidebar'
import ImageStudioPanel from './ImageStudioPanel'
import VeoStudioPanel from './VeoStudioPanel'
import ModelPanel from '@/components/ModelPanel'
import type { ModelInteraction } from '@/components/ModelPanel'
import PicturesLibrary from '@/components/PicturesLibrary'
import WorkspacePanel from '@/components/WorkspacePanel'
import WorkflowBuilderPanel from '@/components/WorkflowBuilderPanel'
import {
  errorMessage,
  readImage,
  readDocument,
  createTrimmedImagePreview,
  createObjectUrlPreview,
  applyThemeStyles,
} from '../utils/ui'
import { executeSlashCommand } from '../utils/slashCommandProcessor'


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

import { DEFAULT_CONFIG } from '../constants/config'

const LAST_WORKSPACE_PATH_KEY = 'mint:last-workspace-path'
const ACTIVE_CONVERSATION_ID_KEY = 'mint:active-conversation-id'

const SIDEBAR_DEFAULT_WIDTH = 264
const SIDEBAR_MIN_WIDTH = 200
const SIDEBAR_MAX_WIDTH = 420
/** Drag the sidebar edge past this width and it snaps fully closed on release. */
const SIDEBAR_COLLAPSE_THRESHOLD = 170

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


import SkillsView from './SkillsView'
import ScheduledTasksView from './ScheduledTasksView'
import LinkedFoldersView from './LinkedFoldersView'
import McpServersView from './McpServersView'
import PluginsView from './PluginsView'
import { isSupportedDocument } from '../utils/documentTypes'
import { useCompanionWidget } from '@/companionWidget'
import {
  listLearnedSkills,
  addLearnedSkill,
  deleteLearnedSkill,
  detectSystemTools,
  reauthMcpServer,
  setProfileValue,
  listCronJobs,
  addCronJob,
  removeCronJob,
  setCronJobEnabled,
  listLinkedFolders,
  addLinkedFolder,
  removeLinkedFolder,
  runSlashCommand,
} from '@/tauri'
import type { SlashResponse } from '../platform'

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

function getCleanPathForView(v: string, activeId?: string): string {
  if (v === 'skills') return '/skills'
  if (v === 'mcp') return '/mcp'
  if (v === 'plugins') return '/plugins'
  if (v === 'pictures') return '/pictures'
  if (v === 'imagine') return '/image-studio'
  if (v === 'veo' || v === 'veo_studio') return '/veo-studio'
  if (v === 'settings') return '/settings'
  if (activeId) return `/chat/${encodeURIComponent(activeId)}`
  return '/chat'
}

/** Keeps a desktop/browser notification body to one readable line. */
function truncateForNotification(text: string, maxChars = 120): string {
  const clean = text.replace(/\s+/g, ' ').trim()
  return clean.length > maxChars ? `${clean.slice(0, maxChars)}…` : clean
}

/** One-line plain-text summary of an AgentApproval for a notification body
 * — ApprovalCard's own renderApprovalDetails returns JSX, not text, so
 * this is a separate, notification-only summary. AgentApproval serializes
 * as a single-key-tagged object per variant, e.g. {"WriteFile": {path, ...}}. */
function describeApproval(payload: any): string {
  const entry = Object.entries(payload?.approval ?? {})[0]
  const kind = entry?.[0]
  const detail: any = entry?.[1]
  switch (kind) {
    case 'WriteFile': return `Write to ${detail?.path}?`
    case 'ApplyPatch': return `Edit ${detail?.path}?`
    case 'RunShell': return `Run: ${detail?.command}`
    case 'NoteWrite': return `Save note to ${detail?.path}?`
    case 'RunPlugin': return `Run plugin: ${detail?.name}?`
    case 'McpTool': return `Use ${detail?.tool} on ${detail?.server}?`
    case 'UserApproval': return detail?.title ?? 'Approval needed'
    case 'AskUser': return detail?.question ?? 'Mint has a question'
    case 'ExitPlanMode':
    case 'EnterPlanMode':
      return 'Plan mode change needs approval'
    default: return 'Mint needs your approval to continue'
  }
}

/** Fires the same notification mechanism as a finished AI reply, but only
 * while the window is unfocused — reused so both surfaces get identical
 * title/permission/dedupe behavior, just a different body string. */
function notifyPendingApproval(payload: any) {
  if (document.hidden || !document.hasFocus()) {
    window.api?.notifyAiResponse?.(truncateForNotification(describeApproval(payload)))
  }
}

export default function MintDashboard() {
  const isDesktopApp = isTauriRuntime()
  const [view, setViewState] = useState<DashboardView>(getInitialViewFromUrl)
  const [conversationId, setConversationId] = useState(activeConversationId)
  // Declared here (not further down with the rest of the workspace-related
  // state) so the URL-change effect below can read it — that effect closes
  // over `workspacePath` to scope its own `getRecentInteractions` call.
  const [workspacePath, setWorkspacePath] = useState(() => window.localStorage.getItem(LAST_WORKSPACE_PATH_KEY) || '')
  // Web only in practice (desktop's window has no mobile-width breakpoint,
  // so nothing ever sets this true there) — declared unconditionally so
  // `changeView` can close it on every navigation without branching.
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false)

  const changeView = (newView: any, targetConversationId?: string) => {
    setMobileSidebarOpen(false)
    setViewState((prev) => {
      const next = typeof newView === 'function' ? newView(prev) : newView
      if (next === 'settings') {
        if ((window as any).api?.openSettings) {
          (window as any).api.openSettings()
        } else {
          if (window.location.pathname !== '/settings') {
            window.history.pushState({}, '', '/settings')
          }
        }
        return prev
      }
      const mappedView: DashboardView = (next === 'veo_studio' ? 'veo' : next) as DashboardView
      const targetPath = getCleanPathForView(mappedView, targetConversationId || conversationId)
      if (typeof window !== 'undefined' && window.location.pathname !== targetPath) {
        window.history.pushState({}, '', targetPath)
      }
      return mappedView
    })
  }

  useEffect(() => {
    const handleUrlChange = () => {
      if (window.location.hash === '#' || window.location.hash === '#/') {
        window.history.replaceState({}, '', window.location.pathname + window.location.search)
      }
      const pathname = (window.location.pathname || '').toLowerCase()
      const hash = (window.location.hash || '').toLowerCase()
      if (pathname.includes('/settings') || hash.includes('/settings')) return
      const nextView = getInitialViewFromUrl()
      setViewState(nextView)

      const urlSessionId = getConversationIdFromUrl()
      if (urlSessionId && urlSessionId !== conversationId) {
        window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, urlSessionId)
        setConversationId(urlSessionId)
        getRecentInteractions(50, urlSessionId, workspacePath || null).then((history) => {
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
  }, [conversationId, workspacePath])
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
  const [sessionAutoApproved, setSessionAutoApproved] = useState(false)
  const sessionAutoApprovedRef = useRef(false)
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => window.localStorage.getItem('mint:sidebar-collapsed') === 'true')
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    const saved = Number(window.localStorage.getItem('mint:sidebar-width'))
    return saved >= SIDEBAR_MIN_WIDTH && saved <= SIDEBAR_MAX_WIDTH ? saved : SIDEBAR_DEFAULT_WIDTH
  })
  const [smartContext, setSmartContext] = useState(() => window.localStorage.getItem('mint:smart-context') !== 'false')
  const [agentMode, setAgentMode] = useState(() => window.localStorage.getItem('mint:agent-mode') === 'true')
  const [planMode, setPlanMode] = useState(() => window.localStorage.getItem('mint:plan-mode') === 'true')
  const [toastMessage, setToastMessage] = useState('')
  const [dashboardDataReady, setDashboardDataReady] = useState(false)
  const [startupTimedOut, setStartupTimedOut] = useState(false)
  const [settingsConfig, setSettingsConfig] = useState<any>(null)
  const [chatSessions, setChatSessions] = useState<ChatSession[]>([])
  const chatEnd = useRef<HTMLDivElement | null>(null)
  const lastNativePasteTimeRef = useRef(0)
  const {
    modelVisible,
    scale,
    interactionEnabled,
    showInteractionGuide,
    isLocked,
    layoutPreset,
    expressionIndex,
    accessoryIndex,
    modelReady,
    proactiveSuggestion,
    setScale,
    setIsLocked,
    setExpressionIndex,
    setAccessoryIndex,
    setModelReady,
    toggleModel,
    changeLayoutPreset,
    updateInteractionEnabled,
    updateInteractionGuide,
    dismissProactiveSuggestion,
    handleProactiveAction,
  } = useCompanionWidget((message) => setError(message))
  const startupReady = (dashboardDataReady && modelReady) || startupTimedOut

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
  // A pending `needs_choice` from the shared slash engine — rendered as a chip bar.
  const [pendingSlashChoice, setPendingSlashChoice] = useState<
    { command: string; title: string; options: { label: string; value: string }[] } | null
  >(null)
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

  // Another process — the CLI's interactive chat, a scheduled task, a
  // messaging bridge — can add messages to this same conversation while
  // this page is open, and nothing here would otherwise notice: Mint's
  // surfaces only share a SQLite file, not a live event bus, so this polls
  // for changes instead of requiring a manual refresh. Kept in a ref rather
  // than the effect's own dependency array so a poll landing doesn't tear
  // down and recreate the interval on every tick.
  const interactionsRef = useRef(interactions)
  useEffect(() => {
    interactionsRef.current = interactions
  }, [interactions])

  useEffect(() => {
    if (view !== 'chat' || !conversationId) return
    const interval = window.setInterval(async () => {
      // Skip while backgrounded (nothing to show anyone) or mid-send (a
      // fetch landing here would clobber the in-flight optimistic/streamed
      // reply with stale history).
      if (document.visibilityState !== 'visible' || sending) return
      try {
        const history = (await getRecentInteractions(50, conversationId, workspacePath || null)).reverse()
        const current = interactionsRef.current
        const currentLast = current[current.length - 1]
        const nextLast = history[history.length - 1]
        if (current.length === history.length && currentLast?.id === nextLast?.id) return
        setInteractions(history)
        setAgentActivitySnapshots((snapshots) => mergeActivitySnapshots(snapshots, history))
      } catch {
        // Best-effort — a transient fetch failure just waits for the next tick.
      }
    }, 3000)
    return () => window.clearInterval(interval)
  }, [view, conversationId, sending, workspacePath])

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
    const history = await getRecentInteractions(50, conversationId, workspacePath || null)
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
      changeView('chat')
      setMessage(query)
    })
    const unlistenVision = window.api.onVisionReady((image) => {
      createTrimmedImagePreview(image)
        .catch(() => image)
        .then((previewDataUri) => {
          setImageAttachments((current) => [...current, { dataUri: image, previewDataUri, name: 'Screen capture' }])
        })
    })
    const handleWindowFocus = () => {
      getRuntimeStatus().then(setStatus).catch(() => {})
      window.settingsApi?.getSettings?.().then((loaded: any) => {
        if (loaded) {
          setSettingsConfig(loaded)
          applyThemeStyles(loaded)
        }
      }).catch(() => {})
      window.api?.clearAiNotifications?.()
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
        notifyPendingApproval(event.payload)
      }
    })
    return () => {
      window.removeEventListener('focus', handleWindowFocus)
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
    if (view === 'workspace' && !agentMode) updateAgentMode(true)
  }, [view, agentMode])

  useEffect(() => {
    chatEnd.current?.scrollIntoView({ behavior: 'smooth' })
  }, [interactions, sending, streamedReply, pendingApproval, agentProgress])

  const showToast = (nextMessage: string) => {
    setToastMessage(nextMessage)
    setTimeout(() => setToastMessage((current) => current === nextMessage ? '' : current), 3000)
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

  const handleSidebarResize = (width: number) => {
    setSidebarWidth(Math.max(SIDEBAR_COLLAPSE_THRESHOLD, Math.min(SIDEBAR_MAX_WIDTH, width)))
  }

  const handleSidebarResizeEnd = (width: number) => {
    if (width < SIDEBAR_COLLAPSE_THRESHOLD) {
      // Dragged small enough — snap fully closed instead of leaving a sliver.
      setSidebarWidth(SIDEBAR_DEFAULT_WIDTH)
      window.localStorage.setItem('mint:sidebar-width', String(SIDEBAR_DEFAULT_WIDTH))
      window.localStorage.setItem('mint:sidebar-collapsed', 'true')
      setSidebarCollapsed(true)
      return
    }
    const clamped = Math.max(SIDEBAR_MIN_WIDTH, Math.min(SIDEBAR_MAX_WIDTH, width))
    setSidebarWidth(clamped)
    window.localStorage.setItem('mint:sidebar-width', String(clamped))
  }

  const updateSmartContext = (enabled: boolean) => {
    window.localStorage.setItem('mint:smart-context', String(enabled))
    setSmartContext(enabled)
  }

  const updateAgentMode = (enabled: boolean) => {
    window.localStorage.setItem('mint:agent-mode', String(enabled))
    setAgentMode(enabled)
  }

  const updatePlanMode = (enabled: boolean) => {
    window.localStorage.setItem('mint:plan-mode', String(enabled))
    setPlanMode(enabled)
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

  async function handleApproval(approved: boolean, autoApproveSession = false, answer?: string) {
    if (!pendingApproval) return
    try {
      if (autoApproveSession) {
        sessionAutoApprovedRef.current = true
        setSessionAutoApproved(true)
      }
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
      setSendingVideoCount(0)
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
      pinnedMcpServer?: string
      forceAgentMode?: boolean
    } = {},
  ) {
    if (sending) return
    const outgoingImages = options.imageAttachments ?? []
    const outgoingVideos = options.videoAttachments ?? []
    const outgoingDocument = options.documentAttachment ?? null
    const shouldUseAgentMode = agentMode || options.forceAgentMode || promptText.toLowerCase().startsWith('search web:')
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
        workspacePath || null,
        conversationId,
        undefined,
        shouldUseAgentMode ? planMode : false,
        options.pinnedMcpServer ?? null,
        (payload) => {
          if (sessionAutoApprovedRef.current) {
            submitToolApproval(payload.token, true).catch((err) => {
              console.error("Auto approval failed:", err)
            })
          } else {
            setPendingApproval(payload)
            notifyPendingApproval(payload)
          }
        },
      )
      setStreamedResponse(response)
      if (document.hidden || !document.hasFocus()) {
        window.api?.notifyAiResponse?.(truncateForNotification(response.text))
      }
      const history = (await getRecentInteractions(50, conversationId, workspacePath || null)).reverse()
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
      // Shared Rust engine first (mint_core::slash). Anything it doesn't own
      // comes back as `not_handled` and falls through to the TS processor below.
      let engineResp: SlashResponse | null = null
      try {
        engineResp = await runSlashCommand(trimmed, workspacePath || null)
      } catch {
        engineResp = null
      }
      if (engineResp && engineResp.kind !== 'not_handled') {
        setMessage('')
        setImageAttachments([])
        setVideoAttachments([])
        setDocumentAttachment(null)
        await handleEngineSlashResponse(trimmed, engineResp)
        return
      }

      const slashResult = executeSlashCommand(trimmed)

      if (slashResult.handled) {
        setMessage('')
        setImageAttachments([])
        setVideoAttachments([])
        setDocumentAttachment(null)

        if (slashResult.action === 'open_image_picker') {
          document.getElementById('vision-file-input')?.click()
          return
        } else if (slashResult.action === 'paste_image') {
          readClipboardImage().then((uri) => {
            if (uri) setImageAttachments((curr) => [...curr, { dataUri: uri, name: 'Clipboard Image' }])
          }).catch(() => {})
          return
        } else if (slashResult.action === 'generate_veo') {
          changeView('veo')
          return
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
    const mcpNames = Object.entries(settingsConfig?.mcpServers || {})
      .filter(([, srv]: [string, any]) => srv?.disabled !== true)
      .map(([name]) => name)
    const pinnedServer = mcpNames.find((name) =>
      new RegExp(`(^|\\s)@${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(\\s|$)`).test(trimmed)
    )
    await sendPrompt(promptText, {
      imageAttachments: currentImages,
      videoAttachments: currentVideos,
      documentAttachment: currentDocument,
      clearComposer: true,
      pinnedMcpServer: pinnedServer,
      forceAgentMode: Boolean(pinnedServer),
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

    if (!file) {
      if (isDesktopApp) {
        // Fallback for Tauri desktop app where WebKitGTK/WebView clipboard paste events do not provide file handles
        const now = Date.now()
        if (now - lastNativePasteTimeRef.current > 100) {
          lastNativePasteTimeRef.current = now
          readClipboardImage().then((dataUri) => {
            if (dataUri) {
              const name = 'Pasted image'
              createTrimmedImagePreview(dataUri)
                .catch(() => dataUri)
                .then((previewDataUri) => {
                  setImageAttachments((current) => [...current, { dataUri, previewDataUri, name }])
                })
            }
          }).catch((err) => {
            console.warn('Failed to read clipboard image via Tauri API:', err)
          })
        }
      }
      // No image in the clipboard (e.g. pasting plain text) — nothing more
      // to do here regardless of platform. Must return before `readImage`
      // below, which requires a real File and would otherwise be called
      // with `file` still null on every non-image paste.
      return false
    }



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

  async function selectWorkspace() {
    try {
      const selected = await selectWorkspaceDirectory()
      if (selected) {
        updateWorkspacePath(selected)
        changeView('workspace')
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

  // Append a synthetic "system" message to the current conversation — used by
  // the slash-command dispatcher for `/help`-style output and `/cron` etc.
  // listings. Mirrors the inline push in `handleSubmit`.
  function pushSystemMessage(userText: string, aiText: string) {
    const systemMsg = {
      id: Date.now(),
      userText,
      aiText,
      createdAt: new Date().toISOString(),
      provider: 'system',
      model: 'mint-cli',
    }
    setInteractions((prev) => [...prev, systemMsg])
    saveSystemInteraction(conversationId, userText, aiText, 'system', 'mint-cli').catch(() => {})
  }

  // Map the shared engine's nav target onto a DashboardView.
  function slashNavView(target: string): DashboardView {
    switch (target) {
      case 'linked_folders':
        return 'link'
      case 'cron':
        return 'cron'
      case 'skills':
        return 'skills'
      case 'plugins':
        return 'plugins'
      case 'mcp':
        return 'mcp'
      case 'veo':
        return 'veo'
      default:
        return 'chat'
    }
  }

  // Act on a response from the shared Rust slash engine (mint_core::slash).
  async function handleEngineSlashResponse(userText: string, resp: SlashResponse) {
    switch (resp.kind) {
      case 'message':
        pushSystemMessage(userText, resp.markdown)
        break
      case 'applied': {
        const wsChange = resp.effects.find((e) => e.kind === 'workspace_changed') as
          | { kind: 'workspace_changed'; path: string }
          | undefined
        if (wsChange) setWorkspacePath(wsChange.path)
        const providerChange = resp.effects.find((e) => e.kind === 'provider_changed') as
          | { kind: 'provider_changed'; display: string }
          | undefined
        if (providerChange) {
          // Render the same "✨ Gemini • model" chip the GUI's own /models shows.
          // The engine already logged the `provider_change` row into the shared
          // "cli" conversation, so this is just the immediate local echo.
          setInteractions((prev) => [
            ...prev,
            {
              id: Date.now(),
              userText: providerChange.display,
              aiText: '',
              createdAt: new Date().toISOString(),
              provider: 'system',
              model: 'provider_change',
            },
          ])
        }
        if (resp.effects.some((e) => e.kind === 'history_cleared')) {
          await clearChatHistory(conversationId).catch(() => {})
          setInteractions([])
        }
        if (resp.markdown) pushSystemMessage(userText, resp.markdown)
        if (resp.effects.some((e) => e.kind !== 'history_cleared' && e.kind !== 'workspace_changed')) {
          // Desktop also gets a `settings-changed` push; refetch anyway so web stays in sync.
          ;(window as any).settingsApi?.getSettings?.().then((c: any) => c && setSettingsConfig(c)).catch(() => {})
          getRuntimeStatus().then(setStatus).catch(() => {})
        }
        break
      }
      case 'navigate':
        if (resp.markdown) pushSystemMessage(userText, resp.markdown)
        changeView(slashNavView(resp.target as any))
        break
      case 'forward_to_agent':
        if (resp.agent_mode) setAgentMode(true)
        await sendPrompt(resp.prompt, { clearComposer: true })
        break
      case 'needs_choice':
        setPendingSlashChoice({ command: resp.command, title: resp.title, options: resp.options })
        break
      case 'exit':
        break
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
    const history = await getRecentInteractions(50, id, workspacePath || null)
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
        const history = await getRecentInteractions(50, nextActive, workspacePath || null)
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
      <div
        className={`app-body ${(sidebarCollapsed && window.innerWidth > 760) ? 'sidebar-collapsed' : ''} ${view === 'pictures' ? 'pictures-open' : ''} ${mobileSidebarOpen ? 'mobile-sidebar-open' : ''}`}
        style={{ '--sidebar-expanded-width': `${sidebarWidth}px` } as CSSProperties}
      >
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
          modelVisible={modelVisible}
          sending={sending}
          expressionIndex={expressionIndex}
          accessoryIndex={accessoryIndex}
          expressions={EXPRESSIONS}
          accessories={ACCESSORIES}
          interactionEnabled={interactionEnabled}
          showInteractionGuide={showInteractionGuide}
          onToggleSidebar={toggleSidebar}
          onSidebarResize={handleSidebarResize}
          onSidebarResizeEnd={handleSidebarResizeEnd}
          onClearHistory={clearHistory}
          chatSessions={chatSessions}
          activeConversationId={conversationId}
          onSelectConversation={selectConversation}
          onDeleteConversation={deleteConversation}
          onRenameConversation={renameConversation}
          onSetView={changeView}
          onToggleModel={toggleModel}
          onSetExpressionIndex={setExpressionIndex}
          onSetAccessoryIndex={setAccessoryIndex}
          onSetInteractionEnabled={updateInteractionEnabled}
          onSetShowInteractionGuide={updateInteractionGuide}
          onShowToast={showToast}
          isSearchOpen={isSearchOpen}
          onSetSearchOpen={setIsSearchOpen}
          showWorkspaceTab={isDesktopApp}
          hasWorkflowsTab={isDesktopApp}
          promoteMediaStudios={!isDesktopApp}
        />
        <main className={`assistant-workspace ${layoutPreset === 'chat-wide' ? 'layout-chat-wide' : 'layout-model-wide'} ${modelVisible || view === 'workspace' ? '' : 'model-hidden'} ${view === 'workspace' ? 'workspace-open' : ''}`} style={(view === 'skills' || view === 'mcp' || view === 'plugins' || view === 'cron' || view === 'link' || view === 'pictures' || view === 'imagine' || view === 'veo') ? { display: 'none' } : undefined}>
          {proactiveSuggestion && (
            <div className="proactive-bar" style={{ position: 'absolute', top: 0, left: 0, right: 0, zIndex: 100 }}>
              <div className="proactive-header">
                <span className="proactive-icon">✨</span>
                <div className="proactive-message">{proactiveSuggestion.message}</div>
                <button className="proactive-dismiss-btn" onClick={dismissProactiveSuggestion}>
                  Dismiss
                </button>
              </div>
              <div className="proactive-chips">
                {proactiveSuggestion.suggestions?.map((sug: any, i: number) => (
                  <button
                    key={i}
                    className="suggestion-chip"
                    onClick={() => handleProactiveAction(sug.action)}
                  >
                    {sug.label}
                  </button>
                ))}
              </div>
            </div>
          )}
          {pendingSlashChoice && (
            <div className="proactive-bar" style={{ position: 'absolute', top: 0, left: 0, right: 0, zIndex: 100 }}>
              <div className="proactive-header">
                <span className="proactive-icon">⚡</span>
                <div className="proactive-message">{pendingSlashChoice.title}</div>
                <button className="proactive-dismiss-btn" onClick={() => setPendingSlashChoice(null)}>
                  Dismiss
                </button>
              </div>
              <div className="proactive-chips">
                {pendingSlashChoice.options.map((opt) => (
                  <button
                    key={opt.value}
                    className="suggestion-chip"
                    onClick={() => {
                      const next = `${pendingSlashChoice.command} ${opt.value}`
                      setPendingSlashChoice(null)
                      runSlashCommand(next, workspacePath || null)
                        .then((r) => handleEngineSlashResponse(next, r))
                        .catch(() => {})
                    }}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>
          )}
          {view === 'workspace' && (
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
            isActive={modelVisible && view !== 'pictures' && view !== 'workspace' && view !== 'workflows' && view !== 'imagine' && view !== 'veo' && view !== 'skills' && view !== 'mcp' && view !== 'plugins'}
            layoutPreset={layoutPreset}
            sending={sending}
            interactionEnabled={interactionEnabled}
            showInteractionGuide={showInteractionGuide}
            toastMessage={toastMessage}
            onSetScale={setScale}
            onSetLocked={setIsLocked}
            onSetView={changeView}
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
            planMode={planMode}
            status={status}
            workspacePath={workspacePath}
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
            onSetPlanMode={isDesktopApp ? updatePlanMode : undefined}
            onSetProvider={changeProvider}
            onSelectWorkspace={isDesktopApp ? selectWorkspace : undefined}
            settingsConfig={settingsConfig}
            onSetModel={changeModel}
            onSetGeminiLiveVoice={changeGeminiLiveVoice}
            onApproval={handleApproval}
            onCancelMessage={handleCancelMessage}
            onClearMessages={() => clearHistory('Clear history')}
            onToggleMobileSidebar={() => setMobileSidebarOpen(!mobileSidebarOpen)}
          />
        </main>
        {view === 'skills' && (
          <div style={{ flex: 1, overflowY: 'auto', background: 'transparent' }}>
            <SkillsView
              listSkills={listLearnedSkills}
              addSkill={addLearnedSkill}
              deleteSkill={deleteLearnedSkill}
              workspacePath={workspacePath}
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
              onReauth={reauthMcpServer}
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
              workspacePath={workspacePath}
            />
          </div>
        )}
        {view === 'link' && (
          <div style={{ flex: 1, overflowY: 'auto', background: 'transparent' }}>
            <LinkedFoldersView
              listLinkedFolders={listLinkedFolders}
              addLinkedFolder={addLinkedFolder}
              removeLinkedFolder={removeLinkedFolder}
              // Desktop: native Tauri picker. Web: asks `mint web` (same
              // machine) to open its own dialog via a loopback-gated route.
              selectFolder={selectLinkedFolderPath}
            />
          </div>
        )}
        <PicturesLibrary view={view} pictures={pictures} onSetView={changeView} onRefreshPictures={refreshPictures} />
        <ImageStudioPanel
          view={view}
          onRefreshPictures={refreshPictures}
          onSendToChat={(_url, imgPrompt) => {
            changeView('chat')
            setMessage(imgPrompt)
          }}
          onToggleMobileSidebar={() => setMobileSidebarOpen(!mobileSidebarOpen)}
        />
        <VeoStudioPanel
          view={view}
          onSendToChat={(vidPrompt) => {
            changeView('chat')
            setMessage(vidPrompt)
          }}
          onToggleMobileSidebar={() => setMobileSidebarOpen(!mobileSidebarOpen)}
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
