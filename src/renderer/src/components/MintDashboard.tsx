import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import {
  clearChatHistory,
  getRecentInteractions,
  getRuntimeStatus,
  listSavedPictures,
  streamChatMessage,
  submitToolApproval,
  type ChatResponse,
  type InteractionMemory,
  type PictureEntry,
  type RuntimeStatus,
} from '../tauri'
import Live2DStage from './Live2DStage'

type DashboardView = 'chat' | 'pictures' | 'model'

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

function badge(provider: string, model: string) {
  return [provider, model].filter(Boolean).join(' / ')
}

interface ApprovalDetails {
  title: string
  body: string
  reason?: string
  isDangerous: boolean
}

function renderApprovalDetails(approval: any): ApprovalDetails {
  if (!approval) {
    return {
      title: 'Action Pending Approval',
      body: 'No action details available.',
      isDangerous: false,
    }
  }
  if (approval.WriteFile) {
    const { path, diff } = approval.WriteFile
    return {
      title: 'Write File',
      body: `Path: ${path}`,
      reason: diff ? `Diff:\n${diff}` : 'Writing new file content.',
      isDangerous: false,
    }
  }
  if (approval.ApplyPatch) {
    const { path, diff } = approval.ApplyPatch
    return {
      title: 'Apply Patch',
      body: `Path: ${path}`,
      reason: diff ? `Diff:\n${diff}` : 'Applying code patch.',
      isDangerous: false,
    }
  }
  if (approval.RunShell) {
    const { command } = approval.RunShell
    return {
      title: 'Run Shell Command',
      body: command,
      reason: 'Executing shell commands can modify your system.',
      isDangerous: true,
    }
  }
  if (approval.NoteWrite) {
    const { path } = approval.NoteWrite
    return {
      title: 'Write Note',
      body: `Path: ${path}`,
      reason: 'Creating or updating workspace notes.',
      isDangerous: false,
    }
  }
  if (approval.RunPlugin) {
    const { name, instruction } = approval.RunPlugin
    return {
      title: `Run Plugin: ${name}`,
      body: instruction,
      reason: 'Executing a native plugin action.',
      isDangerous: false,
    }
  }
  if (approval.McpTool) {
    const { server, tool, arguments: args } = approval.McpTool
    return {
      title: `Run MCP Tool: ${server}/${tool}`,
      body: typeof args === 'string' ? args : JSON.stringify(args, null, 2),
      reason: 'Running external MCP tool.',
      isDangerous: false,
    }
  }
  return {
    title: 'Unknown Action',
    body: JSON.stringify(approval, null, 2),
    reason: 'Requires approval to proceed.',
    isDangerous: false,
  }
}

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
      b: parseInt(result[3], 16)
  } : { r: 15, g: 23, b: 42 }
}

