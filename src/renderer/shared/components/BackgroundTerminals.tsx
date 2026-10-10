import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { ChevronDown, Terminal, Trash2, X } from 'lucide-react'
import { backgroundPlatform, runtimePlatform } from '../platform'
import type { BackgroundJob, BackgroundJobOutput } from '../types'
import './background-terminals.css'

const active = (job: BackgroundJob) => job.status === 'running' || job.status === 'stopping'
const elapsed = (seconds: number) => `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`
const workspaceName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() || path

export function BackgroundTerminals({ workspacePath }: { workspacePath?: string | null }) {
  const [jobs, setJobs] = useState<BackgroundJob[]>([])
  const [scope, setScope] = useState<{ workspace: string | null | undefined; ids: Set<string> }>({ workspace: undefined, ids: new Set() })
  const [open, setOpen] = useState(false)
  const [panelSize, setPanelSize] = useState({ width: 480, height: 600 })
  const [all, setAll] = useState(false)
  const [selected, setSelected] = useState<string | null>(null)
  const [output, setOutput] = useState<BackgroundJobOutput | null>(null)
  const [error, setError] = useState('')
  const [actionError, setActionError] = useState('')
  const [loading, setLoading] = useState(true)
  const [notice, setNotice] = useState('')
  const [follow, setFollow] = useState(true)
  const [pending, setPending] = useState<Set<string>>(new Set())
  const wrapper = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const closeButton = useRef<HTMLButtonElement>(null)
  const log = useRef<HTMLDivElement>(null)
  const statuses = useRef(new Map<string, string>())

  useEffect(() => {
    let disposed = false
    let busy = false
    setLoading(true)
    const refresh = async () => {
      if (busy || document.hidden) return
      busy = true
      try {
        const [next, scoped] = await Promise.all([backgroundPlatform.list(), workspacePath ? backgroundPlatform.list(workspacePath) : Promise.resolve([])])
        if (disposed) return
        let message = ''
        for (const job of next) {
          const previous = statuses.current.get(job.id)
          if (previous && (previous === 'running' || previous === 'stopping') && !active(job)) {
            message = `${workspaceName(job.workspacePath)} · ${job.id}: ${job.status}${job.exitCode !== null ? ` (exit ${job.exitCode})` : ''}`
          }
          statuses.current.set(job.id, job.status)
        }
        if (message) setNotice(message)
        setJobs(next)
        setScope({ workspace: workspacePath, ids: new Set(scoped.map(job => job.id)) })
        setError('')
      } catch (cause) {
        if (!disposed) {
          const message = String(cause)
          setError(`Status unavailable · ${message}`)
          if (/401:|403:/.test(message)) { setJobs([]); setOutput(null); statuses.current.clear() }
        }
      } finally { busy = false; if (!disposed) setLoading(false) }
    }
    void refresh()
    const timer = window.setInterval(() => void refresh(), 1000)
    const resume = () => { if (!document.hidden) void refresh() }
    document.addEventListener('visibilitychange', resume)
    return () => { disposed = true; window.clearInterval(timer); document.removeEventListener('visibilitychange', resume) }
  }, [workspacePath])

  useEffect(() => {
    if (!open || !selected) { setOutput(null); return }
    let disposed = false
    let busy = false
    setOutput(null)
    setActionError('')
    setFollow(true)
    const refresh = async () => {
      if (busy || document.hidden) return
      busy = true
      try {
        const next = await backgroundPlatform.output(selected)
        if (!disposed) { setOutput(next); setActionError('') }
      } catch (cause) { if (!disposed) setActionError(String(cause)) }
      finally { busy = false }
    }
    void refresh()
    const timer = window.setInterval(() => void refresh(), 1000)
    document.addEventListener('visibilitychange', refresh)
    return () => { disposed = true; window.clearInterval(timer); document.removeEventListener('visibilitychange', refresh) }
  }, [open, selected])

  useEffect(() => { if (follow && log.current) log.current.scrollTop = log.current.scrollHeight }, [output, follow])
  useEffect(() => { if (!notice) return; const timer = window.setTimeout(() => setNotice(''), 6000); return () => window.clearTimeout(timer) }, [notice])
  useEffect(() => {
    setSelected(null)
    setAll(false)
  }, [workspacePath])
  useEffect(() => {
    if (!open) return
    closeButton.current?.focus()
    const outside = (event: PointerEvent) => {
      if (!wrapper.current?.contains(event.target as Node)) setOpen(false)
    }
    document.addEventListener('pointerdown', outside)
    return () => document.removeEventListener('pointerdown', outside)
  }, [open])

  useLayoutEffect(() => {
    if (!open) return
    const composer = wrapper.current?.closest('.input-area') || wrapper.current?.parentElement
    const measure = () => {
      const rect = trigger.current?.getBoundingClientRect()
      if (!rect) return
      const width = Math.max(1, Math.min(480, (composer?.getBoundingClientRect().width || window.innerWidth) - 32, window.innerWidth - 32))
      const height = Math.max(0, Math.min(600, window.innerHeight * .65, rect.top - 12))
      setPanelSize(previous => previous.width === width && previous.height === height ? previous : { width, height })
    }
    measure()
    const observer = new ResizeObserver(measure)
    if (composer) observer.observe(composer)
    if (wrapper.current) observer.observe(wrapper.current)
    window.addEventListener('resize', measure)
    return () => { observer.disconnect(); window.removeEventListener('resize', measure) }
  }, [open])

  const current = jobs.filter(job => scope.workspace === workspacePath && scope.ids.has(job.id))
  const count = current.filter(active).length
  const visible = all ? jobs : current
  const close = () => { setOpen(false); trigger.current?.focus() }
  const openServer = async (url: string) => {
    setActionError('')
    try {
      if (runtimePlatform.isTauriRuntime()) {
        const { invoke } = await import('@tauri-apps/api/core')
        await invoke('run_desktop_action', { action: { type: 'open_url', target: url } })
      } else {
        const opened = window.open(url, '_blank', 'noopener,noreferrer')
        if (!opened) throw new Error('The browser blocked the new tab')
      }
    } catch (cause) {
      setActionError(`Could not open browser: ${String(cause)}`)
    }
  }
  const stop = async (job: BackgroundJob) => {
    setPending(previous => new Set(previous).add(job.id))
    setActionError('')
    try {
      const next = await backgroundPlatform.stop(job.id)
      setJobs(previous => previous.map(item => item.id === job.id ? next : item))
    } catch (cause) { setActionError(String(cause)) }
    finally { setPending(previous => { const next = new Set(previous); next.delete(job.id); return next }) }
  }
  const remove = async (job: BackgroundJob) => {
    setPending(previous => new Set(previous).add(job.id))
    setActionError('')
    try {
      await backgroundPlatform.delete(job.id)
      setJobs(previous => previous.filter(item => item.id !== job.id))
      if (selected === job.id) setSelected(null)
      statuses.current.delete(job.id)
    } catch (cause) { setActionError(String(cause)) }
    finally { setPending(previous => { const next = new Set(previous); next.delete(job.id); return next }) }
  }
  const canOpen = Boolean(runtimePlatform.isTauriRuntime()) || ['localhost', '127.0.0.1', '[::1]'].includes(window.location.hostname)

  return <div className="background-terminals" ref={wrapper} onKeyDown={event => {
    if (event.key === 'Escape' && open) { event.preventDefault(); event.stopPropagation(); close() }
  }}>
    <button type="button" className={`background-terminal-trigger ${count ? 'has-active' : ''}`} ref={trigger}
      aria-expanded={open} aria-controls="background-terminal-panel" onClick={() => setOpen(previous => !previous)}>
      <Terminal className="background-terminal-trigger-icon" size={14} aria-hidden="true" />
      <span>{error ? 'Status unavailable' : loading ? 'Terminals…' : 'Terminals'}</span>
      {count > 0 && <span className="background-terminal-count" aria-label={`${count} active background terminals`}>{count}</span>}
      <ChevronDown className={`background-terminal-chevron ${open ? 'is-open' : ''}`} size={13} aria-hidden="true" />
    </button>
    {notice && <span className="background-terminal-notice" role="status">{notice}</span>}
    {open && <section id="background-terminal-panel" className="background-terminal-panel" aria-label="Background terminals" style={{ width: panelSize.width, maxHeight: panelSize.height }}>
      <header className="background-terminal-panel-header">
        <span className="background-terminal-heading-icon" aria-hidden="true">›_</span>
        <span className="background-terminal-panel-title">
          <strong>Background terminals</strong>
          <small>{all ? 'All workspaces' : workspaceName(workspacePath || '') || 'No workspace'}</small>
        </span>
        <button ref={closeButton} type="button" onClick={close} aria-label="Close background terminals">×</button>
      </header>
      <label className="background-terminal-filter"><input type="checkbox" checked={all} onChange={event => { setAll(event.target.checked); setSelected(null) }} /> All workspaces</label>
      {error && <p role="status" className="background-terminal-error">{error}</p>}
      {actionError && <p role="alert" className="background-terminal-error">{actionError}</p>}
      <div className="background-terminal-list">
        {!visible.length && <div className="background-terminal-empty" role="status">
          <span aria-hidden="true">›_</span>
          <strong>{loading ? 'Loading terminals…' : error ? 'Waiting for connection…' : 'No background terminals'}</strong>
          {!loading && !error && <small>Commands started in the background will appear here.</small>}
        </div>}
        {[...visible].reverse().map(job => <article key={job.id} className="background-terminal-job">
          <div className="background-terminal-job-heading"><strong>{job.id}</strong><span className={`background-terminal-state ${job.status}`}>{job.status}</span><span>{elapsed(job.elapsedSeconds)}</span></div>
          <code title={job.command}>{job.command}</code>
          <small>{workspaceName(job.workspacePath)} · {job.cwd}{job.exitCode !== null ? ` · exit ${job.exitCode}` : ''}</small>
          {job.error && <small className="background-terminal-error">{job.error}</small>}
          <div className="background-terminal-actions">
            <button type="button" onClick={() => { setSelected(job.id); setFollow(true) }}>View output</button>
            {job.serverUrl && (canOpen ? <button type="button" onClick={() => void openServer(job.serverUrl!)}>Open browser</button> : <small>Backend-local address: {job.serverUrl} · unavailable from this device</small>)}
            {!active(job) && <button type="button" className="background-terminal-delete" aria-label={`Remove terminal ${job.id}`} title="Remove terminal" disabled={pending.has(job.id)} onClick={() => void remove(job)}><Trash2 size={14} aria-hidden="true" /></button>}
            {active(job) && <button type="button" className="background-terminal-stop" disabled={Boolean(error) || pending.has(job.id) || job.status === 'stopping'} onClick={() => void stop(job)}>{job.status === 'stopping' || pending.has(job.id) ? 'Stopping…' : 'Stop'}</button>}
          </div>
        </article>)}
      </div>
      {selected && <section aria-label={`Output for ${selected}`} className="background-terminal-output">
        <header><strong>{selected} · output</strong><label><input type="checkbox" checked={follow} onChange={event => setFollow(event.target.checked)} /> Follow</label><button type="button" aria-label="Close terminal output" title="Close output" onClick={() => setSelected(null)}><X size={14} aria-hidden="true" /></button></header>
        {output?.truncated && <small>Older output was truncated.</small>}
        <div ref={log} className="background-terminal-log" onWheel={event => { if (event.deltaY < 0) setFollow(false) }} onKeyDown={event => { if (['PageUp', 'ArrowUp', 'Home'].includes(event.key)) setFollow(false) }} onScroll={event => {
          const element = event.currentTarget
          if (element.scrollHeight - element.scrollTop - element.clientHeight > 30) setFollow(false)
        }} tabIndex={0}>
          {output ? <><strong>stdout</strong><pre>{output.stdout || '(empty)'}</pre><strong>stderr</strong><pre>{output.stderr || '(empty)'}</pre></> : <p>{actionError ? 'Output unavailable.' : 'Loading output…'}</p>}
        </div>
      </section>}
    </section>}
  </div>
}
