import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Trash2, X } from 'lucide-react'
import { backgroundPlatform, runtimePlatform } from '../../platform'
import type { BackgroundJob, BackgroundJobOutput } from '../../types'
import './background-terminals.css'

const active = (job: BackgroundJob) => job.status === 'running' || job.status === 'stopping'
const elapsed = (seconds: number) => `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`
const workspaceName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() || path

export const filterScopedJobs = (jobs: BackgroundJob[], workspacePath?: string | null): BackgroundJob[] =>
  workspacePath ? jobs.filter(job => job.workspacePath === workspacePath) : []

export const backgroundPollIntervalMs = (open: boolean, jobs: BackgroundJob[]): number =>
  open || jobs.some(active) ? 3000 : 10000

export function BackgroundTerminals({ workspacePath, panelTarget, onCountChange }: {
  workspacePath?: string | null; panelTarget: HTMLElement | null; onCountChange: (count: number) => void
}) {
  const [jobs, setJobs] = useState<BackgroundJob[]>([])
  const [scope, setScope] = useState<{ workspace: string | null | undefined; ids: Set<string> }>({ workspace: undefined, ids: new Set() })
  const open = Boolean(panelTarget)
  const openRef = useRef(open)
  openRef.current = open
  const jobsRef = useRef(jobs)
  jobsRef.current = jobs
  const [all, setAll] = useState(false)
  const [selected, setSelected] = useState<string | null>(null)
  const [output, setOutput] = useState<BackgroundJobOutput | null>(null)
  const [error, setError] = useState('')
  const [actionError, setActionError] = useState('')
  const [loading, setLoading] = useState(true)
  const [notice, setNotice] = useState('')
  const [follow, setFollow] = useState(true)
  const [pending, setPending] = useState<Set<string>>(new Set())
  const log = useRef<HTMLDivElement>(null)
  const statuses = useRef(new Map<string, string>())

  useEffect(() => {
    let disposed = false
    let busy = false
    let timer: ReturnType<typeof window.setTimeout> | undefined
    setLoading(true)
    const scheduleNext = (latestJobs: BackgroundJob[]) => {
      if (disposed) return
      if (timer !== undefined) window.clearTimeout(timer)
      timer = window.setTimeout(() => void refresh(), backgroundPollIntervalMs(openRef.current, latestJobs))
    }
    const refresh = async () => {
      if (busy || document.hidden) {
        scheduleNext(jobsRef.current)
        return
      }
      busy = true
      let latestJobs = jobsRef.current
      try {
        const next = await backgroundPlatform.list()
        if (disposed) return
        latestJobs = next
        const scoped = filterScopedJobs(next, workspacePath)
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
          if (/401:|403:/.test(message)) { latestJobs = []; setJobs([]); setOutput(null); statuses.current.clear() }
        }
      } finally {
        busy = false
        if (!disposed) {
          setLoading(false)
          scheduleNext(latestJobs)
        }
      }
    }
    void refresh()
    const resume = () => { if (!document.hidden) void refresh() }
    document.addEventListener('visibilitychange', resume)
    return () => {
      disposed = true
      if (timer !== undefined) window.clearTimeout(timer)
      document.removeEventListener('visibilitychange', resume)
    }
  }, [workspacePath, open])

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

  const current = jobs.filter(job => scope.workspace === workspacePath && scope.ids.has(job.id))
  const count = current.filter(active).length
  useEffect(() => { onCountChange(count) }, [count, onCountChange])
  const visible = all ? jobs : current
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

  return <>
    {notice && <span className="background-task-announcement" role="status">{notice}</span>}
    {panelTarget && createPortal(<section className="background-terminals background-terminal-panel" aria-label="Background tasks">
      <header className="background-terminal-panel-header">
        <span className="background-terminal-heading-icon" aria-hidden="true">›_</span>
        <span className="background-terminal-panel-title">
          <strong>Background tasks</strong>
          <small>{all ? 'All workspaces' : workspaceName(workspacePath || '') || 'No workspace'}</small>
        </span>
      </header>
      {notice && <p>{notice}</p>}
      <label className="background-terminal-filter"><input type="checkbox" checked={all} onChange={event => { setAll(event.target.checked); setSelected(null) }} /> All workspaces</label>
      {error && <p role="status" className="background-terminal-error">{error}</p>}
      {actionError && <p role="alert" className="background-terminal-error">{actionError}</p>}
      <div className="background-terminal-list">
        {!visible.length && <div className="background-terminal-empty" role="status">
          <span aria-hidden="true">›_</span>
          <strong>{loading ? 'Loading tasks…' : error ? 'Waiting for connection…' : 'No background tasks'}</strong>
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
    </section>, panelTarget)}
  </>
}
