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
import {
  errorMessage,
  readImage,
  readDocument,
  createTrimmedImagePreview,
  applyThemeStyles,
} from '../../shared/utils/ui'

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

const ACTIVE_CONVERSATION_ID_KEY = 'mint:web-active-conversation-id'

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
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => window.localStorage.getItem('mint:sidebar-collapsed') === 'true')
  const [smartContext, setSmartContext] = useState(() => window.localStorage.getItem('mint:smart-context') !== 'false')
  const [agentMode, setAgentMode] = useState(() => window.localStorage.getItem('mint:agent-mode') === 'true')
  const [toastMessage, setToastMessage] = useState('')
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false)
  const [dashboardDataReady, setDashboardDataReady] = useState(false)
  const [startupTimedOut, setStartupTimedOut] = useState(false)
  const [settingsConfig, setSettingsConfig] = useState<any>(null)
  const [conversationId, setConversationId] = useState(activeConversationId)
  const [chatSessions, setChatSessions] = useState<ChatSession[]>([])
  const chatEnd = useRef<HTMLDivElement | null>(null)
  const startupReady = dashboardDataReady || startupTimedOut

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
    window.api?.onSettingsChanged?.((loaded: any) => {
      setSettingsConfig(loaded)
      applyThemeStyles(loaded)
    })
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

  const changeView = (newView: DashboardView) => {
    setView(newView)
    setMobileSidebarOpen(false)
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
      changeView('chat')
      return
    }
    window.localStorage.setItem(ACTIVE_CONVERSATION_ID_KEY, id)
    setConversationId(id)
    changeView('chat')
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
      await saveSystemInteraction(conversationId, displayName, 'system', 'provider_change')
      await refreshHistory()
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
        <main className="assistant-workspace model-hidden">
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
            chatEnd={chatEnd}
            welcomeInteraction={MOCK_WELCOME_INTERACTION}
            onSubmit={handleSubmit}
            onSelectImage={selectImage}
            onSelectDocument={selectDocument}
            onPasteImage={pasteImage}
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
            settingsConfig={settingsConfig}
            onSetModel={changeModel}
            onApproval={handleApproval}
            onCancelMessage={handleCancelMessage}
            onToggleMobileSidebar={() => setMobileSidebarOpen(!mobileSidebarOpen)}
          />
        </main>
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
