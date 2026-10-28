import React, { useState } from 'react'
import { createRoot } from 'react-dom/client'
import ToolSurfacePage from '../shared/components/workspace/ToolSurfacePage'
import ToolsPanelToggle from '../shared/components/workspace/ToolsPanelToggle'
import { useSessionSources } from '../shared/utils/useSessionSources'
import SessionSources from '../shared/components/workspace/SessionSources'
import { installRendererPlatform, installWorkspacePlatform, type RendererAdapter, type WorkspacePlatform } from '../shared/platform'
import type { ConversationSources } from '../shared/types'
import { sourceAttachments } from '../shared/utils/sourceAttachments'
import '../src/css/desktop-tools.css'

const canvas = document.createElement('canvas'); canvas.width = 640; canvas.height = 400
const context = canvas.getContext('2d')!; context.fillStyle = '#256853'; context.fillRect(0, 0, 640, 400); context.fillStyle = '#dcfce7'; context.font = '36px sans-serif'; context.fillText('Test image', 200, 210)
const originalImage = canvas.toDataURL('image/png')
const preview = await sourceAttachments({ imageAttachments: [{ name: 'reference.png', dataUri: originalImage }] })
const data: ConversationSources = {
  attachments: [
    { ...preview[0], id: 1, interactionId: 2, libraryItemId: 'reference-item' },
    ...['Math-Data-Science-Exam.pdf', '01_slr_model_assumptions.md', '02_least_squares_estimation.md', '03_coefficient_accuracy.md', '04_r_squared_rse.md', '05_statistic.md'].map((name, index) => ({ id: index + 2, interactionId: 4 + index, name, kind: 'file' as const, mimeType: name.endsWith('.pdf') ? 'application/pdf' : 'text/markdown', thumbnailDataUri: null })),
  ],
  webSearchProgress: [{ type: 'ToolEnd', data: { action: 'web_search', input: { query: 'linear regression' }, result: '1. Linear regression reference\nURL: https://example.com/regression\nReference material\n\n2. Least squares guide\nURL: https://docs.example.com/least-squares\nA guide' } }],
}
let fail = false
let deferA = false
let resolveA: ((value: ConversationSources) => void) | null = null
installRendererPlatform({ getConversationSources: async (chatId: string) => {
  if (fail) throw new Error('Offline')
  if (chatId === 'A' && deferA) return new Promise<ConversationSources>(resolve => { resolveA = resolve })
  return chatId === 'A' ? data : { attachments: [], webSearchProgress: [] }
} } as unknown as RendererAdapter)
installWorkspacePlatform({ getGitBranchInfo: async () => ({ isRepository: true, currentBranch: 'Rust', isDirty: true, branches: [], remoteBranches: [], detachedHead: null }) } as unknown as WorkspacePlatform)

