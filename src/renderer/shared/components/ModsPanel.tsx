import { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { ChevronDown, Puzzle, Upload, X } from 'lucide-react'
import { modsPlatform, runtimePlatform } from '../platform'
import type { ModOperation, ModRecord, ModResponse, ModTestReport } from '../mods'
import { ModUi } from './ModUi'
import { ModSessionControls, ModTestResults } from './ModSessionControls'
import { modViewport } from '../utils/modViewport'
import '../css/mods.css'

export function ModsPanel({ workspacePath, chatId, launcherTarget }: { workspacePath?: string | null; chatId: string; launcherTarget?: HTMLElement | null }) {
  const [data, setData] = useState<ModResponse>({ mods: [] })
  const [open, setOpen] = useState(false)
  const [tab, setTab] = useState('manage')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)
  const [reports, setReports] = useState<Record<string, ModTestReport>>({})
  const rootRef = useRef<HTMLDivElement>(null)
  const openRef = useRef(open); openRef.current = open
  const currentScope = useRef('')
  currentScope.current = `${workspacePath}\0${chatId}`
  const [scope, setScope] = useState<'chat' | 'workspace' | 'global'>('chat')
  const generation = useRef(0)
  const importRef = useRef<HTMLInputElement>(null)
  const seen = useRef(new Set<string>())

  useEffect(() => {
    const current = ++generation.current
    let stopped = false
    let timer: ReturnType<typeof setTimeout>
    setData({ mods: [] }); setLoading(true); setError(''); setOpen(false); setTab('manage'); seen.current.clear(); setReports({})
    async function refresh() {
      if (!workspacePath || !chatId || stopped) return
      try {
        const value = await modsPlatform.operation({ workspacePath, chatId, action: 'list' })
        if (!stopped && generation.current === current) {
          setData(value); setLoading(false)
          const fresh = value.mods?.find(m => m.enabled && m.snapshot?.ui.side_panel && !seen.current.has(m.id))
          value.mods?.filter(m => m.enabled).forEach(m => seen.current.add(m.id))
          if (fresh) { setOpen(true); setTab(fresh.id) }
        }
      } catch (e) { if (!stopped && generation.current === current) setError(String(e)) }
      if (!stopped) timer = setTimeout(refresh, 1000)
    }
    void refresh()
    return () => {
      stopped = true; clearTimeout(timer)
      if (workspacePath && chatId) void modsPlatform.operation({ workspacePath, chatId, action: 'close' }).catch(() => {})
    }
  }, [workspacePath, chatId])

  useEffect(() => {
    const element=rootRef.current
    if (!element || !workspacePath || !chatId) return
    let stopped=false
    let previous=''
    let timer: ReturnType<typeof setTimeout> | undefined
    const surface=runtimePlatform.isTauriRuntime() ? 'desktop' as const : 'web' as const
    async function reportViewport() {
      if (stopped) return
      const bounds=element!.getBoundingClientRect()
      const viewport=modViewport(bounds.width,bounds.height,surface,openRef.current)
      if (!viewport) return
      const signature=JSON.stringify(viewport)
      if (signature===previous) return
      previous=signature
      try { await modsPlatform.operation({ workspacePath: workspacePath!, chatId, action: 'viewport', viewport }) }
      catch (e) { if (!stopped) setError(String(e)) }
    }
    const schedule=() => { clearTimeout(timer); timer=setTimeout(() => { void reportViewport() }, 100) }
    const observer=typeof ResizeObserver==='undefined' ? undefined : new ResizeObserver(schedule)
    observer?.observe(element); window.addEventListener('resize',schedule); schedule()
    return () => { stopped=true; clearTimeout(timer); observer?.disconnect(); window.removeEventListener('resize',schedule) }
  }, [workspacePath, chatId, open])

  const operate = useCallback(async (operation: Omit<ModOperation, 'workspacePath' | 'chatId'>) => {
    if (!workspacePath || currentScope.current !== `${workspacePath}\0${chatId}`) return
    const current = generation.current
    setBusy(true); setError('')
    try {
      const value = await modsPlatform.operation({ ...operation, workspacePath, chatId })
      if (current !== generation.current) return
      if (value.testReport && operation.id) setReports(current => ({ ...current, [operation.id!]: value.testReport! }))
      if (value.package) {
        const bytes = Uint8Array.from(atob(value.package), c => c.charCodeAt(0))
        const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }))
        const link = document.createElement('a'); link.href = url; link.download = value.filename || 'mod.mintmod'; link.click(); URL.revokeObjectURL(url)
      }
      const latest = await modsPlatform.operation({ workspacePath, chatId, action: 'list' })
      if (current === generation.current) setData(latest)
    } catch (e) { if (current === generation.current) setError(String(e)) }
    finally { if (current === generation.current) setBusy(false) }
  }, [workspacePath, chatId])

  const activate = (mod: ModRecord) => {
    const permissions = mod.manifest?.permissions || []
    if (permissions.length && !window.confirm(`Enable ${mod.name}?\nRevision: ${mod.hash}\nPermissions: ${permissions.join(', ')}\nReview the source below before approving.`)) return
    if (scope !== 'chat' && !window.confirm(`Enable ${mod.name} for ${scope === 'global' ? 'all workspaces as a personal copy' : 'all chats in this workspace'}?`)) return
    void operate({ action: 'enable', id: mod.id, scope, expectedHash: mod.hash, grantPermissions: permissions.length > 0 })
  }
  const action = (mod: ModRecord, name: string, value?: string) => void operate({ action: 'action', id: mod.id, instanceId: mod.instanceId, command: name, value })
  const mods = data.mods || []
  const active = mods.filter(m => m.enabled)
  const sidePanelMods = active.filter(m => m.snapshot?.ui.side_panel)
  const selected = mods.find(m => m.id === tab)

  const launchButton = <button type="button" className="mods-launch" disabled={!workspacePath} onClick={() => setOpen(!open)} aria-expanded={open} title={workspacePath ? 'Workspace mods' : 'Choose a workspace to use mods'}><Puzzle size={14} aria-hidden="true" /><span>Mods</span>{active.length > 0 && <span className="mods-launch-count" aria-label={`${active.length} active mods`}>{active.length}</span>}</button>
  const panelPosition = launcherTarget ? (() => {
    const bounds = launcherTarget.getBoundingClientRect()
    return {
      top: 'auto',
      right: Math.max(8, window.innerWidth - bounds.right),
      bottom: window.innerHeight - bounds.top + 10,
    }
  })() : undefined

  return <>
    <div className="mint-mods" ref={rootRef}>
      {active.map(mod => mod.snapshot?.ui.above_composer && <section key={mod.id} className="mod-band" aria-label={mod.name}>
        <ModUi node={mod.snapshot.ui.above_composer} onAction={(name, value) => action(mod, name, value)} />
      </section>)}
      {!launcherTarget && launchButton}
      {open && createPortal(<aside className="mods-panel mint-mods" aria-label="Workspace mods" style={panelPosition}>
      <header className="mods-panel-header"><span className="mods-panel-icon" aria-hidden="true"><Puzzle size={18} /></span><div className="mods-panel-title"><strong>Mods</strong><p>{mods.length} installed · {active.length} active</p></div><button type="button" className="mods-close" onClick={() => setOpen(false)} aria-label="Close mods"><X size={17} aria-hidden="true" /></button></header>
      {sidePanelMods.length > 0 && <nav aria-label="Mod panels"><button type="button" aria-pressed={tab === 'manage'} onClick={() => setTab('manage')}>Manage</button>{sidePanelMods.map(m => <button key={m.id} type="button" aria-pressed={tab === m.id} onClick={() => setTab(m.id)}>{m.name}</button>)}</nav>}
      {error && <p role="alert" className="mod-error">{error}</p>}
      {selected?.snapshot?.ui.side_panel && tab !== 'manage' ? <section className="mod-content"><ModUi node={selected.snapshot.ui.side_panel} onAction={(name, value) => action(selected, name, value)} /></section> : <div className="mod-content">
        <div className="mods-toolbar"><label htmlFor="mods-scope">Enable for <select id="mods-scope" value={scope} onChange={e => setScope(e.target.value as typeof scope)}><option value="chat">This chat</option><option value="workspace">This workspace</option><option value="global">All workspaces</option></select></label><button type="button" className="mods-import" disabled={busy} onClick={() => importRef.current?.click()}><Upload size={14} aria-hidden="true" />Import</button></div>
        <input ref={importRef} type="file" accept=".mintmod" hidden onChange={async e => {
          const file = e.target.files?.[0]; e.target.value = ''; if (!file) return
          if (file.size > 1024 * 1024) { setError('Package exceeds 1 MiB'); return }
          const bytes = new Uint8Array(await file.arrayBuffer())
          let binary = ''; for (const b of bytes) binary += String.fromCharCode(b)
          const packed = btoa(binary)
          if (currentScope.current !== `${workspacePath}\0${chatId}`) return
          try {
            const info = await modsPlatform.operation({ workspacePath: workspacePath!, chatId, action: 'inspect_package', package: packed })
            if (currentScope.current !== `${workspacePath}\0${chatId}`) return
            if (info.expectedHash && !window.confirm(`Replace source for ${info.id}? The active revision stays unchanged until you enable the imported source.`)) return
            await operate({ action: 'import', package: packed, expectedHash: info.expectedHash || undefined })
          } catch (e) { if (currentScope.current === `${workspacePath}\0${chatId}`) setError(String(e)) }
        }} />
        {loading && <p role="status" className="mods-empty">Loading mods…</p>}
        {!loading && !mods.length && <div className="mods-empty"><Puzzle size={22} aria-hidden="true" /><strong>No mods yet</strong><p>Ask Mint to create one, or import a .mintmod package. Imported mods stay off until enabled.</p></div>}
        {mods.map(mod => <article key={mod.id} className="mod-entry">
          <div className="mod-entry-heading"><div><strong>{mod.name}</strong><small>Version {mod.manifest?.version || 'unknown'}</small></div><span className={`mod-status ${mod.error ? 'is-error' : mod.enabled ? 'is-active' : ''}`}>{mod.error ? 'Error' : mod.enabled ? 'Active' : 'Off'}</span></div>
          {mod.changed && <p className="mod-hint">Source changed · active revision preserved</p>}
          {mod.reloadError && <p role="alert" className="mod-error">Reload: {mod.reloadError}</p>}
          {(mod.error || mod.sourceError) && <p role="alert" className="mod-error">{mod.error || mod.sourceError}</p>}
          <p className="mod-hint">{mod.manifest?.permissions?.join(', ') || 'UI, events and mod state'}</p>
          <div className="mod-entry-footer"><button type="button" className={`mod-primary-action ${mod.enabled && !mod.changed ? 'is-disable' : ''}`} disabled={busy || !mod.manifest} onClick={() => mod.enabled && !mod.changed ? void operate({ action: 'disable', id: mod.id }) : activate(mod)}>{mod.enabled && !mod.changed ? 'Disable' : mod.changed ? 'Validate & reload' : 'Enable'}</button>
          <details className="mod-entry-more"><summary>More options <ChevronDown size={13} aria-hidden="true" /></summary>
            <div className="mod-actions"><button type="button" disabled={busy || !mod.source} onClick={() => void operate({ action: 'export', id: mod.id })}>Export</button><button type="button" disabled={busy || !mod.source} onClick={() => void operate({ action: 'test', id: mod.id })}>Run tests</button></div>
            {reports[mod.id] && <ModTestResults report={reports[mod.id]} />}
            <details><summary>Source & diagnostics</summary><pre>{mod.source}</pre><pre>{mod.snapshot?.logs?.join('\n') || 'No log messages'}</pre><p className="mod-hint">Active revision: {mod.activeHash || 'None'}</p></details>
            {mod.revisions.length > 1 && <label className="mod-revisions">Restore source revision <select value="" disabled={busy} onChange={e => { const hash = e.target.value; if (hash && window.confirm('Restore this source revision? Review and enable it after restoring.')) void operate({ action: 'rollback', id: mod.id, hash, expectedHash: mod.hash }) }}><option value="">Choose revision</option>{mod.revisions.filter(h => h !== mod.hash).map(h => <option key={h} value={h}>{h.slice(0, 12)}</option>)}</select></label>}
          </details></div>
        </article>)}
        {data.toolRequests?.map(request => <article className="mod-entry" key={request.id}><strong>{request.modId}: {request.action}</strong><pre>{JSON.stringify(request.input, null, 2)}</pre><button type="button" disabled={busy} onClick={() => { if (window.confirm(`Run ${request.action} requested by ${request.modId}?\n${JSON.stringify(request.input, null, 2)}`)) void operate({ action: 'run_request', command: request.id, grantPermissions: true }) }}>Review & run tool</button></article>)}
        <details className="mod-session-settings"><summary>Session settings <ChevronDown size={13} aria-hidden="true" /></summary><ModSessionControls enabled={data.session?.hotReload ?? false} busy={busy} onToggle={enabled => void operate({ action: 'hot_reload', value: enabled })} /></details>
      </div>}
      </aside>, document.body)}
    </div>
    {launcherTarget && createPortal(launchButton, launcherTarget)}
  </>
}
