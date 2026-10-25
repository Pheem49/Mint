import React from 'react'
import { createRoot } from 'react-dom/client'
import ChatPanel from '../shared/components/chat/ChatPanel'
import { chatPanelFixture } from './chatPanelFixture'
import { installRendererPlatform } from '../shared/platform'
import * as webAdapter from '../src-web/tauri'
installRendererPlatform(webAdapter)
let cancellations = 0
const fixture = chatPanelFixture()
createRoot(document.getElementById('root')!).render(<>
  <ChatPanel {...fixture as any} conversation={{ ...fixture.conversation, sending: true }} actions={{ ...fixture.actions, onCancelMessage: () => cancellations++ }} tools={{ ...fixture.tools, open: true }} />
  <aside className="tool-surface-page"><button id="tool-control">Tool control</button></aside>
</>)
async function run() {
  for (let i = 0; i < 100 && !document.getElementById('chat-input'); i++) await new Promise(resolve => setTimeout(resolve, 40))
  await new Promise(resolve => setTimeout(resolve, 100))
  const composer = document.getElementById('chat-input')!
  composer.focus()
  composer.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
  if (cancellations !== 1) throw Error('Escape from chat must cancel while Tools is open')
  document.getElementById('tool-control')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
  if (cancellations !== 1) throw Error('Escape inside Tools must not cancel chat')
  document.getElementById('result')!.textContent = JSON.stringify({ status: 'PASS', checks: ['chat Escape cancels with Tools open', 'Tools Escape leaves generation running'] })
}
run().catch(error => { document.getElementById('result')!.textContent = JSON.stringify({ status: 'FAIL', error: String(error) }) })