function Fixture() {
  const [chatId, setChatId] = useState('A')
  const [open, setOpen] = useState(false)
  const [selected, setSelected] = useState(false)
  const session = useSessionSources(chatId, [], open)
  const noop = () => {}
  const close = () => { setOpen(false); requestAnimationFrame(() => document.querySelector<HTMLButtonElement>('.tools-panel-toggle')?.focus()) }
  const desktop = location.pathname.includes('sessionSources')

  return <>
    <style>{`*{box-sizing:border-box}html,body,#root{width:100%;height:100%;overflow:hidden}.assistant-workspace{flex:1;min-width:0}body{margin:0;background:#111113;color:#e8eaed;font-family:Arial,sans-serif;--text-main:#e8eaed;--text-secondary:#c5cad1;--text-muted:#8b939f;--bg-overlay:#111113;--surface-strong:#25262b;--border:#33353b;--accent:#7edbb5;--border-subtle:#27292d;--bg-surface-soft:#1a1b20;--bg-surface-hover:#22242a}.fixture-bar{display:flex;align-items:center;justify-content:flex-end;gap:12px;height:50px;padding:8px 16px;border-bottom:1px solid #33353b}#qa-results{position:fixed;bottom:6px;left:8px;max-width:calc(100vw - 16px);font-size:10px;white-space:pre-wrap;pointer-events:none}`}</style>
    <header className="fixture-bar"><span>Session sources fixture</span><button id="switch-chat" onClick={() => setChatId(id => id === 'A' ? 'B' : 'A')}>Switch session</button><ToolsPanelToggle open={open} count={0} onToggle={() => setOpen(v => !v)} /></header>
    <div className={`app-body ${open ? 'tool-surface-active' : ''}`} style={{ position: 'relative', height: 'calc(100dvh - 50px)', display: 'flex' }}>
      <main className="assistant-workspace"><h1>Conversation</h1><textarea aria-label="Message" defaultValue="Keep my draft" /></main>
      {open && <ToolSurfacePage surfaces={selected ? [{ id: 'sources', kind: 'sources', title: 'Sources' }] : []} activeSurfaceId={selected ? 'sources' : null} terminalSize={440} desktopTools={desktop}
        sourceCount={session.sources.length} onOpenSources={() => setSelected(true)}
        sourcesContent={<SessionSources key={chatId} {...session} onRetry={session.retry} onOpenLibrary={url => { window.history.pushState({}, '', url); window.dispatchEvent(new PopStateEvent('popstate')); close() }} />}
        onSelect={noop} onClose={() => setSelected(false)} onCloseAll={() => setSelected(false)} onDuplicate={noop} onRename={noop} onCloseOthers={noop} onResizeTerminal={noop} onResizePanel={noop}
        onOpenTerminal={noop} onOpenBrowser={noop} onOpenReview={noop} onOpenFiles={noop} onOpenMods={noop} onOpenBackgroundTasks={noop} onModsPanelTarget={noop} onBackgroundPanelTarget={noop} onClosePanel={close} />}
    </div>
    <pre id="qa-results">Running…</pre>
  </>
}
createRoot(document.getElementById('root')!).render(<Fixture />)
const delay = (ms = 50) => new Promise(resolve => setTimeout(resolve, ms))
async function waitFor(check: () => boolean, name: string) { for (let i = 0; i < 100; i++) { if (check()) return; await delay() } throw new Error(name) }
const results: string[] = []
const check = (condition: unknown, name: string) => { if (!condition) throw new Error(name); results.push(`PASS ${name}`) }
function click(selector: string) { const target = document.querySelector<HTMLElement>(selector); if (!target) throw new Error(`Missing ${selector}`); target.click() }
async function run() {
  await waitFor(() => Boolean(document.querySelector('.tools-panel-toggle')), 'Tools trigger')
  check(!document.querySelector('.session-sources-toggle'), 'standalone Sources button is removed')
  click('.tools-panel-toggle')
  await waitFor(() => document.querySelector('[aria-label="9 session sources"]'), 'Sources count')
  check(true, 'Tools launcher shows the current session source count')
  click('[title="Session sources"]')
  await waitFor(() => Boolean(document.querySelector('.session-source-row')), 'Saved sources')
  check(document.querySelectorAll('.session-source-row').length === 9, 'all session sources are visible without expanding')
  check(document.querySelector<HTMLAnchorElement>('.session-source-row[href^="/library"]')?.getAttribute('href') === '/library?item=reference-item', 'source image links to its Library original')
  check(document.querySelector('.session-source-icon img'), 'sent image has a thumbnail')
  check(preview[0].name === 'reference.png' && (preview[0].thumbnailDataUri?.length || 0) < 262144, 'thumbnail generation preserves name and bounds payload size')
  check(document.querySelector('.session-sources')?.textContent?.includes('Math-Data-Science-Exam.pdf'), 'sent document appears with filename')
  if (location.search.includes('preview')) { document.getElementById('qa-results')!.textContent = ''; return }
  check(!document.querySelector('.session-sources-more'), 'Sources does not hide items behind Show more')
  check([...document.querySelectorAll('.session-sources-group h3')].map(el => el.textContent).join('|') === 'Images1|Files6|Web searches2', 'group headings show their counts')
  const search = document.querySelector<HTMLInputElement>('[aria-label="Search Sources"]')!
  check(search, 'larger source collections offer search')
  const enterSearch = async (value: string) => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(search, value)
    search.dispatchEvent(new Event('input', { bubbles: true }))
    await delay()
  }
  await enterSearch('LEAST_SQUARES')
  check(document.querySelectorAll('.session-source-row').length === 1 && document.querySelector('.session-source-row')?.textContent?.includes('02_least_squares'), 'search filters names without case sensitivity')
  await enterSearch('docs.example.com')
  check(document.querySelectorAll('.session-source-row').length === 1, 'search filters domains')
  await enterSearch('no-such-source')
  check(document.querySelector('.session-sources-empty')?.textContent?.includes('No sources match'), 'unmatched search explains its empty result')
  await enterSearch('')
  check(document.querySelectorAll('.session-source-row').length === 9, 'clearing search restores every source')
  const titleStyle = getComputedStyle(document.querySelector('.session-source-copy strong')!)
  check(titleStyle.webkitLineClamp === '2' && titleStyle.whiteSpace === 'normal', 'source titles allow two lines')
  check(document.querySelector<HTMLAnchorElement>('.session-source-row[href^="https:"]')?.href === 'https://example.com/regression', 'historical web searches link to source pages')
  document.getAnimations().forEach(animation => animation.finish())
  await delay()
  check(document.documentElement.scrollWidth <= innerWidth, 'Tools does not overflow the page')
  const popover = document.querySelector<HTMLElement>('.tool-surface-sources')!
  check(popover.getBoundingClientRect().right <= innerWidth && popover.getBoundingClientRect().left >= 0, 'Sources tool fits viewport')
  check(getComputedStyle(popover).overflowY === 'auto', 'Sources tool can scroll')
  fail = true; await waitFor(() => Boolean(document.querySelector('.session-sources-error')), 'Error state')
  check(document.querySelectorAll('.session-source-row').length === 9, 'refresh error preserves known sources')
  fail = false; click('.session-sources-error button'); await waitFor(() => !document.querySelector('.session-sources-error'), 'Retry')
  check(true, 'retry recovers')
  deferA = true; click('.tools-panel-toggle'); await delay(); click('.tools-panel-toggle'); await waitFor(() => Boolean(resolveA), 'Pending A request')
  click('#switch-chat'); await waitFor(() => Boolean(document.querySelector('.session-sources-empty')), 'Empty B')
  resolveA!(data); await delay()
  check(document.querySelectorAll('.session-source-row').length === 0, 'late response cannot leak another session sources')
  check(document.querySelector('.session-sources-empty')?.textContent?.includes('Images and files you send'), 'empty session explains what appears here')
  deferA=false;click('#switch-chat')
  await waitFor(()=>Boolean(document.querySelector('.session-source-row[href^="/library"]')),'Sources for internal navigation')
  let navigated=false
  const navigation=()=>{navigated=true}
  window.addEventListener('popstate',navigation)
  const libraryLink=document.querySelector<HTMLAnchorElement>('.session-source-row[href^="/library"]')!
  const clickEvent=new MouseEvent('click',{bubbles:true,cancelable:true,button:0})
  let preventedByComponent=false
  const suppressReload=(event:MouseEvent)=>{preventedByComponent=event.defaultPrevented;event.preventDefault()}
  document.addEventListener('click',suppressReload)
  libraryLink.dispatchEvent(clickEvent)
  document.removeEventListener('click',suppressReload)
  check(preventedByComponent && navigated, 'Library source navigation preserves the live application and composer')
  window.removeEventListener('popstate',navigation)
  await waitFor(()=>!document.querySelector('.tool-surface-page'),'Source navigation closes Tools')
  check(document.querySelector<HTMLTextAreaElement>('textarea')?.value === 'Keep my draft', 'Library navigation preserves the draft')
  click('.tools-panel-toggle'); await waitFor(() => Boolean(document.querySelector('.tool-surface-sources')), 'Sources reopens')
  document.querySelector<HTMLButtonElement>('[role="tab"]')!.focus()
  document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  await waitFor(() => !document.querySelector('.tool-surface-page'), 'Escape closes Sources tool')
  await waitFor(() => document.activeElement === document.querySelector('.tools-panel-toggle'), 'Escape restores focus to Tools')
  check(true, 'Escape restores focus to Tools')
  const iconFixture = document.createElement('div')
  document.body.append(iconFixture)
  const iconRoot = createRoot(iconFixture)
  iconRoot.render(<SessionSources sources={[
    { id: 'web-good', kind: 'web', name: 'Website with icon', detail: 'example.com', url: 'https://example.com/with-icon', faviconUrl: originalImage },
    { id: 'web-broken', kind: 'web', name: 'Website without icon', detail: 'other.example', url: 'https://other.example/no-icon', faviconUrl: 'data:image/png;base64,AA==' },
    { id: 'image-thumbnail', kind: 'image', name: 'reference.png', detail: 'Image', thumbnailDataUri: originalImage },
  ]} loading={false} error="" onRetry={() => {}} onOpenLibrary={() => {}} />)
  await waitFor(() => Boolean(iconFixture.querySelector<HTMLImageElement>('.is-web img')?.naturalWidth), 'Website favicon loads')
  const favicon = iconFixture.querySelector<HTMLImageElement>('.is-web img')!
  check(favicon.width === 20 && favicon.height === 20, 'website favicon fits its existing icon slot')
  check(favicon.referrerPolicy === 'no-referrer', 'favicon request omits the chat page referrer')
  await waitFor(() => Boolean(iconFixture.querySelector('[href="https://other.example/no-icon"] .is-web svg')), 'Unavailable favicon falls back')
  check(!iconFixture.querySelector('[href="https://other.example/no-icon"] img'), 'failed favicon uses the globe instead of a broken image')
  check(iconFixture.querySelector<HTMLImageElement>('.is-image img')?.width === 30, 'attachment thumbnails retain their existing size')
  favicon.dispatchEvent(new Event('error'))
  await waitFor(() => Boolean(iconFixture.querySelector('[href="https://example.com/with-icon"] .is-web svg')), 'Late favicon failure falls back')
  check(Boolean(iconFixture.querySelector('[href="https://example.com/with-icon"]')), 'favicon failure preserves the source link')
  iconRoot.unmount(); iconFixture.remove()
  let openedArtifact: any = null
  const artifactFixture = document.createElement('div')
  document.body.append(artifactFixture)
  const artifactRoot = createRoot(artifactFixture)
  artifactRoot.render(<SessionSources sources={[
    { id: 'img-1', kind: 'image', name: 'photo.png', detail: 'Image', thumbnailDataUri: originalImage },
  ]} loading={false} error="" onRetry={() => {}} onOpenLibrary={() => {}} onOpenArtifact={art => { openedArtifact = art }} />)
  await waitFor(() => Boolean(artifactFixture.querySelector('.session-source-row')), 'Image row ready')
  const imgRow = artifactFixture.querySelector<HTMLAnchorElement>('.session-source-row')!
  check(imgRow.getAttribute('title')?.includes('Open image preview in Tools'), 'image source row indicates tool preview')
  imgRow.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }))
  await waitFor(() => Boolean(openedArtifact), 'Image opens via onOpenArtifact')
  check(openedArtifact?.type === 'image' && openedArtifact?.path === 'sources/img-1/photo.png', 'image opens as artifact preview')
  artifactRoot.unmount(); artifactFixture.remove()
  document.getElementById('qa-results')!.textContent = results.join('\n') + '\nALL PASSED'
}
run().catch(error => { document.getElementById('qa-results')!.textContent = results.join('\n') + `\nFAIL ${error.message}` })
