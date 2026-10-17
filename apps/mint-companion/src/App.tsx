import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import Live2DStage from './Live2DStage'
import { activeTurn, applyTurn, characterStatus, modelExpression, terminal, type Event, type Host, type Session, type Turn } from './state'
import { radialMenuOrigin } from './radial-layout'

type Settings = { hostId: string; chatId: string; scale: number; expression: number; accessory: number; locked: boolean; guide: boolean; alwaysOnTop: boolean; normalWindow: boolean; panelOpen: boolean }
const defaults: Settings = { hostId: '', chatId: '', scale: 1, expression: -1, accessory: 0, locked: false, guide: false, alwaysOnTop: true, normalWindow: false, panelOpen: true }
const expressions = ['Auto', 'Default', 'Dazed', 'Dazed eyes', 'Photo', 'Poke', 'Cat filter']
const accessories = ['None', 'Apron', 'Glasses', 'Pen']
const areas = ['head', 'cheek', 'left hand', 'right hand', 'body', 'lower body']
const interactionRadialPositions = [
  { x: 101, y: 158 }, { x: 64, y: 222 }, { x: -10, y: 222 },
  { x: -47, y: 158 }, { x: -10, y: 94 }, { x: 64, y: 94 },
]

function RadialAction({ label, icon, onClick, disabled = false, expanded = false, ariaExpanded, className = '', style }: {
  label: string
  icon: string
  onClick: () => void
  disabled?: boolean
  expanded?: boolean
  ariaExpanded?: boolean
  className?: string
  style?: CSSProperties
}) {
  return <button type="button" aria-label={label} aria-expanded={ariaExpanded} title={label} disabled={disabled} onClick={onClick} style={style} className={`radial-menu-button ${expanded ? 'radial-menu-button-expanded' : ''} ${className}`}>
    <span aria-hidden="true">{icon}</span>
    <span aria-hidden="true" className="radial-menu-tooltip glass-panel">{label}</span>
  </button>
}

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
  const [sending, setSending] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [actionsOpen, setActionsOpen] = useState(false)
  const [expandedActionGroup, setExpandedActionGroup] = useState<string | null>(null)
  const [menuOrigin, setMenuOrigin] = useState({ left: 0, top: 0 })
  const menuButtonRef = useRef<HTMLButtonElement>(null)
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

  useLayoutEffect(() => {
    if (!actionsOpen) return
    const placeMenu = () => {
      const button = menuButtonRef.current
      if (!button) return
      const bounds = button.getBoundingClientRect()
      setMenuOrigin(radialMenuOrigin({ x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 }, window.innerWidth, window.innerHeight))
    }
    placeMenu()
    window.addEventListener('resize', placeMenu)
    return () => window.removeEventListener('resize', placeMenu)
  }, [actionsOpen])

  useEffect(() => {
    if (!actionsOpen) return
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { setActionsOpen(false); setExpandedActionGroup(null) }
    }
    document.addEventListener('keydown', closeOnEscape)
    return () => document.removeEventListener('keydown', closeOnEscape)
  }, [actionsOpen])

  function toggleActionGroup(group: string) {
    setExpandedActionGroup(current => current === group ? null : group)
  }

  function toggleActionMenu() {
    if (!actionsOpen && menuButtonRef.current) {
      const bounds = menuButtonRef.current.getBoundingClientRect()
      setMenuOrigin(radialMenuOrigin({ x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 }, window.innerWidth, window.innerHeight))
    }
    setActionsOpen(value => !value)
    setExpandedActionGroup(null)
  }

  function closeActionMenu() {
    setActionsOpen(false)
    setExpandedActionGroup(null)
  }

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
  async function interact(area: string) {
    if (!canSend) return
    setSending(true); setError('')
    try {
      await invoke('send_command', { command: { type: 'interact', chatId: settings.chatId, requestId: crypto.randomUUID(), area } })
    } catch (reason) { setError(`Message could not be submitted: ${String(reason)}. It will not be retried automatically.`) }
    finally { setSending(false) }
  }
  const automaticStatus = characterStatus(feedback || !current || !terminal(current.status) ? current?.status ?? 'idle' : 'idle', online, sessionExists)
  return <main className={`relative isolate flex h-dvh min-h-0 flex-col gap-3 overflow-hidden p-3 text-ice-50 sm:gap-4 sm:p-4 ${settings.normalWindow ? 'normal-window' : ''}`}>
    <div aria-hidden="true" className="companion-noise pointer-events-none absolute inset-0 -z-10 overflow-hidden rounded-[28px]" />
    <header className="relative z-20 flex h-12 w-full shrink-0 items-center justify-end" onPointerDown={event => { if (event.button === 0 && event.target === event.currentTarget) getCurrentWindow().startDragging() }}>
      <div className="relative shrink-0">
        <button ref={menuButtonRef} aria-label="Companion action tree" aria-expanded={actionsOpen} aria-controls="companion-action-tree" title="Companion menu" onClick={toggleActionMenu} style={actionsOpen ? { position: 'fixed', left: `${menuOrigin.left + 86}px`, top: `${menuOrigin.top + 86}px` } : undefined} className={`glass-panel group grid size-12 place-items-center rounded-full border text-xl transition duration-300 hover:-translate-y-0.5 hover:border-ice-200/45 hover:bg-ice-200/15 ${actionsOpen ? 'z-30 border-ice-200/40 bg-ice-200/15 text-white' : 'border-white/10 bg-black/10 text-ice-100/80'}`}>
          <span className={`text-xl transition-transform duration-300 ${actionsOpen ? 'rotate-0' : 'group-hover:rotate-12'}`}>{actionsOpen ? '×' : '☷'}</span>
        </button>
        {actionsOpen && <nav id="companion-action-tree" aria-label="Companion actions" style={{ left: `${menuOrigin.left}px`, top: `${menuOrigin.top}px` }} className="pointer-events-none fixed z-20 size-[220px]">
          <svg aria-hidden="true" viewBox="0 0 220 220" className="absolute inset-0 h-full w-full overflow-visible text-ice-200/45">
            {(!expandedActionGroup || expandedActionGroup === 'conversation') && <path d="M110 110 L14 110" pathLength="1" className="radial-connector animate-[connector-in_420ms_ease-out_both]" style={{ animationDelay: '20ms' }} />}
            {(!expandedActionGroup || expandedActionGroup === 'companion') && <path d="M110 110 L27 158" pathLength="1" className="radial-connector animate-[connector-in_420ms_ease-out_both]" style={{ animationDelay: '65ms' }} />}
            {(!expandedActionGroup || expandedActionGroup === 'appearance') && <path d="M110 110 L62 193" pathLength="1" className="radial-connector animate-[connector-in_420ms_ease-out_both]" style={{ animationDelay: '110ms' }} />}
            {(!expandedActionGroup || expandedActionGroup === 'window') && <path d="M110 110 L110 206" pathLength="1" className="radial-connector animate-[connector-in_420ms_ease-out_both]" style={{ animationDelay: '155ms' }} />}
            {expandedActionGroup === 'conversation' && <path d="M14 132 V148" pathLength="1" className="radial-connector animate-[connector-in_300ms_ease-out_both]" />}
            {expandedActionGroup === 'companion' && interactionRadialPositions.map(({ x, y }, index) => <path key={`interaction-spoke-${index}`} d={`M27 158 L${x} ${y}`} pathLength="1" className="radial-connector animate-[connector-in_420ms_ease-out_both]" style={{ animationDelay: `${index * 35}ms` }} />)}
            {expandedActionGroup === 'appearance' && <path d="M62 215 V233" pathLength="1" className="radial-connector animate-[connector-in_300ms_ease-out_both]" />}
            {expandedActionGroup === 'window' && <path d="M110 228 V236 L78 244 M110 236 L130 244" pathLength="1" className="radial-connector animate-[connector-in_300ms_ease-out_both]" />}
          </svg>
          {(!expandedActionGroup || expandedActionGroup === 'conversation') && <div className="pointer-events-auto absolute z-10 animate-[bubble-in_360ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: '180px', top: '84px', animationDelay: '35ms' }}>
            <RadialAction label="Conversation" icon="◌" expanded={expandedActionGroup === 'conversation'} ariaExpanded={expandedActionGroup === 'conversation'} onClick={() => toggleActionGroup('conversation')} />
          </div>}
          {expandedActionGroup === 'conversation' && <div className="pointer-events-auto absolute z-20 animate-[bubble-in_320ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: '184px', top: '148px' }}><RadialAction label={settings.panelOpen ? 'Hide chat panel' : 'Show chat panel'} icon="▤" onClick={() => { setSettings(s => ({ ...s, panelOpen: !s.panelOpen })); closeActionMenu() }} /></div>}

          {(!expandedActionGroup || expandedActionGroup === 'companion') && <div className="pointer-events-auto absolute z-10 animate-[bubble-in_360ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: '167px', top: '132px', animationDelay: '80ms' }}>
            <RadialAction label="Interact with Mint" icon="✦" expanded={expandedActionGroup === 'companion'} ariaExpanded={expandedActionGroup === 'companion'} onClick={() => toggleActionGroup('companion')} />
          </div>}
          {expandedActionGroup === 'companion' && areas.map((area, index) => {
            const { x, y } = interactionRadialPositions[index]
            return <div key={area} className="pointer-events-auto absolute z-20 animate-[bubble-in_320ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: `${220 - x - 26}px`, top: `${y - 26}px`, animationDelay: `${index * 35}ms` }}><RadialAction label={`Interact with ${area}`} icon={['✧', '♡', '◀', '▶', '♢', '▽'][index]} disabled={!canSend || settings.locked} onClick={() => { interact(area); closeActionMenu() }} /></div>
          })}

          {(!expandedActionGroup || expandedActionGroup === 'appearance') && <div className="pointer-events-auto absolute z-10 animate-[bubble-in_360ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: '132px', top: '167px', animationDelay: '125ms' }}>
            <RadialAction label="Appearance" icon="⚙" expanded={expandedActionGroup === 'appearance'} ariaExpanded={expandedActionGroup === 'appearance'} onClick={() => toggleActionGroup('appearance')} />
          </div>}
          {expandedActionGroup === 'appearance' && <div className="pointer-events-auto absolute z-20 animate-[bubble-in_320ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: '136px', top: '233px' }}><RadialAction label={settingsOpen ? 'Close settings' : 'Open settings'} icon="⚙" onClick={() => { setSettingsOpen(value => !value); closeActionMenu() }} /></div>}

          {(!expandedActionGroup || expandedActionGroup === 'window') && <div className="pointer-events-auto absolute z-10 animate-[bubble-in_360ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: '84px', top: '180px', animationDelay: '170ms' }}>
            <RadialAction label="Window" icon="▢" expanded={expandedActionGroup === 'window'} ariaExpanded={expandedActionGroup === 'window'} onClick={() => toggleActionGroup('window')} />
          </div>}
          {expandedActionGroup === 'window' && <>
            <div className="pointer-events-auto absolute z-20 animate-[bubble-in_320ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: '120px', top: '244px' }}><RadialAction label="Minimize Companion" icon="−" onClick={() => { closeActionMenu(); getCurrentWindow().minimize().catch(reason => setError(String(reason))) }} /></div>
            <div className="pointer-events-auto absolute z-20 animate-[bubble-in_320ms_cubic-bezier(.16,1,.3,1)_both]" style={{ right: '68px', top: '244px', animationDelay: '45ms' }}><RadialAction label="Quit Companion" icon="×" onClick={() => invoke('quit_app')} /></div>
          </>}
        </nav>}
      </div>
    </header>

    <div className="relative min-h-0 flex-1">
      <div aria-hidden="true" className="orbit-ring pointer-events-none absolute left-1/2 top-[44%] aspect-square w-[min(72vw,420px)] -translate-x-1/2 -translate-y-1/2 rounded-full" />
      <div aria-hidden="true" className="orbit-ring pointer-events-none absolute left-1/2 top-[44%] aspect-square w-[min(62vw,360px)] -translate-x-1/2 -translate-y-1/2 rounded-full border-dashed opacity-70 [animation:orbit_36s_linear_infinite]" />
      <div aria-hidden="true" className="pointer-events-none absolute left-1/2 top-[43%] size-[min(62vw,360px)] -translate-x-1/2 -translate-y-1/2 rounded-full bg-sky-300/10 blur-3xl animate-glow" />
      <span aria-hidden="true" className="absolute left-[18%] top-[27%] size-1.5 rounded-full bg-ice-100 shadow-[0_0_14px_3px_rgba(130,215,255,.65)] animate-twinkle" />
      <span aria-hidden="true" className="absolute right-[17%] top-[54%] size-1 rounded-full bg-white shadow-[0_0_12px_3px_rgba(130,215,255,.7)] [animation:twinkle_2.7s_ease-in-out_1s_infinite]" />
      <Live2DStage key={modelKey} scale={settings.scale} expressionIndex={modelExpression(settings.expression, automaticStatus)} accessoryIndex={settings.accessory} isLocked={settings.locked} isActive={visible} status={automaticStatus} onLoadError={setModelError} />
      {modelError && <div className="glass-panel absolute inset-x-4 top-[16%] rounded-2xl p-4 text-sm leading-relaxed text-rose-100" role="alert">The character could not load. Chat and settings are available.<button className="mt-3 block min-h-10 rounded-lg bg-white/10 px-4 py-2.5 text-sm" onClick={() => { setModelError(''); setModelKey(k => k + 1) }}>Retry character</button></div>}
    </div>

    {settingsOpen && <section className="glass-panel animate-enter absolute right-3 top-[4.8rem] z-30 grid max-h-[48dvh] w-[min(340px,calc(100vw-24px))] grid-cols-2 gap-x-3 gap-y-4 overflow-auto rounded-2xl p-4 sm:right-4" aria-label="Companion settings">
      <div className="col-span-2 flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold text-white">Companion settings <span className="ml-1 text-ice-200">✦</span></h2>
        <button type="button" aria-label="Close settings" title="Close settings" onClick={() => setSettingsOpen(false)} className="grid size-9 shrink-0 place-items-center rounded-full border border-white/15 bg-white/5 text-base text-ice-100/75 transition hover:border-ice-200/45 hover:bg-ice-200/10 hover:text-white">×</button>
      </div>
      <label className="col-span-2 flex flex-col gap-2 text-[11px] text-ice-100/70">Character size<input className="accent-sky-300" type="range" min="0.5" max="1.5" step="0.05" value={settings.scale} onChange={e => setSettings(s => ({ ...s, scale: +e.target.value }))} /></label>
      <label className="flex min-w-0 flex-col gap-1.5 text-[11px] text-ice-100/70">Expression<select className="min-w-0 rounded-xl border border-white/15 bg-night-950/75 px-2.5 py-2 text-xs text-white" value={settings.expression} onChange={e => setSettings(s => ({ ...s, expression: +e.target.value }))}>{expressions.map((name, i) => <option key={name} value={i-1}>{name}</option>)}</select></label>
      <label className="flex min-w-0 flex-col gap-1.5 text-[11px] text-ice-100/70">Accessory<select className="min-w-0 rounded-xl border border-white/15 bg-night-950/75 px-2.5 py-2 text-xs text-white" value={settings.accessory} onChange={e => setSettings(s => ({ ...s, accessory: +e.target.value }))}>{accessories.map((name, i) => <option key={name} value={i}>{name}</option>)}</select></label>
      {([['locked', 'Lock interactions'], ['alwaysOnTop', 'Always on top'], ['normalWindow', 'Normal window']] as const).map(([key, label]) => <label key={key} className="flex items-center gap-2 text-[11px] text-ice-100/75"><input className="accent-sky-300" type="checkbox" checked={settings[key]} onChange={e => setSettings(s => ({ ...s, [key]: e.target.checked }))} />{label}</label>)}
    </section>}

    {settings.panelOpen && <section className="glass-panel animate-enter relative z-10 flex max-h-[43dvh] shrink-0 flex-col overflow-hidden rounded-[22px] p-3.5 sm:p-4" aria-label="Chat with Mint">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div className="min-w-0"><h1 className="truncate text-sm font-semibold tracking-wide text-white">A little company</h1><p className="mt-0.5 text-[10px] text-ice-100/55">Your conversation, shared with Mint</p></div>
        <span className={`flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] ${online ? 'border-emerald-200/20 bg-emerald-200/10 text-emerald-100' : 'border-white/10 bg-black/10 text-ice-100/65'}`}><span className={`size-1.5 rounded-full ${online ? 'bg-emerald-300 animate-pulse' : 'bg-slate-400'}`} />{online ? 'Connected' : 'Waiting for Mint'}</span>
      </div>
      <div className="grid shrink-0 grid-cols-2 gap-2.5">
        <label className="flex min-w-0 flex-col gap-1.5 text-[10px] font-medium uppercase tracking-[.13em] text-ice-100/55">Mint instance<select className="w-full min-w-0 rounded-xl border border-white/15 bg-night-950/65 px-2.5 py-2.5 text-xs normal-case tracking-normal text-white transition hover:border-ice-200/40 disabled:opacity-50" value={settings.hostId} onChange={e => setSettings(s => ({ ...s, hostId: e.target.value, chatId: '' }))}><option value="">Choose Mint</option>{settings.hostId && !hosts.some(h => h.hostId === settings.hostId) && <option value={settings.hostId}>Selected Mint · offline</option>}{hosts.map(h => <option key={h.hostId} value={h.hostId}>{h.name}</option>)}</select></label>
        <label className="flex min-w-0 flex-col gap-1.5 text-[10px] font-medium uppercase tracking-[.13em] text-ice-100/55">Conversation<select className="w-full min-w-0 rounded-xl border border-white/15 bg-night-950/65 px-2.5 py-2.5 text-xs normal-case tracking-normal text-white transition hover:border-ice-200/40 disabled:opacity-45" disabled={!online} value={settings.chatId} onChange={e => setSettings(s => ({ ...s, chatId: e.target.value }))}><option value="">Choose conversation</option>{settings.chatId && !sessionExists && <option value={settings.chatId}>Selected conversation unavailable</option>}{sessions.map(s => <option key={s.chatId} value={s.chatId}>{s.title || s.chatId}</option>)}</select></label>
      </div>
      {!online && <p className="my-2.5 text-[11px] leading-relaxed text-ice-100/60">{connectionError || 'Open Mint Desktop, CLI, or Web to connect. I’ll wait right here.'}</p>}
      <div className="chat-scrollbar my-1 min-h-0 flex-1 overflow-y-auto text-xs leading-relaxed" ref={scrollRef} aria-live="polite">{turns.filter(t => t.chatId === settings.chatId && (t.text || t.prompt)).sort((a,b) => a.turnId-b.turnId).slice(-12).map(t => <article key={t.turnId} className="animate-enter border-t border-white/[.07] py-2 first:border-0">{t.prompt && <p className="mb-2 text-right text-ice-100/70"><strong className="mb-0.5 block text-[9px] font-medium uppercase tracking-[.17em] text-ice-200/65">You</strong>{t.prompt}</p>}{t.text && <p className="text-white/90"><strong className="mb-0.5 block text-[9px] font-medium uppercase tracking-[.17em] text-ice-200">Mint <span aria-hidden="true">✦</span></strong>{t.text}</p>}</article>)}{online && !turns.some(t => t.chatId === settings.chatId && t.text) && <p className="py-2 text-center text-[11px] text-ice-100/60">A little company while Mint works <span className="inline-block animate-pulse text-ice-200">✦</span></p>}</div>
      {current?.status === 'queued' || turns.some(t => t.chatId === settings.chatId && t.status === 'queued') ? <p role="status" className="mb-2 text-[10px] text-ice-100/65">Your message is queued behind the current work.</p> : null}
      {error && <p className="mb-2 text-[11px] text-rose-200" role="alert">{error}</p>}
      <p className="mt-2.5 text-center text-[9px] tracking-wide text-ice-100/45">Chat history is shared with Mint · send messages from Desktop or CLI</p>
    </section>}
  </main>
}
