import React, { useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import WorkspacePanel from '../src/components/WorkspacePanel'
import ToolSurfacePage, { type ToolSurface } from '../shared/components/workspace/ToolSurfacePage'
import { installRendererPlatform, installWorkspacePlatform, runtimePlatform, workspacePlatform } from '../shared/platform'
import { createWorkspacePreviewOpener } from '../shared/utils/workspacePreview'
import { WORKSPACE_FILE_MIME, workspacePreviewId } from '../shared/utils/workspaceDrag'
import { samplePdf, sampleWav } from './mediaFixtures'
import videoUrl from './fixtures/preview.webm?url'
import { PreviewObjectUrls } from '../shared/utils/previewObjectUrls'
import '../shared/theme/tokens.css'
import '../src/css/sidebar-workspace.css'

const roots = ['/tmp/mint-drop-a', '/tmp/mint-drop-b']
const names = ['photo.png', 'notes.md', 'main.py', 'archive.zip', 'broken.txt', 'index.html', 'guide.pdf', 'tone.wav', 'clip.webm', 'broken.wav', 'broken.webm']
const calls: string[] = []
const revoked: string[] = []
const originalRevoke = URL.revokeObjectURL.bind(URL)
URL.revokeObjectURL = url => { revoked.push(url); originalRevoke(url) }
let failRead = true
let externalTarget = ''
;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
  if (command !== 'run_desktop_action') throw Error(`Unexpected native call: ${command}`)
  externalTarget = args.action.target
  return { success: true }
} }
const image = 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="400" height="260"><rect width="400" height="260" fill="#0e6551"/><text x="80" y="140" fill="white" font-size="36">Workspace image</text></svg>')
installRendererPlatform({
  isTauriRuntime: () => false,
  readWorkspaceMedia: async (path: string, root: string) => {
    calls.push(`media:${root}:${path}`)
    if (path.startsWith('broken.') && path !== 'broken.txt') { const blob = new Blob(['invalid media'], { type: path.endsWith('.wav') ? 'audio/wav' : 'video/webm' }); return { url: URL.createObjectURL(blob), mimeType: blob.type, objectUrl: true } }
    if (path === 'clip.webm') return { url: videoUrl, mimeType: 'video/webm' }
    const blob = new Blob([path === 'guide.pdf' ? samplePdf() : sampleWav()], { type: path === 'guide.pdf' ? 'application/pdf' : 'audio/wav' })
    return { url: URL.createObjectURL(blob), mimeType: blob.type, objectUrl: true, byteLength: blob.size }
  },
  readWorkspaceImage: async (path: string, root: string) => { calls.push(`image:${root}:${path}`); return image },
  readWorkspaceFile: async (path: string, root: string) => { calls.push(`read:${root}:${path}`); if (path === 'broken.txt' && failRead) throw Error('File unavailable'); return path.endsWith('.md') ? '# Preview heading\n\nMarkdown content' : 'print("Hello from Workspace")' },
} as any)
installWorkspacePlatform({
  listWorkspaceHistory: async () => [],
  getWorkspaceSnapshot: async ({ root }: { root: string }) => ({ path: root, revision: 1, git: { isRepository: false, branches: [], remoteBranches: [], isDirty: false }, tree: { name: root.split('/').at(-1), path: '.', kind: 'directory', children: [
    ...names.map(name => ({ name, path: name, kind: 'file', children: [] })),
    { name: 'folder', path: 'folder', kind: 'directory', children: [] },
  ] } }),
  startHtmlPreview: async (root: string, path: string) => { calls.push(`serve:${root}:${path}`); await new Promise(r => setTimeout(r, 30)); return { url: path === 'index.html' ? 'data:text/html,<h1>HTML preview</h1>' : image, jobId: 'preview' } },
} as any)
const noop = () => {}
function Fixture() {
  const [root, setRoot] = useState(roots[0])
  const [surfaces, setSurfaces] = useState<ToolSurface[]>([])
  const [active, setActive] = useState<string | null>(null)
  const current = useRef(surfaces); current.current = surfaces
  const owner = useRef(new PreviewObjectUrls())
  React.useEffect(() => { owner.current.update(surfaces.flatMap(tab => tab.kind === 'preview' ? [tab.artifact] : [])) }, [surfaces])
  const opener = useRef<ReturnType<typeof createWorkspacePreviewOpener> | null>(null)
  opener.current ??= createWorkspacePreviewOpener({
    readWorkspaceFile: (path, root) => runtimePlatform.readWorkspaceFile(path, root),
    readWorkspaceImage: (path, root) => runtimePlatform.readWorkspaceImage(path, root),
    readWorkspaceMedia: (path, root) => runtimePlatform.readWorkspaceMedia(path, root),
    startHtmlPreview: (root, path) => workspacePlatform.startHtmlPreview(root, path),
    selectIfOpen: id => { if (!current.current.some(tab => tab.id === id)) return false; setActive(id); return true },
    open: artifact => { owner.current.accept(artifact); const id = workspacePreviewId(artifact.workspacePath!, artifact.path); setSurfaces(tabs => [...tabs, { id, kind: 'preview', title: artifact.path, artifact }]); setActive(id) },
  })
  return <>
    <style>{`*{box-sizing:border-box}html,body,#root{margin:0;height:100%;overflow:hidden;background:var(--bg-overlay);color:var(--text-main);font-family:Arial,sans-serif}.fixture-header{height:40px;padding:6px}.app-body{position:relative;height:calc(100% - 40px);display:flex}.fixture-workspace{width:320px;overflow:auto}.assistant-workspace{flex:1;min-width:0;padding:20px}#result{position:fixed;bottom:0;left:0;max-width:40%;font-size:9px;white-space:pre-wrap;pointer-events:none;z-index:900}`}</style>
    <header className="fixture-header"><button id="switch-root" onClick={() => setRoot(value => value === roots[0] ? roots[1] : roots[0])}>Switch workspace</button></header>
    <div className="app-body tool-surface-active" style={{ '--tool-surface-width': '480px' } as React.CSSProperties}>
      <div className="fixture-workspace"><WorkspacePanel workspacePath={root} agentMode sending={false} onEnableAgentMode={noop} onSetMessage={noop} onWorkspaceReady={noop} /></div>
      <main className="assistant-workspace"><h1>Conversation</h1><textarea defaultValue="Keep my draft" /></main>
      <ToolSurfacePage surfaces={surfaces} activeSurfaceId={active} workspacePath={root} terminalSize={480} desktopTools onDropWorkspaceFile={file => opener.current!(file.root, file.path)}
        onSelect={setActive} onClose={id => { setSurfaces(tabs => tabs.filter(tab => tab.id !== id)); setActive(null) }} onCloseAll={noop} onDuplicate={noop} onRename={noop} onCloseOthers={noop} onResizeTerminal={noop} onResizePanel={noop}
        onOpenSources={noop} onOpenTerminal={noop} onOpenBrowser={noop} onOpenReview={noop} onOpenFiles={noop} onOpenMods={noop} onOpenBackgroundTasks={noop} onModsPanelTarget={noop} onBackgroundPanelTarget={noop} onClosePanel={noop} />
    </div>
  </>
}
createRoot(document.getElementById('root')!).render(<Fixture />)
const delay = (ms = 40) => new Promise(resolve => setTimeout(resolve, ms))
async function until(check: () => unknown) { for (let i = 0; i < 100; i++) { if (check()) return; await delay() } throw Error('Timed out') }
const results: string[] = []
function check(value: unknown, name: string) { if (!value) throw Error(name); results.push(name) }
const tools = () => document.querySelector<HTMLElement>('.tool-surface-page')!
async function begin(path: string) {
  const row = document.querySelector<HTMLElement>(`.workspace-tree-row[title="${path}"]`)!
  const dataTransfer = new DataTransfer()
  row.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer }))
  await delay()
  return dataTransfer
}
async function drop(path: string) {
  const dataTransfer = await begin(path)
  tools().dispatchEvent(new DragEvent('dragenter', { bubbles: true, cancelable: true, dataTransfer }))
  tools().dispatchEvent(new DragEvent('dragover', { bubbles: true, cancelable: true, dataTransfer }))
  await delay()
  tools().dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer }))
  await delay()
}
async function run() {
  await until(() => document.querySelector('.workspace-tree-row[title="photo.png"]'))
  const drag = await begin('photo.png')
  check(JSON.parse(drag.getData(WORKSPACE_FILE_MIME)).root === roots[0], 'real Workspace drag includes its source root')
  check(drag.getData('text/plain') === '@photo.png', 'dragging into chat keeps the existing file mention payload')
  check(document.querySelector('.tool-surface-drag-shield'), 'internal file drags protect the drop target above iframe previews')
  tools().dispatchEvent(new DragEvent('dragenter', { bubbles: true, dataTransfer: drag }))
  await delay()
  check(document.querySelector('.tool-surface-drop-overlay')?.textContent?.includes('Drop to preview'), 'dragging over Tools shows the Mint drop overlay')
  tools().dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: drag }))
  await until(() => document.querySelector<HTMLImageElement>('.artifact-preview-panel img')?.naturalWidth)
  check(!document.querySelector('.tool-surface-drop-overlay'), 'dropping clears the drag overlay')
  check(calls.filter(call => call.startsWith('serve:')).length === 0 && calls.filter(call => call.startsWith('image:')).length === 1, 'image reads directly without starting any background preview service')
  check(!document.querySelector('[aria-label="Open preview"]'), 'direct images do not offer an HTTP-only browser action')
  await drop('notes.md'); await until(() => document.querySelector('.artifact-preview-panel')?.textContent?.includes('Preview heading') && !document.querySelector('.artifact-preview-panel')?.textContent?.includes('# Preview heading'))
  check(true, 'Markdown opens rendered content')
  await drop('main.py'); await until(() => document.querySelector('.artifact-preview-panel')?.textContent?.includes('Hello from Workspace'))
  check(true, 'code opens readable content')
  let before = calls.length
  await drop('photo.png'); await until(() => document.querySelector<HTMLImageElement>('.artifact-preview-panel img')?.naturalWidth)
  check(calls.length === before && document.querySelectorAll('[role="tab"]').length === 3, 'dropping an open file selects its existing tab without reloading')
  document.querySelector<HTMLButtonElement>('[title="Reload content from file"]')!.click()
  await until(() => calls.length === before + 1)
  await until(() => document.querySelector<HTMLImageElement>('.artifact-preview-panel img')?.naturalWidth)
  check(calls.at(-1) === `image:${roots[0]}:photo.png` && !calls.some(call => call.startsWith('serve:')), 'Reload rereads the image directly without a service')
  before = calls.length
  await drop('archive.zip'); await until(() => document.querySelector('.workspace-file-info'))
  check(document.querySelector('.workspace-file-info')?.textContent?.includes('ZIP') && calls.length === before, 'unsupported files show details without decoding binary content')
  document.querySelector<HTMLButtonElement>('.workspace-file-info button')!.click()
  await until(() => externalTarget)
  check(externalTarget === 'file:///tmp/mint-drop-a/archive.zip', 'external-app button opens the original local file through the native boundary')
  await drop('broken.txt'); await until(() => document.querySelector('.tool-surface-drop-error'))
  check(document.querySelectorAll('[role="tab"]').length === 4, 'a failed preview leaves existing tabs intact')
  failRead = false; await drop('broken.txt'); await until(() => document.querySelectorAll('[role="tab"]').length === 5)
  check(!document.querySelector('.tool-surface-drop-error'), 'failed drops can be retried')
  await drop('index.html'); await until(() => document.querySelector('.artifact-preview-panel iframe'))
  const nextDrag = await begin('photo.png')
  check(document.querySelector('.tool-surface-drag-shield'), 'a drop shield remains available when the active preview is an iframe')
  tools().dispatchEvent(new DragEvent('dragenter', { bubbles: true, dataTransfer: nextDrag }))
  await delay(); window.dispatchEvent(new DragEvent('dragend'))
  await delay()
  check(!document.querySelector('.tool-surface-drag-shield') && !document.querySelector('.tool-surface-drop-overlay'), 'cancelling a drag clears all drag feedback')
  const folder = await begin('folder')
  check(!folder.types.includes(WORKSPACE_FILE_MIME) && !document.querySelector('.tool-surface-drag-shield'), 'folders are not treated as previewable files')
  window.dispatchEvent(new DragEvent('dragend'))
  document.querySelector<HTMLButtonElement>('#switch-root')!.click()
  await until(() => document.querySelector('.workspace-root-row')?.textContent?.includes('mint-drop-b'))
  await drop('photo.png'); await until(() => document.querySelectorAll('[role="tab"]').length === 7)
  check(calls.includes('image:/tmp/mint-drop-b:photo.png'), 'same filename from another Workspace opens its own pinned preview')
  check(document.querySelector<HTMLTextAreaElement>('textarea')?.value === 'Keep my draft', 'preview drops preserve the chat draft')
  const serverCalls = calls.filter(call => call.startsWith('serve:')).length
  await drop('guide.pdf')
  await until(() => document.querySelector<HTMLCanvasElement>('.workspace-pdf-preview canvas')?.width && document.querySelector('.workspace-pdf-controls')?.textContent?.includes('1 / 2') && !document.querySelector('.workspace-pdf-pages [role="status"]'))
  check(!document.querySelector('.workspace-pdf-pages [role="alert"]'), 'PDF renders inside Tools with the bundled viewer')
  document.querySelector<HTMLButtonElement>('[aria-label="Next PDF page"]')!.click()
  await until(() => document.querySelector('.workspace-pdf-controls')?.textContent?.includes('2 / 2') && !document.querySelector('.workspace-pdf-pages [role="status"]'))
  check(true, 'PDF next-page navigation renders the second page')
  document.querySelector<HTMLButtonElement>('[aria-label="Zoom PDF in"]')!.click()
  await until(() => document.querySelector('.workspace-pdf-controls')?.textContent?.includes('125%'))
  check(true, 'PDF zoom controls work')
  await drop('tone.wav')
  await until(() => document.querySelector<HTMLAudioElement>('audio')?.readyState! >= 1)
  const audio = document.querySelector<HTMLAudioElement>('audio')!
  check(audio.controls && !audio.autoplay && audio.duration > 0, 'audio decodes with controls and does not autoplay')
  const audioBlob = audio.querySelector('source')!.src
  audio.currentTime = .25
  await until(() => audio.currentTime >= .2)
  check(true, 'audio can seek within the file')
  await drop('clip.webm')
  await until(() => document.querySelector<HTMLVideoElement>('video')?.readyState! >= 1)
  const video = document.querySelector<HTMLVideoElement>('video')!
  check(video.controls && !video.autoplay && video.videoWidth > 0, 'video decodes with native controls and does not autoplay')
  check(audio.paused && !audio.querySelector('source')?.getAttribute('src'), 'switching tabs stops and releases the old media element')
  check(calls.filter(call => call.startsWith('serve:')).length === serverCalls, 'PDF, audio and video do not start preview services')
  document.querySelector<HTMLButtonElement>('[aria-label="Close tone.wav"]')!.click()
  await until(() => revoked.includes(audioBlob))
  check(true, 'closing the last media tab releases its Blob URL')
  for (const name of ['broken.wav', 'broken.webm']) {
    await drop(name)
    await until(() => document.querySelector('.workspace-media-error') && document.querySelector('.workspace-file-info button'))
    check(true, `${name} falls back to file details and an external-app button`)
  }
  if (location.search.includes('preview')) { document.getElementById('result')!.textContent = ''; return }
  document.getElementById('result')!.textContent = JSON.stringify({ status: 'PASS', checks: results })
}
run().catch(error => { document.getElementById('result')!.textContent = JSON.stringify({ status: 'FAIL', error: String(error), checks: results }) })
