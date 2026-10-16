import React, { useState } from 'react'
import { createRoot } from 'react-dom/client'
import ChatPanel from '../shared/components/ChatPanel'
import { chatPanelFixture } from './chatPanelFixture'
import { installRendererPlatform, installWorkspacePlatform } from '../shared/platform'
import * as webAdapter from '../src-web/tauri'
import type { AgentProgress, ConversationSources } from '../shared/types'
import '../shared/theme/tokens.css'
import '../src-web/css/app-shell.css'
import '../src-web/css/chat.css'
import '../src-web/css/mobile.css'

const image = 'data:image/svg+xml;base64,' + btoa('<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#73d6b0"/></svg>')
const search = (url: string): AgentProgress => ({ type: 'ToolEnd', data: { action: 'web_search', input: {}, result: `1. Source reference\nURL: ${url}\nReference` } })
const data: ConversationSources = { attachments: [
  { id: 1, interactionId: 2, libraryItemId: 'reference', name: 'reference.png', kind: 'image', mimeType: 'image/png', thumbnailDataUri: image },
  { id: 2, interactionId: 2, libraryItemId: 'notes', name: 'notes.pdf', kind: 'file', mimeType: 'application/pdf', thumbnailDataUri: null },
], webSearchProgress: [search('https://example.com/reference')] }
const requests: string[] = []
let fail = false, deferA = false
let resolveA: ((data: ConversationSources) => void) | undefined
installRendererPlatform({ ...webAdapter,
  getConversationSources: async chatId => { requests.push(chatId); if (fail) throw new Error('Offline'); if (chatId === 'A' && deferA) return new Promise(resolve => { resolveA = resolve }); return chatId === 'A' ? data : { attachments: [], webSearchProgress: [] } },
  modsOperation: async () => ({ mods: [] }), fetchProviderModels: async () => [],
})
installWorkspacePlatform({ getGitBranchInfo: async () => ({ isRepository: false, currentBranch: null, branches: [], remoteBranches: [], detachedHead: null, isDirty: false }) } as any)
const originalFetch = window.fetch.bind(window)
window.fetch = (input, options) => String(input).includes('localhost:11434') ? Promise.resolve(new Response('{"models":[]}')) : originalFetch(input, options)
let selectChat = (_id: string) => {}, addLiveSearch = () => {}, setRunning = (_running: boolean) => {}
let cancellations = 0
function Fixture() {
  const props = chatPanelFixture()
  const [chatId, setChatId] = useState('A')
  const [draft, setDraft] = useState('Keep my draft')
  const [live, setLive] = useState<AgentProgress[]>([])
  const [running, changeRunning] = useState(false)
  setRunning = changeRunning
  selectChat = id => { setChatId(id); setLive([]) }
  addLiveSearch = () => setLive([search('https://example.com/live')])
  return <>
    <style>{`html,body,#root{margin:0;height:100%;width:100%;background:var(--bg-overlay);color:var(--text-main);font-family:Arial,sans-serif}*{box-sizing:border-box}#root{display:flex;flex-direction:column}.fixture-controls{padding:6px;flex:0 0 auto}.assistant-workspace{flex:1;min-height:0;min-width:0;display:flex}#qa-results{position:fixed;bottom:0;left:0;max-width:100vw;font-size:9px;white-space:pre-wrap;overflow-wrap:anywhere;pointer-events:none;z-index:900}`}</style>
    <div className="fixture-controls">Real Web ChatPanel · Sources fixture</div>
    <main className="assistant-workspace chat-only"><ChatPanel {...props as any} conversation={{ ...props.conversation, chatId, message: draft, sending: running, agentProgress: live, workspacePath: '/workspace/a-very-long-project-name' }} actions={{ ...props.actions, onSetMessage: setDraft, onCancelMessage: () => { cancellations++ } }} showSessionSources /></main>
    <pre id="qa-results">Running…</pre>
  </>
}
createRoot(document.getElementById('root')!).render(<Fixture />)
const delay = (ms = 50) => new Promise(resolve => setTimeout(resolve, ms))
async function waitFor(check: () => boolean, name: string) { for (let i = 0; i < 100; i++) { if (check()) return; await delay() } throw new Error(name) }
const results: string[] = []
const check = (value: unknown, name: string) => { if (!value) throw new Error(name); results.push(`PASS ${name}`) }
const click = (selector: string) => { const element = document.querySelector<HTMLElement>(selector); if (!element) throw new Error(`Missing ${selector}`); element.focus(); element.click() }
async function run() {
  await waitFor(() => Boolean(document.querySelector('.session-sources-toggle')), 'Sources entry appears')
  check(requests.length === 0, 'closed Sources does not fetch')
  const actions = document.querySelector('.chat-header-actions')!
  check(actions.firstElementChild?.classList.contains('session-sources-toggle'), 'Sources sits beside Tools in the real chat header')
  check(actions.getBoundingClientRect().right <= innerWidth, 'header actions fit the viewport')
  click('.session-sources-toggle')
  await waitFor(() => document.querySelectorAll('.session-source-row').length === 3, 'Session sources load')
  check(requests[0] === 'A', 'Web fetches the current conversation')
  check(document.querySelector('.session-sources-heading')?.textContent?.includes('This session'), 'panel identifies its session scope')
  const bounds = document.querySelector('.session-sources-popover')!.getBoundingClientRect()
  check(bounds.left >= 0 && bounds.right <= innerWidth && bounds.bottom <= innerHeight, 'Sources panel fits the viewport')
  check(Boolean(document.querySelector('.session-source-icon.is-web img, .session-source-icon.is-web svg')), 'website sources retain their favicon or fallback')
  addLiveSearch(); await waitFor(() => document.querySelectorAll('.session-source-row').length === 4, 'Live source updates')
  check(true, 'live search sources appear without reopening')
  if (location.search.includes('preview')) { document.getElementById('qa-results')!.textContent = ''; return }
  setRunning(true); await delay()
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  await waitFor(() => !document.querySelector('.session-sources-popover'), 'Escape closes Sources')
  check(document.activeElement === document.querySelector('.session-sources-toggle'), 'Escape restores focus to Sources')
  check(cancellations === 0, 'closing Sources with Escape does not cancel the running agent')
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  check(cancellations === 1, 'Escape still cancels the agent when Sources is closed')
  setRunning(false); await delay()
  const closedCount = requests.length; await delay(2200)
  check(requests.length === closedCount, 'closing Sources stops its polling')
  deferA = true; click('.session-sources-toggle'); await waitFor(() => Boolean(resolveA), 'Deferred A request')
  selectChat('B'); await waitFor(() => Boolean(document.querySelector('.session-sources-empty')), 'Session B is empty')
  resolveA!(data); await delay()
  check(document.querySelectorAll('.session-source-row').length === 0, 'late A response cannot leak into B')
  fail = true; deferA = false; selectChat('A'); await waitFor(() => Boolean(document.querySelector('.session-sources-error')), 'Source error')
  fail = false; click('.session-sources-error button'); await waitFor(() => document.querySelectorAll('.session-source-row').length === 3, 'Retry loads saved sources')
  check(true, 'Sources retries a failed request')
  let navigated = false
  const onNavigate = () => { navigated = true }; window.addEventListener('popstate', onNavigate)
  click('.session-source-row[href^="/library"]'); await waitFor(() => !document.querySelector('.session-sources-popover'), 'Library navigation closes Sources')
  check(navigated && location.pathname === '/library', 'source link opens Library through SPA navigation')
  check(document.querySelector<HTMLTextAreaElement>('#chat-input')?.value === 'Keep my draft', 'opening a source preserves the composer draft')
  window.removeEventListener('popstate', onNavigate)
  click('.session-sources-toggle'); await waitFor(() => Boolean(document.querySelector('.session-sources-popover')), 'Sources reopens')
  document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
  await waitFor(() => !document.querySelector('.session-sources-popover'), 'Outside click closes')
  check(true, 'outside click closes Sources')
  document.getElementById('qa-results')!.textContent = results.join('\n') + '\nALL PASSED'
}
run().catch(error => { document.getElementById('qa-results')!.textContent = results.join('\n') + `\nFAIL ${error.message}` })
