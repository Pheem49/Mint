import React, { useState } from 'react'
import { createRoot } from 'react-dom/client'
import ChatMessageItem from '../shared/components/ChatMessageItem'
import { ProgressActivity } from '../shared/components/ProgressActivity'
import ViewedImages from '../shared/components/ViewedImages'
import type { AgentProgress } from '../shared/types'
import { compactAgentProgressForPersistence, reduceLiveAgentProgress } from '../shared/agentProgress'
import '../shared/theme/tokens.css'
if (new URLSearchParams(location.search).get('platform') === 'desktop') {
  await import('../src/css/app-shell.css'); await import('../src/css/chat.css')
} else {
  await import('../src-web/css/app-shell.css'); await import('../src-web/css/chat.css')
}

const canvas = document.createElement('canvas'); canvas.width = 960; canvas.height = 600
const context = canvas.getContext('2d')!
context.fillStyle = '#1b3534'; context.fillRect(0, 0, 960, 600)
context.fillStyle = '#8bd8c3'; context.fillRect(56, 56, 848, 100)
context.fillStyle = '#dce9e4'; context.font = '28px sans-serif'; context.fillText('view_image · screenshot.png', 56, 220)
context.strokeStyle = '#8bd8c3'; context.lineWidth = 3; context.strokeRect(56, 270, 390, 260); context.strokeRect(480, 270, 424, 260)
const image = canvas.toDataURL('image/png')
const svg = `data:image/svg+xml;base64,${btoa('<svg xmlns="http://www.w3.org/2000/svg" width="420" height="260"><rect width="420" height="260" fill="#21343b"/><circle cx="210" cy="130" r="80" fill="#91d6b5"/></svg>')}`
const input = { path: '/workspace/ภาพตัวอย่างที่มีชื่อไฟล์ยาวเพื่อทดสอบการแสดงผลบนโทรศัพท์มือถือ.png' }
const start: AgentProgress = { type: 'ToolStart', data: { action: 'view_image', input } }
const textActivity: AgentProgress = { type: 'ToolEnd', data: { action: 'read_file', input: {}, result: 'Read' } }
const finished: AgentProgress[] = [textActivity, textActivity, start,
  { type: 'ToolEnd', data: { action: 'view_image', input, result: `${image}\n\n[Hook] Viewed successfully` } },
  { type: 'ToolEnd', data: { callId: 'image-2', action: 'view_image', input: { path: '/workspace/diagram.svg' }, result: svg } },
]
const noop = () => {}
let continueRun = noop
function Fixture() {
  const [done, setDone] = useState(false)
  const [mode, setMode] = useState('live')
  const [open, setOpen] = useState(true)
  const [continued, setContinued] = useState(false)
  continueRun = () => setContinued(true)
  const progress = done ? continued ? Array.from({ length: 40 }, () => ({ ...textActivity })).reduce(reduceLiveAgentProgress, finished) : finished : [start]
  return <>
    <style>{`*{box-sizing:border-box}body{margin:0;padding:16px;background:#101112;color:var(--text-main);font-family:Arial,sans-serif}.fixture-controls{display:flex;flex-wrap:wrap;gap:8px;margin-bottom:20px}.fixture-message{max-width:760px;margin:auto;min-width:0}#qa-results{white-space:pre-wrap;font-size:10px;overflow-wrap:anywhere}.message-bubble{width:100%}`}</style>
    <div className="fixture-controls"><button id="finish" onClick={() => setDone(true)}>Finish view_image</button><button id="history" onClick={() => setMode('history')}>Reload saved history</button><button id="switch" onClick={() => setMode('other')}>Other conversation</button><button id="live" onClick={() => setMode('live')}>Live view</button></div>
    <div className="fixture-message">
      {mode === 'live' ? <><ProgressActivity progress={progress} id="live" isOpen={open} onToggle={() => setOpen(value => !value)} /><div className="message ai-message"><div className="bubble-wrapper"><div className="message-bubble"><ViewedImages progress={progress} /></div></div></div></>
        : <ChatMessageItem interaction={{ id: mode === 'other' ? 8 : 7, status: 'completed', aiText: 'I have inspected the images.', userText: 'ช่วยดูรูปภาพนี้หน่อย', createdAt: '2026-10-16T05:00:00Z', agentActivity: mode === 'other' ? [] : JSON.parse(JSON.stringify(compactAgentProgressForPersistence(finished))) }} copiedId={null} speakingText={null} agentActivitySnapshots={{}} thinkingExpanded={{}} onThinkingExpandedChange={noop} handleCopyMessage={noop} speak={noop} toggleActivity={noop} renderFileChanges={() => null} />}
    </div><pre id="qa-results">Running…</pre>
  </>
}
createRoot(document.getElementById('root')!).render(<Fixture />)
const delay = (ms = 40) => new Promise(resolve => setTimeout(resolve, ms))
async function waitFor(check: () => boolean, name: string) { for (let i = 0; i < 120; i++) { if (check()) return; await delay() } throw new Error(name) }
const results: string[] = []
const check = (value: unknown, name: string) => { if (!value) throw new Error(name); results.push(`PASS ${name}`) }
const click = (selector: string) => { const element = document.querySelector<HTMLElement>(selector); if (!element) throw new Error(`Missing ${selector}`); element.focus(); element.click() }
async function run() {
  await waitFor(() => Boolean(document.querySelector('.agent-activity-item')), 'Tool starts')
  check(!document.querySelector('[data-viewed-image]'), 'no image appears before view_image completes')
  click('#finish')
  await waitFor(() => document.querySelectorAll('.viewed-image-button img').length === 2, 'Both results appear')
  await waitFor(() => [...document.querySelectorAll<HTMLImageElement>('.viewed-image-button img')].every(img => img.naturalWidth > 0), 'PNG and SVG load')
  // Capture final layout without depending on headless compositor animation timing.
  document.getAnimations().forEach(animation => { if (animation.effect?.getTiming().iterations !== Infinity) animation.finish() })
  check(getComputedStyle(document.querySelector('.message')!).opacity === '1', 'chat content is visibly rendered')
  check(document.querySelector('.agent-activity-target-text')!.getBoundingClientRect().width > 40, 'view_image row keeps room for the file path')
  check(document.documentElement.scrollWidth <= innerWidth, 'images and long filenames fit viewport')
  click('.agent-activity-item')
  await waitFor(() => Boolean(document.querySelector('.agent-activity-output')), 'Expanded tool output')
  check(!document.querySelector('.agent-activity-output')?.textContent?.includes('data:image/'), 'tool output does not dump base64')
  check(document.querySelector('.agent-activity-output')?.textContent?.includes('[Hook] Viewed successfully'), 'hook notes remain readable')
  if (location.search.includes('preview')) { document.getElementById('qa-results')!.textContent = ''; return }
  const trigger = document.querySelector<HTMLButtonElement>('.viewed-image-button')!
  trigger.focus(); trigger.click()
  await waitFor(() => Boolean(document.querySelector('dialog[open] img')), 'Expanded preview')
  check(document.querySelector<HTMLImageElement>('dialog[open] img')?.naturalWidth === 960, 'expanded preview uses original pixels')
  const rect = document.querySelector('dialog[open]')!.getBoundingClientRect()
  check(rect.left >= 0 && rect.right <= innerWidth && rect.bottom <= innerHeight, 'expanded preview fits viewport')
  const openedDialog = document.querySelector('dialog[open]')
  continueRun(); await delay(100)
  check(document.querySelector('dialog[open]') === openedDialog, 'preview stays open as older activity is trimmed during a run')
  click('dialog[open] button')
  await waitFor(() => !document.querySelector('dialog[open]'), 'Preview closes')
  check(document.activeElement === trigger, 'closing preview restores keyboard focus')
  click('#history')
  await waitFor(() => Boolean(document.querySelector('.agent-activity-history')), 'Saved history mounts')
  check(!document.querySelector('.agent-activity-card-history') && document.querySelectorAll('[data-viewed-image]').length === 2, 'saved images remain visible with Activity collapsed')
  click('#switch'); await waitFor(() => !document.querySelector('[data-viewed-image]'), 'Session switch')
  check(!document.querySelector('[data-viewed-image]'), 'images do not leak into another conversation')
  click('#live'); await waitFor(() => document.querySelectorAll('[data-viewed-image]').length === 2, 'Live view returns')
  document.querySelector('.viewed-image-button img')!.dispatchEvent(new Event('error'))
  await waitFor(() => Boolean(document.querySelector('.viewed-image-error')), 'Image error')
  check(document.querySelectorAll('.viewed-image-button').length === 1, 'a broken image leaves other images available')
  document.getElementById('qa-results')!.textContent = results.join('\n') + '\nALL PASSED'
}
run().catch(error => { document.getElementById('qa-results')!.textContent = results.join('\n') + `\nFAIL ${error.message}` })
