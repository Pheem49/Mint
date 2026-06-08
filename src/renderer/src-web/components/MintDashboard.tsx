import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from 'react'
import {
  clearChatHistory,
  deleteChatSession,
  renameChatSession,
  getRecentInteractions,
  getRuntimeStatus,
  listChatSessions,
  listSavedPictures,
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
import PicturesLibrary from './PicturesLibrary'

const DEFAULT_CONFIG = {
  theme: 'dark',
  accentColor: '#4f83e6',
  systemTextColor: '#f8fafc',
  customBgStart: '#0f172a',
  customBgEnd: '#1e1b4b',
  customPanelBg: '#1e293b',
  glassBlur: 'blur(16px)',
  fontFamily: "'Outfit', sans-serif",
  fontSize: '15px',
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
  aiText: `มิ้นท์กำลังรอสแตนด์บายเตรียมพร้อมช่วยคุณทีมอยู่เลยค่ะ! ✨ แล้วก็แอบนั่งจัดระเบียบข้อมูลนิดๆ หน่อยๆ ให้พร้อมใช้ด้วยค่ะ 😊💖\n\nแต่พอคุณทีมทักมา มิ้นท์ก็วางมือจากทุกอย่างมาคุยกับคุณทีมก่อนเลยนะคะค้าา! ช่วงนี้มีอะไรให้มิ้นท์ช่วยดูแล หรืออยากชวนคุยเรื่องไหนเป็นพิเศษไหมคะ มิ้นท์พร้อมมว๊ากกกค่ะ! 🚀🎯`,
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
  document.documentElement.style.fontSize = cfg.fontSize || '15px'

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
  const [agentProgress, setAgentProgress] = useState<AgentProgress[]>([])
  const [imageAttachments, setImageAttachments] = useState<Array<{ dataUri: string; name: string; previewDataUri?: string }>>([])
  const [documentAttachment, setDocumentAttachment] = useState<DocumentAttachment | null>(null)
  const [pendingApproval, setPendingApproval] = useState<any | null>(null)
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => window.localStorage.getItem('mint:sidebar-collapsed') === 'true')
  const [smartContext, setSmartContext] = useState(() => window.localStorage.getItem('mint:smart-context') !== 'false')
  const [agentMode, setAgentMode] = useState(() => window.localStorage.getItem('mint:agent-mode') !== 'false')
  const [toastMessage, setToastMessage] = useState('')
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false)
  const [dashboardDataReady, setDashboardDataReady] = useState(false)
  const [startupTimedOut, setStartupTimedOut] = useState(false)
  const [settingsConfig, setSettingsConfig] = useState<any>(null)
  const [conversationId, setConversationId] = useState(activeConversationId)
  const [chatSessions, setChatSessions] = useState<ChatSession[]>([])
  const chatEnd = useRef<HTMLDivElement | null>(null)
  const startupReady = dashboardDataReady || startupTimedOut

  async function refreshHistory() {
    const history = await getRecentInteractions(50, conversationId)
    setInteractions(history.reverse())
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

  async function handleApproval(approved: boolean) {
    if (!pendingApproval) return
    try {
      await submitToolApproval(pendingApproval.token, approved)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setPendingApproval(null)
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const trimmed = message.trim()
    const hasAttachments = imageAttachments.length > 0 || Boolean(documentAttachment)
    if ((!trimmed && !hasAttachments) || sending) return
    const promptText = trimmed || (imageAttachments.length > 1 ? 'Describe these images.' : imageAttachments.length === 1 ? 'Describe this image.' : 'Summarize this document.')
    const shouldUseAgentMode = agentMode || promptText.toLowerCase().startsWith('search web:')
    const outgoingImage = imageAttachments.map((img) => img.dataUri).join(' ')
    const outgoingImageCount = imageAttachments.length
    const outgoingDocument = documentAttachment
    setSending(true)
    setSendingMessage(promptText)
    setSendingImageCount(outgoingImageCount)
    setError('')
    setStreamedReply('')
    setStreamedResponse(null)
    setAgentProgress([])
    setMessage('')
    setImageAttachments([])
    setDocumentAttachment(null)

    try {
      const response = await streamChatMessage(
        shouldUseAgentMode ? promptText : `/chat ${promptText}`,
        (chunk) => setStreamedReply((current) => `${current}${chunk}`),
        outgoingImage,
        null,
        '',
        (progress) => setAgentProgress((current) => [...current, progress].slice(-24)),
        outgoingDocument,
        null,
        conversationId,
      )
      setStreamedResponse(response)
      await refreshHistory()
      await refreshChatSessions()
      await refreshPictures()
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
      setStreamedReply('')
      setStreamedResponse(null)
      setMessage('')
      setImageAttachments([])
    } catch (reason) {
      setError(errorMessage(reason))
    }
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
    const history = await getRecentInteractions(50, id)
    setInteractions(history.reverse())
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
        const history = await getRecentInteractions(50, nextActive)
        setInteractions(history.reverse())
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

  async function changeProvider(provider: string) {
    try {
      const config = await window.settingsApi.getSettings()
      config.aiProvider = provider
      await window.settingsApi.saveSettings(config)
      setSettingsConfig(config)
      setStatus(await getRuntimeStatus())
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
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }



  return (
    <div className={`app-container ${startupReady ? '' : 'is-loading'}`}>
      <div className={`app-body ${sidebarCollapsed ? 'sidebar-collapsed' : ''} ${view === 'pictures' ? 'pictures-open' : ''} ${mobileSidebarOpen ? 'mobile-sidebar-open' : ''}`}>
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
        />
        <main className="assistant-workspace model-hidden">
          <ChatPanel
            interactions={interactions}
            sending={sending}
            sendingMessage={sendingMessage}
            sendingImageCount={sendingImageCount}
            streamedReply={streamedReply}
            streamedResponse={streamedResponse}
            agentProgress={agentProgress}
            message={message}
            imageAttachments={imageAttachments}
            documentName={documentAttachment?.filename ?? ''}
            pendingApproval={pendingApproval}
            smartContext={smartContext}
            agentMode={agentMode}
            status={status}
            chatEnd={chatEnd}
            welcomeInteraction={MOCK_WELCOME_INTERACTION}
            onSubmit={handleSubmit}
            onSelectImage={selectImage}
            onSelectDocument={selectDocument}
            onPasteImage={pasteImage}
            onReadClipboardImage={readClipboardImage}
            onSetMessage={setMessage}
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
            onToggleMobileSidebar={() => setMobileSidebarOpen(!mobileSidebarOpen)}
          />
        </main>
        <PicturesLibrary view={view} pictures={pictures} onSetView={changeView} />
      </div>
      <div className={`startup-loading ${startupReady ? 'is-hidden' : ''}`} aria-live="polite" aria-busy={!startupReady}>
        <div className="startup-loading-content">
          <div className="startup-loading-dots" aria-hidden="true"><span /><span /><span /></div>
          <div className="startup-loading-text">Loading Agent Mint</div>
        </div>
      </div>
      {error && (
        <div className="mint-error" style={{ position: 'absolute', bottom: '20px', right: '20px', zIndex: 100, margin: 0, boxShadow: '0 8px 24px rgba(0,0,0,0.3)' }}>
          {error}
          <button onClick={() => setError('')} style={{ marginLeft: '12px', background: 'transparent', border: 0, color: 'white', cursor: 'pointer' }}>✕</button>
        </div>
      )}
    </div>
  )
}
