import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from 'react'
import {
  clearChatHistory,
  getRecentInteractions,
  getRuntimeStatus,
  listSavedPictures,
  streamChatMessage,
  submitToolApproval,
  listen,
  type AgentProgress,
  type ChatResponse,
  type PictureEntry,
  type RuntimeStatus,
} from '../tauri'
import ChatPanel, { PicturesLibrary } from './ChatPanel'
import DashboardSidebar, { type DashboardView } from './DashboardSidebar'
import ModelPanel from './ModelPanel'
import type { ModelInteraction } from './ModelPanel'

const EXPRESSIONS = [
  "ปกติ (Default)",
  "呆猫 (Dumb Cat)",
  "呆猫眼珠摇晃 (Dumb Cat Eye Roll)",
  "拍照 (Take Photo)",
  "点一下 (Poke)",
  "猫咪滤镜 (Cat Filter)",
]

const ACCESSORIES = [
  "ปกติ (None)",
  "ผ้ากันเปื้อน (Apron)",
  "แว่นตา (Glasses)",
  "ท่าถือปากกา (Hold Pen)",
]

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
  const [sendingHasImage, setSendingHasImage] = useState(false)
  const [streamedReply, setStreamedReply] = useState('')
  const [streamedResponse, setStreamedResponse] = useState<ChatResponse | null>(null)
  const [agentProgress, setAgentProgress] = useState<AgentProgress[]>([])
  const [imageDataUri, setImageDataUri] = useState<string | null>(null)
  const [imageName, setImageName] = useState('')
  const [pendingApproval, setPendingApproval] = useState<any | null>(null)
  const [modelVisible, setModelVisible] = useState(() => window.localStorage.getItem('mint:model-visible') !== 'false')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => window.localStorage.getItem('mint:sidebar-collapsed') === 'true')
  const [smartContext, setSmartContext] = useState(() => window.localStorage.getItem('mint:smart-context') !== 'false')
  const [agentMode, setAgentMode] = useState(() => window.localStorage.getItem('mint:agent-mode') !== 'false')
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
  const chatEnd = useRef<HTMLDivElement | null>(null)
  const startupReady = (dashboardDataReady && modelReady) || startupTimedOut

  async function refreshHistory() {
    const history = await getRecentInteractions()
    setInteractions(history.reverse())
  }

  async function refreshPictures() {
    setPictures(await listSavedPictures())
  }

  useEffect(() => {
    Promise.allSettled([
      getRuntimeStatus().then(setStatus),
      refreshHistory(),
      window.settingsApi?.getSettings()
        .then((loaded: any) => applyThemeStyles({ ...DEFAULT_CONFIG, ...loaded })),
    ]).then((results) => {
      const failure = results.find((result) => result.status === 'rejected')
      if (failure?.status === 'rejected') setError(errorMessage(failure.reason))
      setDashboardDataReady(true)
    })
    window.api.onSpotlightToChat((query) => {
      setView('chat')
      setMessage(query)
    })
    window.api?.onSettingsChanged?.(applyThemeStyles)

    const unlistenPromise = listen<any>('tool-approval-requested', (event) => setPendingApproval(event.payload))
    return () => {
      unlistenPromise.then((unlisten) => unlisten())
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
    if (!trimmed || sending) return
    const outgoingImage = imageDataUri
    setSending(true)
    setSendingMessage(trimmed)
    setSendingHasImage(Boolean(outgoingImage))
    setError('')
    setStreamedReply('')
    setStreamedResponse(null)
    setAgentProgress([])
    setMessage('')
    setImageDataUri(null)
    setImageName('')

    try {
      const response = await streamChatMessage(
        agentMode ? trimmed : `/chat ${trimmed}`,
        (chunk) => setStreamedReply((current) => `${current}${chunk}`),
        outgoingImage,
        null,
        '',
        (progress) => setAgentProgress((current) => [...current, progress].slice(-24)),
      )
      setStreamedResponse(response)
      await refreshHistory()
      await refreshPictures()
      setStreamedReply('')
      setStreamedResponse(null)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSending(false)
      setSendingMessage('')
      setSendingHasImage(false)
    }
  }

  async function selectImage(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    if (!file) return
    try {
      setImageDataUri(await readImage(file))
      setImageName(file.name)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      event.target.value = ''
    }
  }

  async function clearHistory(action: 'New chat' | 'Clear history') {
    if (!window.confirm(`${action} will clear the current conversation history. Continue?`)) return
    try {
      await clearChatHistory()
      setInteractions([])
      setStreamedReply('')
      setStreamedResponse(null)
      setMessage('')
      setImageDataUri(null)
      setImageName('')
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  async function changeProvider(provider: string) {
    try {
      const config = await window.settingsApi.getSettings()
      config.aiProvider = provider
      await window.settingsApi.saveSettings(config)
      setStatus(await getRuntimeStatus())
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
    setSendingHasImage(false)
    setError('')
    setStreamedReply('')
    setStreamedResponse(null)
    setAgentProgress([])

    try {
      const response = await streamChatMessage(`/chat ${interactionMessage}`, (chunk) => setStreamedReply((current) => `${current}${chunk}`), null, null, instruction)
      setStreamedResponse(response)
      await refreshHistory()
      setStreamedReply('')
      setStreamedResponse(null)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSending(false)
      setSendingMessage('')
      setSendingHasImage(false)
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
          onSetView={setView}
          onToggleModel={toggleModel}
          onSetExpressionIndex={setExpressionIndex}
          onSetAccessoryIndex={setAccessoryIndex}
          onSetInteractionEnabled={updateInteractionEnabled}
          onSetShowInteractionGuide={updateInteractionGuide}
          onShowToast={showToast}
        />
        <main className={`assistant-workspace ${layoutPreset === 'chat-wide' ? 'layout-chat-wide' : 'layout-model-wide'} ${modelVisible ? '' : 'model-hidden'}`}>
          <ModelPanel
            scale={scale}
            expressionIndex={expressionIndex}
            accessoryIndex={accessoryIndex}
            isLocked={isLocked}
            isActive={view !== 'pictures'}
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
            sending={sending}
            sendingMessage={sendingMessage}
            sendingHasImage={sendingHasImage}
            streamedReply={streamedReply}
            streamedResponse={streamedResponse}
            agentProgress={agentProgress}
            message={message}
            imageDataUri={imageDataUri}
            imageName={imageName}
            pendingApproval={pendingApproval}
            smartContext={smartContext}
            agentMode={agentMode}
            status={status}
            chatEnd={chatEnd}
            welcomeInteraction={MOCK_WELCOME_INTERACTION}
            onSubmit={handleSubmit}
            onSelectImage={selectImage}
            onSetMessage={setMessage}
            onRemoveImage={() => { setImageDataUri(null); setImageName('') }}
            onSetSmartContext={updateSmartContext}
            onSetAgentMode={updateAgentMode}
            onSetProvider={changeProvider}
            onApproval={handleApproval}
          />
        </main>
        <PicturesLibrary view={view} pictures={pictures} onSetView={setView} />
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
