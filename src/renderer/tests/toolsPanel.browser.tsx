import React, { useState } from 'react'
import { createRoot } from 'react-dom/client'
import ToolSurfacePage, { type ToolSurface } from '../shared/components/ToolSurfacePage'
import ToolsPanelToggle from '../shared/components/ToolsPanelToggle'
import { ModsPanel } from '../shared/components/ModsPanel'
import { BackgroundTerminals } from '../shared/components/BackgroundTerminals'
import { installRendererPlatform, type RendererAdapter } from '../shared/platform'
import type { BackgroundJob } from '../shared/types'

// Isolated browser fixture: only the external runtime boundary is replaced.
const job: BackgroundJob = { id: 'job-1', command: 'npm run dev', cwd: '/workspace/alpha', workspacePath: '/workspace/alpha', chatId: 'chat-a', pid: 10, status: 'running', exitCode: null, error: null, startedAt: 0, endedAt: null, elapsedSeconds: 12, serverUrl: null }
const other = { ...job, id: 'job-2', workspacePath: '/workspace/beta', cwd: '/workspace/beta' }
let jobs = [job, other]
let failJobs = false
let modCloses = 0
let listCalls = 0
let outputCalls = 0
let stopCalls = 0
const enabledMod = { id: 'weather', name: 'Weather', enabled: true, hash: 'rev-1', activeHash: 'rev-1', revisions: ['rev-1'], manifest: { version: '1.0.0', permissions: [] }, snapshot: { ui: { above_composer: { type: 'text', text: 'Weather band' } } } }
let mods = [enabledMod]
installRendererPlatform({
  isTauriRuntime: () => false,
  listBackgroundJobs: async (workspace?: string) => { listCalls++; if (failJobs) throw new Error('offline'); return workspace ? jobs.filter(j => j.workspacePath === workspace) : [...jobs] },
  getBackgroundJob: async (id: string) => { outputCalls++; return { ...jobs.find(j => j.id === id), stdout: 'Server ready', stderr: '', truncated: false } },
  stopBackgroundJob: async (id: string) => { stopCalls++; jobs = jobs.map(j => j.id === id ? { ...j, status: 'stopped', exitCode: 0 } : j); return jobs.find(j => j.id === id) },
  deleteBackgroundJob: async (id: string) => { jobs = jobs.filter(j => j.id !== id) },
  modsOperation: async (op: { action: string; workspacePath: string }) => {
    if (op.action === 'close') modCloses++
    return { mods: op.workspacePath.endsWith('alpha') ? mods : [], session: { hotReload: false } }
  },
} as unknown as RendererAdapter)

function Fixture() {
  const [open, setOpen] = useState(false)
  const [surfaces, setSurfaces] = useState<ToolSurface[]>([])
  const [active, setActive] = useState<string | null>(null)
  const [workspace, setWorkspace] = useState('/workspace/alpha')
  const [count, setCount] = useState(0)
  const [modsTarget, setModsTarget] = useState<HTMLDivElement | null>(null)
  const [tasksTarget, setTasksTarget] = useState<HTMLDivElement | null>(null)
  const desktop = new URLSearchParams(location.search).has('desktop')
  const utility = (kind: 'mods' | 'background') => {
    setOpen(true); setActive(kind)
    setSurfaces(previous => previous.some(s => s.id === kind) ? previous : [...previous, { id: kind, kind, title: kind === 'mods' ? 'Mods' : 'Background tasks' }])
  }
  const noop = () => {}
  return <>
    <style>{`*{box-sizing:border-box}body{margin:0;font-family:Arial,sans-serif;--text-main:#e7ece9;--text-muted:#9aa5a0;--bg-surface:#181c1a;--panel-bg:#181c1a;--surface-bg:#222824;--surface-strong:#2c342e;--border:#38413b;--accent:#7edbb5;--accent-subtle:#253e32;background:#111613;color:var(--text-main)}.app-body{position:relative;height:calc(100dvh - 50px);display:flex}.assistant-workspace{flex:1;padding:24px;min-width:0}.fixture-header{height:50px;display:flex;align-items:center;gap:12px;padding:8px 16px}.fixture-composer{margin-top:60px}#qa-results{position:fixed;bottom:0;left:0;z-index:1000;max-width:45vw;font-size:10px;white-space:pre-wrap;pointer-events:none}`}</style>
    <header className="fixture-header"><span>Mint Tools fixture</span><ToolsPanelToggle open={open} count={count} onToggle={() => setOpen(v => !v)} className={desktop ? 'mint-titlebar-icon-button' : ''} /><button id="switch-workspace" onClick={() => setWorkspace(w => w.endsWith('alpha') ? '/workspace/beta' : '/workspace/alpha')}>Switch workspace</button></header>
    <div className={`app-body ${open ? 'tool-surface-active' : ''}`} style={{ '--tool-surface-width': '440px' } as React.CSSProperties}>
      <main className="assistant-workspace"><h1>Conversation</h1><p>{workspace}</p><div className="fixture-composer"><ModsPanel workspacePath={workspace} chatId="chat-a" panelTarget={modsTarget} onOpen={() => utility('mods')} /><textarea aria-label="Message" placeholder="Ask Mint…" /></div></main>
      {open && <ToolSurfacePage surfaces={surfaces} activeSurfaceId={active} workspacePath={workspace} terminalSize={440} desktopTools={desktop} backgroundTaskCount={count}
        onSelect={setActive} onClose={id => { setSurfaces(s => s.filter(v => v.id !== id)); setActive(null) }} onCloseAll={noop} onDuplicate={noop} onRename={noop} onCloseOthers={noop} onResizeTerminal={noop} onResizePanel={noop}
        onOpenSources={noop} onOpenTerminal={noop} onOpenBrowser={noop} onOpenReview={noop} onOpenFiles={noop} onOpenMods={() => utility('mods')} onOpenBackgroundTasks={() => utility('background')} onModsPanelTarget={setModsTarget} onBackgroundPanelTarget={setTasksTarget} onClosePanel={() => setOpen(false)} />}
      <BackgroundTerminals workspacePath={workspace} panelTarget={tasksTarget} onCountChange={setCount} />
    </div>
    <pre id="qa-results">Running…</pre>
  </>
}
createRoot(document.getElementById('root')!).render(<Fixture />)

