import React, { useState } from 'react'
import { createRoot } from 'react-dom/client'
import ChatMessageItem from '../shared/components/chat/ChatMessageItem'
import { installRendererPlatform, type RendererAdapter } from '../shared/platform'
import { compactAgentProgressForPersistence, reduceLiveAgentProgress } from '../shared/agentProgress'
import type { AgentProgress } from '../shared/types'
import '../shared/theme/tokens.css'
if (new URLSearchParams(location.search).get('platform') === 'desktop') {
  await import('../src/css/app-shell.css'); await import('../src/css/chat.css')
} else {
  await import('../src-web/css/app-shell.css'); await import('../src-web/css/chat.css')
}

const canvas = document.createElement('canvas'); canvas.width = 960; canvas.height = 600
const ctx = canvas.getContext('2d')!
ctx.fillStyle = '#102a25'; ctx.fillRect(0, 0, 960, 600)
ctx.fillStyle = '#94dcc1'; ctx.font = '36px sans-serif'; ctx.fillText('Search results', 48, 80)
ctx.fillStyle = '#d8e6e0'; ctx.font = '25px sans-serif'; ctx.fillText('ทดสอบ Mint · Matching result', 48, 165)
ctx.strokeStyle = '#94dcc1'; ctx.strokeRect(48, 215, 864, 320)
const image = canvas.toDataURL('image/png')
installRendererPlatform({ readMcpArtifact: async (id, chat) => {
  if (id !== 'browser-image' || chat !== 'browser-chat') throw new Error('Artifact unavailable')
  return image
} } as RendererAdapter)

const artifacts: AgentProgress = { type: 'ToolArtifacts', data: { callId: 'capture', warnings: [], artifacts: [{ id: 'browser-image', name: 'result.png', mimeType: 'image/png', kind: 'image', size: 12000, chatId: 'browser-chat', callId: 'capture', server: 'browser', metadata: { url: 'http://localhost:3000/search', width: 960, height: 600, capturedAt: '2026-10-19T10:00:00Z' } }] } }
const original: AgentProgress[] = [
  { type: 'ToolEnd', data: { action: 'browser_screenshot', callId: 'capture', input: {}, result: 'Screenshot saved as an artifact.', status: 'success' } }, artifacts,
  // Reopening the same image should not create another gallery card.
  { ...artifacts, data: { ...artifacts.data, callId: 'view-again' } },
  { type: 'RunCompleted', data: { summary: { outcome: 'UNVERIFIED' } as any } },
]
const saved = compactAgentProgressForPersistence(Array.from({ length: 45 }, (_, index): AgentProgress => ({ type: 'ToolEnd', data: { action: 'read_file', input: { path: `file-${index}` }, result: 'Read' } })).reduce(reduceLiveAgentProgress, original))
const noop = () => {}
function Fixture() {
  const [other, setOther] = useState(false)
  return <>
    <style>{`*{box-sizing:border-box}body{margin:0;padding:20px;background:#101112;color:var(--text-main);font-family:Arial,sans-serif}.fixture{max-width:760px;margin:24px auto}.fixture-controls{display:flex;gap:8px;flex-wrap:wrap}.message-bubble{width:100%}#qa-results{font-size:12px;white-space:pre-wrap;overflow-wrap:anywhere}.mcp-artifact>span{overflow-wrap:anywhere}`}</style>
    <div className="fixture-controls"><button id="switch-chat" onClick={() => setOther(v => !v)}>Switch conversation</button></div>
    <div className="fixture"><ChatMessageItem interaction={{ id: other ? 8 : 7, status: 'completed', aiText: 'Search finished. Visual review remains unverified.', userText: 'เปิดเว็บ กรอกคำค้น แล้วแคปผลให้ดู', createdAt: '2026-10-19T10:00:00Z', agentActivity: other ? [] : JSON.parse(JSON.stringify(saved)) }} copiedId={null} speakingText={null} agentActivitySnapshots={{}} thinkingExpanded={{}} onThinkingExpandedChange={noop} handleCopyMessage={noop} speak={noop} toggleActivity={noop} renderFileChanges={() => null} /></div>
    <pre id="qa-results">Running…</pre>
  </>
}
createRoot(document.getElementById('root')!).render(<Fixture />)
const delay = () => new Promise(resolve => setTimeout(resolve, 40))
async function waitFor(check: () => boolean, label: string) { for (let i = 0; i < 100; i++) { if (check()) return; await delay() } throw new Error(label) }
const results: string[] = []
function check(value: unknown, label: string) { if (!value) throw new Error(label); results.push(`PASS ${label}`) }
async function run() {
  await waitFor(() => !!document.querySelector<HTMLImageElement>('.mcp-image-button img')?.naturalWidth, 'artifact image loads')
  check(document.querySelectorAll('.mcp-image-button').length === 1, 'reopened artifact is shown once')
  check(document.querySelector('[data-completion-status="unverified"]'), 'unverified state survives history serialization')
  check(!document.querySelector('.agent-activity-toggle[aria-expanded="true"]'), 'image is visible with Activity collapsed')
  const download = document.querySelector<HTMLAnchorElement>('a[download="result.png"]')
  check(download?.href.startsWith('data:image/png'), 'original image has a download link')
  document.querySelector<HTMLButtonElement>('.mcp-image-button')!.click()
  await waitFor(() => !!document.querySelector('dialog[open]'), 'image expands')
  check(document.querySelector('dialog[open] img'), 'expanded image is present')
  document.querySelector<HTMLButtonElement>('dialog[open] button')!.click()
  await waitFor(() => !document.querySelector('dialog[open]'), 'image closes')
  document.querySelector<HTMLButtonElement>('#switch-chat')!.click()
  await waitFor(() => !document.querySelector('.mcp-image-button'), 'other chat has no previous images')
  document.querySelector<HTMLButtonElement>('#switch-chat')!.click()
  await waitFor(() => !!document.querySelector<HTMLImageElement>('.mcp-image-button img')?.naturalWidth, 'saved image returns')
  check(document.documentElement.scrollWidth <= innerWidth, 'cards fit the viewport')
  document.getElementById('qa-results')!.textContent = results.join('\n') + '\nALL PASS'
}
run().catch(error => { document.getElementById('qa-results')!.textContent = `FAIL ${error.message}\n${results.join('\n')}` })
