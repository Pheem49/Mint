import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
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
  const [smartContext, setSmartContext] = useState(true)
  const [agentMode, setAgentMode] = useState(true)
  const [scale, setScale] = useState(1.00)
  const [showInteractionGuide, setShowInteractionGuide] = useState(true)
  const [isLocked, setIsLocked] = useState(false)
  const chatEnd = useRef<HTMLDivElement | null>(null)

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
      {/* Drag Region / Titlebar Header */}
      <header className="drag-region">
        <div className="header-content">
          <img src="/assets/icon.png" className="app-logo" alt="logo" />
          <h1>Agent Mint</h1>
        </div>
        <div className="titlebar-drag-space" />
        <div className="window-controls">
          <button className="minimize-btn" onClick={() => window.api.minimizeWindow()}>−</button>
          <button className="maximize-btn" onClick={() => window.api.maximizeWindow()}>⬜</button>
          <button className="close-btn" onClick={() => window.api.closeWindow()}>✕</button>
        </div>
      </header>

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
            <span aria-hidden="true">💬</span>
            <span>Chat</span>
          </button>
          <button className={`sidebar-top-action ${view === 'pictures' ? 'is-active' : ''}`} onClick={() => setView('pictures')}>
            <span aria-hidden="true">🖼️</span>
            <span>Pictures</span>
          </button>

          {/* Model Section */}
          <div className="sidebar-section">
            <div className="sidebar-section-title">Model</div>
            <div className="sidebar-model-controls">
              <button className="change-expression-btn">
                <span aria-hidden="true">⭐</span>
                <span>Expression</span>
              </button>
              <button className="accessory-cycle-btn">
                <span aria-hidden="true">♾️</span>
                <span>Accessory</span>
              </button>
              <button
                className={`toggle-interaction-btn ${showInteractionGuide ? 'active' : ''}`}
                onClick={() => setShowInteractionGuide(!showInteractionGuide)}
              >
                <span aria-hidden="true">🎯</span>
                <span className="mint-status-label">Interact</span>
              </button>
              <button className="interaction-guide-btn" onClick={() => setShowInteractionGuide(!showInteractionGuide)}>
                <span aria-hidden="true">✛</span>
                <span>Areas</span>
              </button>
            </div>
          </div>

          {/* Assistant Section */}
          <div className="sidebar-section">
            <div className="sidebar-section-title">Assistant</div>
            <button className="sidebar-project active">
              <span aria-hidden="true">🟢</span>
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
              <span aria-hidden="true">💠</span>
              <span>Live2D Model</span>
            </button>
            <button className="sidebar-project">
              <span aria-hidden="true">⚙️</span>
              <span>Smart Tools</span>
            </button>
          </div>

          {/* Sidebar Footer Actions */}
          <div className="sidebar-bottom-actions">
            <button className="clear-btn" onClick={() => clearHistory('Clear history')}>
              <span aria-hidden="true">🗑️</span>
              <span>Clear</span>
            </button>
            <button className="settings-btn" onClick={() => window.api.openSettings()}>
              <span aria-hidden="true">⚙️</span>
              <span>Settings</span>
            </button>
          </div>
        </aside>

        {/* Assistant Workspace (Center and Right columns) */}
        <main className={`assistant-workspace layout-chat ${modelVisible ? '' : 'model-hidden'}`}>
          {/* Left Column: Live2D Stage */}
          <section className="model-stage">
            <div className={`model-shell ${showInteractionGuide ? 'show-interaction-guide' : ''} model-bg-stage`}>
              {/* Floating controls bar */}
              <div className="model-panel-controls">
                <button
                  className={`model-panel-control ${isLocked ? 'is-active' : ''}`}
                  onClick={() => setIsLocked(!isLocked)}
                >
                  {isLocked ? '🔒' : '🔓'}
                </button>
                <button className="model-panel-control">⤢</button>
                <div className="model-scale-control">
                  <input
                    type="range"
                    min="0.5"
                    max="2.0"
                    step="0.05"
                    value={scale}
                    onChange={(e) => setScale(parseFloat(e.target.value))}
                  />
                  <span className="model-scale-value">{scale.toFixed(2)}x</span>
                  <button className="model-scale-reset" onClick={() => setScale(1.00)}>⟲</button>
                </div>
                <button className="model-panel-control" onClick={() => setView('pictures')}>🖼️</button>
                <div className="layout-preset-group">
                  <button className="layout-preset-btn is-active">◫</button>
                  <button className="layout-preset-btn">⊟</button>
                </div>
              </div>

              {/* Status Badge */}
              <div className="model-activity-badge" data-state={sending ? "thinking" : "idle"}>
                <span className="mint-status-dot" />
                <span>{sending ? "Thinking" : "Idle"}</span>
              </div>

              {/* Model Mount & Display */}
              <div className="model-mount" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <img
                  src="/live2d_mint_placeholder.png"
                  onError={(e) => {
                    // Fallback if public asset is not found/copied yet
                    e.currentTarget.src = "https://images.unsplash.com/photo-1607604276583-eef5d076aa5f?w=600&auto=format&fit=crop"
                  }}
                  className="model-placeholder"
                  style={{
                    maxHeight: '85%',
                    transform: `scale(${scale})`,
                    transition: 'transform 0.15s ease-out',
                    borderRadius: '12px'
                  }}
                  alt="Mint Live2D"
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
                  <div className="message ai-message">
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
                      onChange={(e) => setSmartContext(e.target.checked)}
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
                      onChange={(e) => setAgentMode(e.target.checked)}
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
              <div className="pictures-empty-icon">🖼️</div>
              <p>No saved pictures yet</p>
              <span>Images appear here after a message with an attachment is sent successfully.</span>
            </div>
          ) : (
            <div className="pictures-grid">
              {pictures.map((picture) => (
                <a className="picture-card" href={picture.url} target="_blank" rel="noreferrer" key={picture.id}>
                  <img src={picture.url} alt={picture.message || picture.filename} />
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
    </div>
  )
}