const delay = (ms = 50) => new Promise(resolve => setTimeout(resolve, ms))
async function waitFor(check: () => boolean, label: string) {
  for (let n = 0; n < 100; n++) { if (check()) return; await delay() }
  throw new Error(label)
}
function click(selector: string) { const target = document.querySelector<HTMLElement>(selector); if (!target) throw new Error(`Missing ${selector}`); target.click() }
const results: string[] = []
const check = (condition: unknown, name: string) => { if (!condition) throw new Error(name); results.push(`PASS ${name}`) }
async function run() {
  await waitFor(() => Boolean(document.querySelector('.tools-task-count')), 'Initial task count')
  check(document.querySelector('.tools-task-count')?.textContent === '1', 'badge counts current workspace while Tools is closed')
  check(document.body.textContent?.includes('Weather band'), 'mod band renders with Tools closed')
  click('.tools-panel-toggle'); await delay()
  click('[title="Background tasks"]'); await waitFor(() => Boolean(document.querySelector('.background-terminal-job')), 'Tasks panel')
  check(document.querySelector('.tool-surface-page .background-terminal-panel'), 'tasks render inside Tools')
  check(document.querySelectorAll('.background-terminal-job').length === 1, 'workspace filter excludes other jobs')
  click('.background-terminal-actions button'); await waitFor(() => Boolean(document.querySelector('.background-terminal-log')?.textContent?.includes('Server ready')), 'Output')
  check(outputCalls > 0, 'output loads on demand')
  click('.background-terminal-stop'); await waitFor(() => stopCalls === 1 && !document.querySelector('.tools-panel-toggle .tools-task-count'), 'Stop and badge')
  check(document.querySelector('.background-terminal-state')?.textContent === 'stopped', 'Stop updates job status and badge')
  click('[aria-label="Close tools panel"]'); await delay()
  const callsBefore = listCalls; await delay(1100)
  check(listCalls > callsBefore, 'monitor keeps polling while Tools is closed')
  check(modCloses === 0, 'closing Tools keeps mod session alive')
  click('.tools-panel-toggle'); await delay()
  click('[aria-label="Switch tool"]'); await delay(); click('[title="Workspace mods"]')
  await waitFor(() => Boolean(document.querySelector('.mods-panel')), 'Mods panel')
  check(document.querySelector('.tool-surface-page .mods-panel'), 'mods render inside Tools')
  check(document.querySelector('.mods-panel')?.textContent?.includes('Weather'), 'installed mod appears in Tools')
  check(document.body.textContent?.includes('Weather band'), 'mod band survives tab switching')
  click('#switch-workspace'); await waitFor(() => Boolean(document.querySelector('.mods-empty')), 'New workspace mods')
  check(!document.querySelector('.mods-panel')?.textContent?.includes('Weather'), 'workspace switch clears previous mods')
  await waitFor(() => document.querySelector('.tools-panel-toggle .tools-task-count')?.textContent === '1', 'New workspace count')
  check(true, 'workspace switch refreshes badge')
  click('[role="tab"]'); await waitFor(() => Boolean(document.querySelector('.background-terminal-job')), 'Other workspace tasks')
  check(document.querySelector('.background-terminal-job')?.textContent?.includes('job-2'), 'tasks follow workspace switch')
  failJobs = true; await waitFor(() => Boolean(document.querySelector('.background-terminal-error')), 'Error state')
  check(document.querySelector('.background-terminal-stop')?.hasAttribute('disabled'), 'connection failure disables Stop')
  failJobs = false; await delay(1100)
  check(!document.querySelector('.background-terminal-error'), 'polling recovers after connection failure')
  // Measure the resting layout rather than an in-flight entrance transform.
  document.getAnimations().forEach(animation => animation.finish())
  await delay()
  check(document.documentElement.scrollWidth <= innerWidth, 'no page horizontal overflow')
  const filter = document.querySelector<HTMLInputElement>('.background-terminal-filter input')!
  filter.focus()
  filter.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  await delay()
  check(!document.querySelector('.tool-surface-page'), 'Escape closes Tools from portalled task content')
  document.getElementById('qa-results')!.textContent = results.join('\n') + '\nALL PASSED'
}
if (!location.search.includes('preview')) run().catch(error => { document.getElementById('qa-results')!.textContent = results.join('\n') + `\nFAIL ${error.message}` })
