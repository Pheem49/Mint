import React, { useState } from 'react'
import { createRoot } from 'react-dom/client'
import WorkspaceContextPopover from '../shared/components/WorkspaceContextPopover'
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
  return <>
    <style>{`*{box-sizing:border-box}body{margin:0;background:#111113;color:#e8eaed;font-family:Arial,sans-serif;--text-main:#e8eaed;--text-secondary:#c5cad1;--text-muted:#8b939f;--bg-overlay:#111113;--surface-strong:#25262b;--border:#33353b;--accent:#7edbb5;--border-subtle:#27292d;--bg-surface-soft:#1a1b20;--bg-surface-hover:#22242a}.fixture-bar{display:flex;align-items:center;justify-content:flex-end;gap:12px;height:50px;padding:8px 16px;border-bottom:1px solid #33353b}#qa-results{position:fixed;bottom:6px;left:8px;max-width:calc(100vw - 16px);font-size:10px;white-space:pre-wrap;pointer-events:none}`}</style>
    <header className="fixture-bar"><span>Session sources fixture</span><button id="switch-chat" onClick={() => setChatId(id => id === 'A' ? 'B' : 'A')}>Switch session</button><WorkspaceContextPopover workspacePath="/workspace/Mint-CLI" terminalCount={0} chatId={chatId} liveProgress={[]} recentChanges={[]} onOpenWorkspace={() => {}} onOpenReview={() => {}} onRefreshWorkspace={() => {}} /></header>
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
  await waitFor(() => Boolean(document.querySelector('.workspace-context-trigger')), 'Trigger')
  click('.workspace-context-trigger')
  await waitFor(() => Boolean(document.querySelector('.session-source-row')), 'Saved sources')
  check(document.querySelectorAll('.session-source-row').length === 5, 'initial list shows every source category without flooding the popover')
  check(document.querySelector<HTMLAnchorElement>('.session-source-row[href^="/library"]')?.getAttribute('href') === '/library?item=reference-item', 'source image links to its Library original')
  check(document.querySelector('.session-source-icon img'), 'sent image has a thumbnail')
  check(preview[0].name === 'reference.png' && (preview[0].thumbnailDataUri?.length || 0) < 262144, 'thumbnail generation preserves name and bounds payload size')
  check(document.querySelector('.session-sources')?.textContent?.includes('Math-Data-Science-Exam.pdf'), 'sent document appears with filename')
  if (location.search.includes('preview')) { document.getElementById('qa-results')!.textContent = ''; return }
  click('.session-sources-more')
  await waitFor(() => document.querySelectorAll('.session-source-row').length === 9, 'Expanded list')
  check(document.querySelectorAll('.session-source-row').length === 9, 'show more reveals all session sources')
  check(document.querySelector<HTMLAnchorElement>('.session-source-row[href^="https:"]')?.href === 'https://example.com/regression', 'historical web searches link to source pages')
  const popover = document.querySelector<HTMLElement>('.workspace-context-popover')!
  check(popover.getBoundingClientRect().right <= innerWidth && popover.getBoundingClientRect().left >= 0, 'popover fits viewport')
  check(getComputedStyle(popover).overflowY === 'auto', 'expanded sources can scroll')
  fail = true; await waitFor(() => Boolean(document.querySelector('.session-sources-error')), 'Error state')
  check(document.querySelectorAll('.session-source-row').length === 9, 'refresh error preserves known sources')
  fail = false; click('.session-sources-error button'); await waitFor(() => !document.querySelector('.session-sources-error'), 'Retry')
  check(true, 'retry recovers')
  deferA = true; click('.workspace-context-trigger'); await delay(); click('.workspace-context-trigger'); await waitFor(() => Boolean(resolveA), 'Pending A request')
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
  await waitFor(()=>!document.querySelector('.workspace-context-popover'),'Source navigation closes its popover')
  document.getElementById('qa-results')!.textContent = results.join('\n') + '\nALL PASSED'
}
run().catch(error => { document.getElementById('qa-results')!.textContent = results.join('\n') + `\nFAIL ${error.message}` })
