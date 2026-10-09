import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import Live2DStage from './Live2DStage'
import { activeTurn, applyTurn, characterStatus, modelExpression, terminal, type Event, type Host, type Session, type Turn } from './state'

type Settings = { hostId: string; chatId: string; scale: number; expression: number; accessory: number; locked: boolean; guide: boolean; alwaysOnTop: boolean; normalWindow: boolean; panelOpen: boolean }
const defaults: Settings = { hostId: '', chatId: '', scale: 1, expression: -1, accessory: 0, locked: false, guide: false, alwaysOnTop: true, normalWindow: false, panelOpen: true }
const expressions = ['Auto', 'Default', 'Dazed', 'Dazed eyes', 'Photo', 'Poke', 'Cat filter']
const accessories = ['None', 'Apron', 'Glasses', 'Pen']
const areas = ['head', 'cheek', 'left hand', 'right hand', 'body', 'lower body']
export default function App() {
  const [settings, setSettings] = useState(defaults)
  const settingsRef = useRef(settings); settingsRef.current = settings
  const [ready, setReady] = useState(false)
  const [hosts, setHosts] = useState<Host[]>([])
  const [sessions, setSessions] = useState<Session[]>([])
  const [turns, setTurns] = useState<Turn[]>([])
  const [online, setOnline] = useState(false)
  const [connectionError, setConnectionError] = useState('')
  const [error, setError] = useState('')
  const [text, setText] = useState('')
  const [sending, setSending] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [visible, setVisible] = useState(true)
  const [modelError, setModelError] = useState('')
  const [modelKey, setModelKey] = useState(0)
  const [feedback, setFeedback] = useState(false)
  const hostRef = useRef(''); hostRef.current = settings.hostId
  const scrollRef = useRef<HTMLDivElement>(null)
  const current = activeTurn(turns, settings.chatId)
  const sessionExists = sessions.some(s => s.chatId === settings.chatId)
  const status = online ? sessionExists ? current?.status ?? 'idle' : 'choose conversation' : 'offline'
  const canSend = online && sessionExists && !sending

  useEffect(() => {
    let disposed = false
    const subscriptions: Array<() => void> = []
    const initialize = async () => {
      subscriptions.push(await listen<{ hostId: string; online: boolean; message: string }>('companion-connection', ({ payload }) => {
        if (payload.hostId !== hostRef.current) return
        setOnline(payload.online); setConnectionError(payload.message)
      }))
      subscriptions.push(await listen<Event>('companion-event', ({ payload }) => {
        if ('hostId' in payload && payload.hostId !== hostRef.current) return
        if (payload.type === 'snapshot') {
          if (payload.version !== 1) { setOnline(false); setConnectionError('Update Mint and Companion to compatible versions.'); return }
          setSessions(payload.sessions)
          setTurns(previous => payload.turns.reduce((next, turn) => applyTurn(next, turn, payload.hostId, hostRef.current), previous))
        } else if (payload.type === 'update') {
          setTurns(previous => applyTurn(previous, payload.turn, payload.hostId, hostRef.current))
        } else if (payload.type === 'error') setError(payload.message)
      }))
      subscriptions.push(await listen<boolean>('companion-visible', ({ payload }) => setVisible(payload)))
      const saved = await invoke<Settings>('load_settings')
      if (!disposed) { setSettings({ ...defaults, ...saved }); setReady(true) }
      else subscriptions.splice(0).forEach(unlisten => unlisten())
    }
    initialize().catch(reason => { if (!disposed) { setError(String(reason)); setReady(true) } })
    return () => { disposed = true; subscriptions.splice(0).forEach(unlisten => unlisten()) }
  }, [])
  useEffect(() => {
    if (!ready) return
    let disposed = false
    const discover = async () => {
      try {
        const found = await invoke<Host[]>('discover_hosts')
        if (disposed) return
        setHosts(found)
        if (!settingsRef.current.hostId && found.length === 1) setSettings(s => ({ ...s, hostId: found[0].hostId }))
      } catch (reason) { if (!disposed) setConnectionError(String(reason)) }
    }
    discover(); const timer = setInterval(discover, 5000)
    return () => { disposed = true; clearInterval(timer) }
  }, [ready])
  useEffect(() => {
    if (!ready) return
    setOnline(false); setSessions([]); setTurns([]); setError('')
    invoke('connect_host', { hostId: settings.hostId, chatId: settings.chatId }).catch(reason => setConnectionError(String(reason)))
  }, [ready, settings.hostId])
  useEffect(() => {
    if (!online || !settings.chatId || !sessionExists) return
    invoke('send_command', { command: { type: 'subscribe', chatId: settings.chatId } }).catch(reason => setError(String(reason)))
  }, [online, settings.chatId, sessionExists])
  useEffect(() => {
    if (!settings.chatId && sessions.length === 1) setSettings(s => ({ ...s, chatId: sessions[0].chatId }))
  }, [sessions, settings.chatId])
  useEffect(() => {
    if (!ready) return
    const timer = setTimeout(() => { invoke('save_settings', { settings }).catch(reason => setError(String(reason))) }, 250)
    return () => clearTimeout(timer)
  }, [settings, ready])
  useEffect(() => {
    setFeedback(true)
    const timer = setTimeout(() => setFeedback(false), 2200)
    return () => clearTimeout(timer)
  }, [current?.turnId, current?.status])
  useEffect(() => { scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight }) }, [turns])
  async function send(message: string, area?: string) {
    if (!canSend || (!area && !message.trim())) return
    setSending(true); setError('')
    try {
      await invoke('send_command', { command: area ? { type: 'interact', chatId: settings.chatId, requestId: crypto.randomUUID(), area } : { type: 'send_chat', chatId: settings.chatId, requestId: crypto.randomUUID(), message: message.trim() } })
      if (!area) setText('')
    } catch (reason) { setError(`Message could not be submitted: ${String(reason)}. It will not be retried automatically.`) }
    finally { setSending(false) }
  }
  const automaticStatus = characterStatus(feedback || !current || !terminal(current.status) ? current?.status ?? 'idle' : 'idle', online, sessionExists)
  return <main className={`companion ${settings.normalWindow ? 'normal-window' : ''}`}>
    <header className="toolbar">
      <button className="drag-handle" title="Drag window" onPointerDown={event => { if (event.button === 0) getCurrentWindow().startDragging() }}>Mint Companion</button>
      <span className="status" role="status">{status === 'working' && current?.tool ? `Working · ${current.tool}` : status}</span>
      <button aria-label="Toggle chat" aria-pressed={settings.panelOpen} onClick={() => setSettings(s => ({ ...s, panelOpen: !s.panelOpen }))}>Chat</button>
      <button aria-label="Toggle settings" aria-pressed={settingsOpen} onClick={() => setSettingsOpen(v => !v)}>Settings</button>
      <button aria-label="Hide Companion" onClick={() => invoke('hide_window')}>Hide</button>
      <button aria-label="Quit Companion" onClick={() => invoke('quit_app')}>×</button>
    </header>
    <div className="character">
      <Live2DStage key={modelKey} scale={settings.scale} expressionIndex={modelExpression(settings.expression, automaticStatus)} accessoryIndex={settings.accessory} isLocked={settings.locked} isActive={visible} status={automaticStatus} onLoadError={setModelError} />
      {settings.guide && <div className="interaction-guide">{areas.map((area, index) => <button key={area} className={`area area-${index}`} disabled={!canSend || settings.locked} onClick={() => send('', area)}>{area}</button>)}</div>}
      {!settings.guide && <button className="pat-button" disabled={!canSend || settings.locked} onClick={() => send('', 'head')}>Pat Mint</button>}
      {modelError && <div className="model-error" role="alert">The character could not load. Chat and settings are available.<button onClick={() => { setModelError(''); setModelKey(k => k + 1) }}>Retry character</button></div>}
    </div>
    {settingsOpen && <section className="settings-panel" aria-label="Companion settings">
      <label>Size<input type="range" min="0.5" max="1.5" step="0.05" value={settings.scale} onChange={e => setSettings(s => ({ ...s, scale: +e.target.value }))} /></label>
      <label>Expression<select value={settings.expression} onChange={e => setSettings(s => ({ ...s, expression: +e.target.value }))}>{expressions.map((name, i) => <option key={name} value={i-1}>{name}</option>)}</select></label>
      <label>Accessory<select value={settings.accessory} onChange={e => setSettings(s => ({ ...s, accessory: +e.target.value }))}>{accessories.map((name, i) => <option key={name} value={i}>{name}</option>)}</select></label>
      {([['locked', 'Lock interactions'], ['guide', 'Show interaction areas'], ['alwaysOnTop', 'Always on top'], ['normalWindow', 'Normal window']] as const).map(([key, label]) => <label key={key} className="check"><input type="checkbox" checked={settings[key]} onChange={e => setSettings(s => ({ ...s, [key]: e.target.checked }))} />{label}</label>)}
    </section>}
    {settings.panelOpen && <section className="chat-panel" aria-label="Chat with Mint">
      <div className="connection-fields">
        <label>Mint instance<select value={settings.hostId} onChange={e => setSettings(s => ({ ...s, hostId: e.target.value, chatId: '' }))}><option value="">Choose Mint</option>{settings.hostId && !hosts.some(h => h.hostId === settings.hostId) && <option value={settings.hostId}>Selected Mint · offline</option>}{hosts.map(h => <option key={h.hostId} value={h.hostId}>{h.name}</option>)}</select></label>
        <label>Conversation<select disabled={!online} value={settings.chatId} onChange={e => setSettings(s => ({ ...s, chatId: e.target.value }))}><option value="">Choose conversation</option>{settings.chatId && !sessionExists && <option value={settings.chatId}>Selected conversation unavailable</option>}{sessions.map(s => <option key={s.chatId} value={s.chatId}>{s.title || s.chatId}</option>)}</select></label>
      </div>
      {!online && <p className="connection-note">{connectionError || 'Open Mint Desktop, CLI, or the Web backend to connect. Companion will wait here.'}</p>}
      <div className="transcript" ref={scrollRef} aria-live="polite">{turns.filter(t => t.chatId === settings.chatId && (t.text || t.prompt)).sort((a,b) => a.turnId-b.turnId).slice(-12).map(t => <article key={t.turnId}>{t.prompt && <p className="user-message"><strong>You</strong>{t.prompt}</p>}{t.text && <p><strong>Mint</strong>{t.text}</p>}</article>)}{online && !turns.some(t => t.chatId === settings.chatId && t.text) && <p className="empty-note">A little company while Mint works.</p>}</div>
      {current?.status === 'queued' || turns.some(t => t.chatId === settings.chatId && t.status === 'queued') ? <p role="status" className="connection-note">Your message is queued behind the current work.</p> : null}
      {error && <p className="error" role="alert">{error}</p>}
      <form onSubmit={e => { e.preventDefault(); send(text) }}><textarea aria-label="Message to Mint" placeholder="Talk to Mint…" value={text} maxLength={4000} disabled={!online || !sessionExists} onChange={e => setText(e.target.value)} rows={2} /><button disabled={!canSend || !text.trim()}>Send</button></form>
      <small>Chat shares this conversation. Run tools and approve tasks in Mint.</small>
    </section>}
  </main>
}