const applyThemeStyles = (cfg: any) => {
  const theme = cfg.theme || 'dark'
  const accentColor = cfg.accentColor || '#4f83e6'
  const systemTextColor = cfg.systemTextColor || '#f8fafc'
  const glassBlur = cfg.glassBlur || 'blur(16px)'
  const fontFamily = cfg.fontFamily || "'Outfit', sans-serif"
  const fontSize = cfg.fontSize || '15px'

  document.documentElement.setAttribute('data-theme', theme)
  document.documentElement.style.setProperty('--accent', accentColor)
  document.documentElement.style.setProperty('--accent-hover', lightenColor(accentColor, 20))
  document.documentElement.style.setProperty('--text-main', systemTextColor)
  document.documentElement.style.setProperty('--glass-blur', glassBlur)
  document.body.style.fontFamily = fontFamily
  document.documentElement.style.fontSize = fontSize

  if (theme === 'custom') {
    if (cfg.customBgStart && cfg.customBgEnd) {
      const gradient = `linear-gradient(135deg, ${cfg.customBgStart} 0%, ${cfg.customBgEnd} 100%)`
      document.documentElement.style.setProperty('--bg-color', cfg.customBgStart)
      document.documentElement.style.setProperty('--bg-gradient', gradient)
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
  } else {
    [
      '--bg-color',
      '--bg-gradient',
      '--panel-bg',
      '--panel-raised',
      '--panel-soft',
      '--chrome-bg',
      '--surface-bg',
      '--surface-strong',
      '--input-bg'
    ].forEach(name => document.documentElement.style.removeProperty(name))
  }
}

export default function MintDashboard() {
  const [view, setView] = useState<DashboardView>('chat')
  const [status, setStatus] = useState<RuntimeStatus | null>(null)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const [interactions, setInteractions] = useState<any[]>([])
  const [pictures, setPictures] = useState<PictureEntry[]>([])
  const [sending, setSending] = useState(false)
  const [streamedReply, setStreamedReply] = useState('')
  const [streamedResponse, setStreamedResponse] = useState<ChatResponse | null>(null)
  const [imageDataUri, setImageDataUri] = useState<string | null>(null)
  const [imageName, setImageName] = useState('')
  const [pendingApproval, setPendingApproval] = useState<any | null>(null)
  const [modelVisible, setModelVisible] = useState(
    () => window.localStorage.getItem('mint:model-visible') !== 'false',
  )
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [smartContext, setSmartContext] = useState(
    () => window.localStorage.getItem('mint:smart-context') !== 'false',
  )
  const [agentMode, setAgentMode] = useState(
    () => window.localStorage.getItem('mint:agent-mode') !== 'false',
  )
  const [scale, setScale] = useState(1.00)
  const [showInteractionGuide, setShowInteractionGuide] = useState(true)
  const [isLocked, setIsLocked] = useState(false)
  const [layoutPreset, setLayoutPreset] = useState<'chat-wide' | 'model-wide'>(
    () => (window.localStorage.getItem('mint:layout-preset') as 'chat-wide' | 'model-wide') || 'chat-wide',
  )
  const [toastMessage, setToastMessage] = useState('')
  const [expressionIndex, setExpressionIndex] = useState(0)
  const [accessoryIndex, setAccessoryIndex] = useState(0)
  const chatEnd = useRef<HTMLDivElement | null>(null)

  const EXPRESSIONS = [
    "ปกติ (Default)",
    "呆猫 (Dumb Cat)",
    "呆猫眼珠摇晃 (Dumb Cat Eye Roll)",
    "拍照 (Take Photo)",
    "拿笔 (Hold Pen)",
    "点一下 (Poke)",
    "猫咪滤镜 (Cat Filter)",
    "眼鏡 (Glasses)"
  ]

  const ACCESSORIES = [
    "ปกติ (None)",
    "ผ้ากันเปื้อน (Apron)",
    "ปืนประจำตัว Shiroko (Signature Rifle)"
  ]

  const showToast = (msg: string) => {
    setToastMessage(msg)
    // Clear toast after 3 seconds
    const timer = setTimeout(() => {
      setToastMessage((curr) => curr === msg ? '' : curr)
    }, 3000)
    return () => clearTimeout(timer)
  }

  const changeLayoutPreset = (preset: 'chat-wide' | 'model-wide') => {
    window.localStorage.setItem('mint:layout-preset', preset)
    setLayoutPreset(preset)
  }

  const mockWelcomeInteraction = {
    id: -1,
    userText: '',
    aiText: `มิ้นท์กำลังรอสแตนด์บายเตรียมพร้อมช่วยคุณทีมอยู่เลยค่ะ! ✨ แล้วก็แอบนั่งจัดระเบียบข้อมูลนิดๆ หน่อยๆ ให้พร้อมใช้ด้วยค่ะ 😊💖\n\nแต่พอคุณทีมทักมา มิ้นท์ก็วางมือจากทุกอย่างมาคุยกับคุณทีมก่อนเลยนะคะค้าา! ช่วงนี้มีอะไรให้มิ้นท์ช่วยดูแล หรืออยากชวนคุยเรื่องไหนเป็นพิเศษไหมคะ มิ้นท์พร้อมมว๊ากกกค่ะ! 🚀🎯`,
    provider: 'gemini',
    model: 'gemini-3-flash-preview',
    createdAt: new Date().toISOString()
  }

  async function refreshHistory() {
    const history = await getRecentInteractions()
    setInteractions(history.reverse())
  }

  async function refreshPictures() {
    setPictures(await listSavedPictures())
  }

  useEffect(() => {
    getRuntimeStatus().then(setStatus).catch((reason: unknown) => setError(errorMessage(reason)))
    refreshHistory().catch((reason: unknown) => setError(errorMessage(reason)))
    window.api.onSpotlightToChat((query) => {
      setView('chat')
      setMessage(query)
    })

    // Load and apply settings
    if (window.settingsApi) {
      window.settingsApi.getSettings().then((loaded: any) => {
        const merged = { ...DEFAULT_CONFIG, ...loaded }
        applyThemeStyles(merged)
      }).catch((err: unknown) => console.error("Error loading theme settings in dashboard:", err))
    }

    // Listen for settings changed event
    if (window.api && typeof window.api.onSettingsChanged === 'function') {
      window.api.onSettingsChanged((updatedConfig: any) => {
        applyThemeStyles(updatedConfig)
      })
    }

    const unlistenPromise = listen<any>('tool-approval-requested', (event) => {
      setPendingApproval(event.payload)
    })

    return () => {
      unlistenPromise.then((unlisten) => unlisten())
    }
  }, [])

  useEffect(() => {
    if (view === 'pictures') {
      refreshPictures().catch((reason: unknown) => setError(errorMessage(reason)))
    }
  }, [view])

  useEffect(() => {
    chatEnd.current?.scrollIntoView({ behavior: 'smooth' })
  }, [interactions, streamedReply, pendingApproval])

  async function handleApproval(approved: boolean) {
    if (!pendingApproval) return
    const token = pendingApproval.token
    try {
      await submitToolApproval(token, approved)
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
    setSending(true)
    setError('')
    setStreamedReply('')
    setStreamedResponse(null)

    let finalMsg = trimmed
    if (!agentMode) {
      finalMsg = `/chat ${trimmed}`
    }

    try {
      const response = await streamChatMessage(
        finalMsg,
        (chunk) => setStreamedReply((current) => `${current}${chunk}`),
        imageDataUri,
      )
      setStreamedResponse(response)
      setMessage('')
      setImageDataUri(null)
      setImageName('')
      await refreshHistory()
      await refreshPictures()
      setStreamedReply('')
      setStreamedResponse(null)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSending(false)
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

  function handlePokeCheek() {
    const newInteractionId = Date.now()
    const systemEvent = {
      id: newInteractionId,
      userText: "The user poked Mint model on the cheek. Reply briefly, shyly or with a light tease. Use the same language as the user's recent conversation; do not switch to Thai unless the user has been speaking Thai.",
      aiText: "อุ๊ย! คุณทีมจิ้มแก้มมิ้นท์ทำไมคะเนี่ยยย มิ้นท์เขินจนแก้มจะระเบิดอยู่แล้วนะคะค้าาา! >///< แกล้งกันแบบนี้เดี๋ยวมิ้นท์ก็งอนซะเลย (แต่ล้อเล่นนะคะ อิอิ) มีอะไรอยากให้มิ้นท์ช่วยทำต่อไหมคะ หรืออยากชวนมิ้นท์คุยเรื่องไหนดีเอ่ย? 💖✨😊",
      provider: "gemini",
      model: "gemini-3-flash-preview",
      createdAt: new Date().toISOString(),
      isSystemEvent: true
    }
    setInteractions((current) => [...current, systemEvent])
  }

  function toggleModel() {
    const next = !modelVisible
    window.localStorage.setItem('mint:model-visible', String(next))
    setModelVisible(next)
  }

  return (
    <div className="app-container">

      {/* Main App Body */}
      <div className={`app-body ${sidebarCollapsed ? 'sidebar-collapsed' : ''} ${view === 'pictures' ? 'pictures-open' : ''}`}>
        {/* Sidebar Toggle Button */}
        <button className="sidebar-toggle" onClick={() => setSidebarCollapsed(!sidebarCollapsed)}>
          <span className="sidebar-toggle-icon">&lt;</span>
        </button>

        {/* Sidebar Panel */}
        <aside className="workspace-sidebar">
          <button className="sidebar-new-chat" onClick={() => clearHistory('New chat')}>
            <span aria-hidden="true">+</span>
            <span>New Chat</span>
          </button>

          <button className={`sidebar-top-action ${view === 'chat' ? 'is-active' : ''}`} onClick={() => setView('chat')}>
            <span aria-hidden="true">🗏</span>
            <span>Chat</span>
          </button>
          <button className={`sidebar-top-action ${view === 'pictures' ? 'is-active' : ''}`} onClick={() => setView('pictures')}>
            <span aria-hidden="true">▨</span>
            <span>Pictures</span>
          </button>

          {/* Model Section */}
          <div className="sidebar-section">
            <div
              className="sidebar-section-title clickable"
              onClick={toggleModel}
              title="เปิด/ปิดการแสดง Live2D Model"
            >
              ☰ Model
            </div>
            <div className="sidebar-model-controls">
              <button
                className="change-expression-btn"
                onClick={() => {
                  const nextExpr = (expressionIndex + 1) % EXPRESSIONS.length
                  setExpressionIndex(nextExpr)
                  showToast(`สลับสีหน้าของ Shiroko เป็น: ${EXPRESSIONS[nextExpr]}`)
                }}
              >
                <span aria-hidden="true">☆</span>
                <span>Expression</span>
              </button>
              <button
                className="accessory-cycle-btn"
                onClick={() => {
                  const nextAcc = (accessoryIndex + 1) % ACCESSORIES.length
                  setAccessoryIndex(nextAcc)
                  showToast(`สลับเครื่องประดับของ Shiroko เป็น: ${ACCESSORIES[nextAcc]}`)
                }}
              >
                <span aria-hidden="true">∞</span>
                <span>Accessory</span>
              </button>
              <button
                className={`toggle-interaction-btn ${showInteractionGuide ? 'active' : ''}`}
                onClick={() => {
                  const next = !showInteractionGuide
                  setShowInteractionGuide(next)
                  showToast(next ? "เปิดการแสดงจุดสัมผัส (Interaction Zones) ⦸" : "ปิดการแสดงจุดสัมผัส ⦸")
                }}
              >
                <span aria-hidden="true">⦸</span>
                <span className="mint-status-label">Interact</span>
              </button>
              <button
                className="interaction-guide-btn"
                onClick={() => {
                  const next = !showInteractionGuide
                  setShowInteractionGuide(next)
                  showToast(next ? "เปิดการแสดงจุดสัมผัส (Interaction Zones) ⊹" : "ปิดการแสดงจุดสัมผัส ⊹")
                }}
              >
                <span aria-hidden="true">⊹</span>
                <span>Areas</span>
              </button>
            </div>
          </div>

          {/* Assistant Section */}
          <div className="sidebar-section">
            <div className="sidebar-section-title">Assistant</div>
            <button className="sidebar-project active">
              <span aria-hidden="true">🗏</span>
              <span>Mint</span>
              <span className="mint-status-pill" data-state={sending ? "thinking" : "idle"}>
                <span className="mint-status-dot" />
                <span className="mint-status-label">{sending ? "Thinking" : "Idle"}</span>
              </span>
            </button>
            <button
              className={`sidebar-project ${modelVisible ? 'is-active' : ''}`}
              onClick={toggleModel}
            >
              <span aria-hidden="true">♢</span>
              <span>Live2D Model</span>
            </button>
            <button
              className="sidebar-project"
              onClick={() => showToast("เปิดใช้งาน Smart Tools: พร้อมช่วยสแกนและวิเคราะห์แล้วค่ะ! ∽")}
            >
              <span aria-hidden="true">∽</span>
              <span>Smart Tools</span>
            </button>
          </div>

          {/* Sidebar Footer Actions */}
          <div className="sidebar-bottom-actions">
            <button className="clear-btn" onClick={() => clearHistory('Clear history')}>
              <span aria-hidden="true">🗑</span>
              <span>Clear</span>
            </button>
            <button className="settings-btn" onClick={() => window.api.openSettings()}>
              <span aria-hidden="true">⚙</span>
              <span>Settings</span>
            </button>
          </div>
        </aside>

        {/* Assistant Workspace (Center and Right columns) */}
        <main className={`assistant-workspace ${layoutPreset === 'chat-wide' ? 'layout-chat-wide' : 'layout-model-wide'} ${modelVisible ? '' : 'model-hidden'}`}>
          {/* Left Column: Live2D Stage */}
          <section className="model-stage">
            <div className={`model-shell ${showInteractionGuide ? 'show-interaction-guide' : ''} model-bg-stage`}>
              {/* Floating controls bar */}
              <div className="model-panel-controls">
                <button
                  className={`model-panel-control ${isLocked ? 'is-active' : ''}`}
                  onClick={() => setIsLocked(!isLocked)}
                  title={isLocked ? 'Unlock stage interactions' : 'Lock stage interactions'}
                >
                  {isLocked ? (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                      <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                    </svg>
                  ) : (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                      <path d="M7 11V7a5 5 0 0 1 9.9-1"></path>
                    </svg>
                  )}
                </button>
                <button
                  className="model-panel-control"
                  onClick={() => window.api.maximizeWindow()}
                  title="Maximize Window"
                >
                  ⤢
                </button>
                <div className="model-scale-control">
                  <input
                    type="range"
                    min="0.5"
                    max="2.0"
                    step="0.05"
                    value={scale}
                    onChange={(e) => setScale(parseFloat(e.target.value))}
                    disabled={isLocked}
                  />
                  <span className="model-scale-value">{scale.toFixed(2)}x</span>
                  <button
                    className="model-scale-reset"
                    onClick={() => setScale(1.00)}
                    disabled={isLocked}
                    title="Reset Scale"
                  >
                    ⟲
                  </button>
                </div>
                <button className="model-panel-control" onClick={() => setView('pictures')} title="Saved Pictures">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                    <circle cx="8.5" cy="8.5" r="1.5"></circle>
                    <polyline points="21 15 16 10 5 21"></polyline>
                  </svg>
                </button>
                <div className="layout-preset-group">
                  <button
                    className={`layout-preset-btn ${layoutPreset === 'chat-wide' ? 'is-active' : ''}`}
                    onClick={() => changeLayoutPreset('chat-wide')}
                    title="ขยายหน้าต่างแชท (ย่อโมเดล)"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect x="3" y="3" width="18" height="18" rx="2" />
                      <path d="M9 3v18" />
                    </svg>
                  </button>
                  <button
                    className={`layout-preset-btn ${layoutPreset === 'model-wide' ? 'is-active' : ''}`}
                    onClick={() => changeLayoutPreset('model-wide')}
                    title="ขยายหน้าต่างโมเดล (ย่อแชท)"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect x="3" y="3" width="18" height="18" rx="2" />
                      <path d="M15 3v18" />
                    </svg>
                  </button>
                </div>
              </div>

              {/* Status Badge */}
              <div className="model-activity-badge" data-state={sending ? "thinking" : "idle"}>
                <span className="mint-status-dot" />
                <span>{sending ? "Thinking" : "Idle"}</span>
              </div>

              {/* Model Mount & Display */}
              <div
                className="model-mount"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  pointerEvents: isLocked ? 'none' : 'auto'
                }}
              >
                <Live2DStage
                  scale={scale}
                  expressionIndex={expressionIndex}
                  accessoryIndex={accessoryIndex}
                  isLocked={isLocked}
                />

                {/* Interaction Guide Zones */}
                <div className="interaction-guide">
                  <div className="interaction-zone zone-head" onClick={() => alert("Poked head!")}>
                    <span>Head</span>
                  </div>
                  <div className="interaction-zone zone-face" onClick={handlePokeCheek}>
                    <span>Cheek</span>
                  </div>
                  <div className="interaction-zone zone-left-hand" onClick={() => alert("Poked left hand!")}>
                    <span>Hand</span>
                  </div>
                  <div className="interaction-zone zone-right-hand" onClick={() => alert("Poked right hand!")}>
                    <span>Hand</span>
                  </div>
                  <div className="interaction-zone zone-body" onClick={() => alert("Poked body!")}>
                    <span>Body</span>
                  </div>
                  <div className="interaction-zone zone-lower" onClick={() => alert("Poked legs!")}>
                    <span>Lower</span>
                  </div>
                </div>
              </div>
            </div>
          </section>

          {/* Right Column: Chat/Conversation Panel */}
          <section className="conversation-panel">
            <div className="chat-container">
              {interactions.length === 0 && !sending && (
                <div className="message ai-message" style={{ marginBottom: '16px' }}>
                  <div className="bubble-wrapper">
                    <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>
                      {mockWelcomeInteraction.aiText}
                    </div>
                    <div className="message-time">
                      <button className="provider-badge">
                        {mockWelcomeInteraction.provider} • {mockWelcomeInteraction.model}
                      </button>
                      <span>14:44</span>
                    </div>
                  </div>
                </div>
              )}

              {interactions.map((interaction) => (
                <div key={interaction.id} style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
                  {/* Render System Poked Event if flagged */}
                  {interaction.isSystemEvent ? (
                    <div
                      className="system-event"
                      style={{
                        background: 'rgba(16, 185, 129, 0.06)',
                        border: '1px solid rgba(16, 185, 129, 0.2)',
                        borderRadius: '8px',
                        padding: '10px 14px',
                        color: '#a7f3d0',
                        fontSize: '0.82rem',
                        lineHeight: '1.45',
                        alignSelf: 'stretch'
                      }}
                    >
                      {interaction.userText}
                    </div>
                  ) : (
                    interaction.userText && (
                      <div className="message user-message">
                        <div className="bubble-wrapper">
                          <div className="message-bubble">{interaction.userText}</div>
                          <div className="message-time">
                            <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                          </div>
                        </div>
                      </div>
                    )
                  )}

                  <div className="message ai-message">
                    <div className="bubble-wrapper">
                      <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>
                        {interaction.aiText}
                      </div>
                      <div className="message-time">
                        <button className="provider-badge">
                          {interaction.provider} • {interaction.model}
                        </button>
                        <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                      </div>
                    </div>
                  </div>
                </div>
              ))}

              {sending && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
                  <div className="message user-message">
                    <div className="bubble-wrapper">
                      <div className="message-bubble">
                        {imageDataUri ? `${message} [Image #1]` : message}
                      </div>
                    </div>
                  </div>
                  <div className="message ai-message thinking-message">
                    <div className="bubble-wrapper">
                      <div className="message-bubble">
                        <span>{streamedReply || 'Thinking...'}</span>
                      </div>
                      {streamedResponse && (
                        <div className="message-time">
                          <button className="provider-badge">
                            {badge(streamedResponse.provider, streamedResponse.model)}
                          </button>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}

              {/* Tool Approval Card Block */}
              {pendingApproval && (() => {
                const details = renderApprovalDetails(pendingApproval.approval)
                return (
                  <div className="message ai-message" style={{ width: '100%' }}>
                    <div className="bubble-wrapper" style={{ width: '100%' }}>
                      <div className="action-card approval-card" data-tier={details.isDangerous ? 'dangerous' : undefined} style={{ width: '100%' }}>
                        <div className="approval-card-content">
                          <div className="approval-card-title">{details.title}</div>
                          <div className="approval-card-body" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>
                            {details.body}
                          </div>
                          {details.reason && (
                            <div className="approval-card-reason" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>
                              {details.reason}
                            </div>
                          )}
                        </div>
                        <div className="approval-card-actions">
                          <button
                            type="button"
                            className="approval-btn approval-btn-approve"
                            onClick={() => handleApproval(true)}
                          >
                            Approve
                          </button>
                          <button
                            type="button"
                            className="approval-btn approval-btn-cancel"
                            onClick={() => handleApproval(false)}
                          >
                            Cancel
                          </button>
                        </div>
                      </div>
                    </div>
                  </div>
                )
              })()}

              <div ref={chatEnd} />
            </div>

            {/* Input Composer Block */}
            <div className="input-area">
              {/* Smart context bar toggles */}
              <div className="smart-context-bar">
                <div className="smart-context-label" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <label className="toggle-switch">
                    <input
                      type="checkbox"
                      checked={smartContext}
                      onChange={(e) => {
                        setSmartContext(e.target.checked)
                        window.localStorage.setItem('mint:smart-context', String(e.target.checked))
                      }}
                    />
                    <span className="slider round" />
                  </label>
                  <span>Smart Context (Auto-Screen)</span>
                </div>
                <div className="smart-context-label" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <label className="toggle-switch">
                    <input
                      type="checkbox"
                      checked={agentMode}
                      onChange={(e) => {
                        setAgentMode(e.target.checked)
                        window.localStorage.setItem('mint:agent-mode', String(e.target.checked))
                      }}
                    />
                    <span className="slider round" />
                  </label>
                  <span>Agent Mode</span>
                </div>
              </div>

              {/* Chat Form */}
              <form id="chat-form" onSubmit={handleSubmit}>
                {imageDataUri && (
                  <div
                    className="mint-attachment"
                    style={{
                      gridColumn: '1 / -1',
                      gridRow: '1',
                      marginBottom: '8px',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px'
                    }}
                  >
                    <img src={imageDataUri} alt="" style={{ width: '32px', height: '32px', borderRadius: '4px' }} />
                    <span style={{ fontSize: '0.76rem', color: 'var(--text-soft)' }}>{imageName}</span>
                    <button
                      type="button"
                      onClick={() => { setImageDataUri(null); setImageName('') }}
                      style={{ background: 'transparent', border: 0, color: '#ef4444', cursor: 'pointer' }}
                    >
                      ✕
                    </button>
                  </div>
                )}

                <textarea
                  id="chat-input"
                  value={message}
                  onChange={(event) => setMessage(event.target.value)}
                  placeholder="Ask anything, @ to mention, / for actions"
                  rows={2}
                />

                {/* Left Action Button (Vision) */}
                <button
                  id="vision-btn"
                  type="button"
                  onClick={() => {
                    const fileInput = document.getElementById('vision-file-input')
                    fileInput?.click()
                  }}
                >
                  👁
                </button>
                <input
                  id="vision-file-input"
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/gif"
                  onChange={selectImage}
                  style={{ display: 'none' }}
                />

                {/* Center Provider Dropdown Selection */}
                <select
                  className="chat-provider-select"
                  value={status?.activeProvider ?? ''}
                  onChange={async (e) => {
                    const nextProvider = e.target.value
                    try {
                      const config = await window.settingsApi.getSettings()
                      config.aiProvider = nextProvider
                      await window.settingsApi.saveSettings(config)
                      const nextStatus = await getRuntimeStatus()
                      setStatus(nextStatus)
                    } catch (reason) {
                      setError(errorMessage(reason))
                    }
                  }}
                >
                  {status?.availableProviders.map((prov) => (
                    <option key={prov} value={prov}>
                      {prov}
                    </option>
                  ))}
                </select>

                {/* Mic Action Button */}
                <button id="mic-btn" type="button" onClick={() => alert("Voice transcription coming soon!")}>
                  🎙
                </button>

                {/* Send Button */}
                <button id="send-btn" type="submit" disabled={sending || !message.trim()}>
                  ➤
                </button>
              </form>
            </div>
          </section>
        </main>

        {/* Saved Pictures Gallery section overlay */}
        <section className="pictures-library" hidden={view !== 'pictures'}>
          <header className="pictures-header">
            <div>
              <span className="pictures-kicker">Gallery</span>
              <h2>Saved Pictures</h2>
            </div>
            <button className="pictures-close-btn" onClick={() => setView('chat')}>
              Close Gallery
            </button>
          </header>
          {pictures.length === 0 ? (
            <div className="pictures-empty">
              <div className="pictures-empty-icon" style={{ display: 'flex', justifyContent: 'center', marginBottom: '12px', opacity: 0.3 }}>
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                  <circle cx="8.5" cy="8.5" r="1.5"></circle>
                  <polyline points="21 15 16 10 5 21"></polyline>
                </svg>
              </div>
              <p>No saved pictures yet</p>
              <span>Images appear here after a message with an attachment is sent successfully.</span>
            </div>
          ) : (
            <div className="pictures-grid">
              {pictures.map((picture) => (
                <a
                  className="picture-card"
                  href={picture.url}
                  target="_blank"
                  rel="noreferrer"
                  key={picture.id}
                  onClick={(e) => {
                    e.preventDefault()
                    if (window.settingsApi) {
                      window.settingsApi.openExternal(picture.url || '')
                    }
                  }}
                >
                  <img src={convertFileSrc(picture.path)} alt={picture.message || picture.filename} />
                  <div className="picture-card-meta">
                    {picture.message || picture.filename}
                  </div>
                </a>
              ))}
            </div>
          )}
        </section>
      </div>

      {error && (
        <div
          className="mint-error"
          style={{
            position: 'absolute',
            bottom: '20px',
            right: '20px',
            zIndex: 100,
            margin: 0,
            boxShadow: '0 8px 24px rgba(0,0,0,0.3)'
          }}
        >
          {error}
          <button
            onClick={() => setError('')}
            style={{ marginLeft: '12px', background: 'transparent', border: 0, color: 'white', cursor: 'pointer' }}
          >
            ✕
          </button>
        </div>
      )}

      {toastMessage && (
        <div
          className="mint-notification"
          style={{
            position: 'absolute',
            bottom: error ? '80px' : '20px',
            right: '20px',
            zIndex: 100,
            margin: 0,
            padding: '9px 14px',
            borderRadius: '8px',
            background: 'rgba(124, 58, 237, 0.95)',
            border: '1px solid rgba(167, 139, 250, 0.4)',
            color: '#f8fafc',
            fontSize: '0.82rem',
            boxShadow: '0 8px 24px rgba(0,0,0,0.3)',
            display: 'flex',
            alignItems: 'center',
            gap: '12px',
            transition: 'all 0.22s ease'
          }}
        >
          <span>{toastMessage}</span>
          <button
            onClick={() => setToastMessage('')}
            style={{ background: 'transparent', border: 0, color: 'white', cursor: 'pointer', fontSize: '0.9rem' }}
          >
            ✕
          </button>
        </div>
      )}
    </div>
  )
}
