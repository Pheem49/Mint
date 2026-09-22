import '../../src/css/tool-surfaces.css'
import { useState } from 'react'
import type { PointerEvent as ReactPointerEvent } from 'react'
import type { ArtifactFile } from './ArtifactPreviewPanel'
import { ArtifactPreviewPanel } from './ArtifactPreviewPanel'
import CodeReviewPage from './CodeReviewPage'
import TerminalDock from './TerminalDock'
import type { FileChange } from '../types'

export type ToolSurface =
  | { id: string; kind: 'preview'; title: string; artifact: ArtifactFile }
  | { id: string; kind: 'review'; title: string; reviewTitle: string; changes: FileChange[] }
  | { id: string; kind: 'terminal'; title: string }

interface Props {
  surfaces: ToolSurface[]
  activeSurfaceId: string | null
  workspacePath?: string | null
  terminalSize: number
  onSelect: (id: string) => void
  onClose: (id: string) => void
  onCloseAll: () => void
  onDuplicate: (id: string) => void
  onRename: (id: string) => void
  onCloseOthers: (id: string) => void
  onResizeTerminal: (size: number) => void
  onResizePanel: (size: number) => void
  onOpenTerminal: () => void
  onOpenBrowser: () => void
  onOpenReview: () => void
  onOpenFiles: () => void
  onOpenSideChat: () => void
  onClosePanel: () => void
}

function surfaceIcon(kind: ToolSurface['kind']) {
  if (kind === 'terminal') return '〉_'
  if (kind === 'review') return '▣'
  return '◫'
}

export default function ToolSurfacePage({
  surfaces,
  activeSurfaceId,
  workspacePath,
  terminalSize,
  onSelect,
  onClose,
  onCloseAll,
  onDuplicate,
  onRename,
  onCloseOthers,
  onResizeTerminal,
  onResizePanel,
  onOpenTerminal,
  onOpenBrowser,
  onOpenReview,
  onOpenFiles,
  onOpenSideChat,
  onClosePanel,
}: Props) {
  const active = surfaces.find((surface) => surface.id === activeSurfaceId) ?? null
  const [switcherOpen, setSwitcherOpen] = useState(false)
  const [openTabMenuId, setOpenTabMenuId] = useState<string | null>(null)

  const handlePanelResizeStart = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return
    event.preventDefault()
    const startX = event.clientX
    const startWidth = event.currentTarget.parentElement?.getBoundingClientRect().width ?? 440
    const handle = event.currentTarget
    handle.setPointerCapture?.(event.pointerId)
    const onMove = (moveEvent: PointerEvent) => {
      const maxWidth = Math.min(900, window.innerWidth - 120)
      const nextWidth = Math.max(300, Math.min(maxWidth, startWidth + startX - moveEvent.clientX))
      onResizePanel(Math.round(nextWidth))
    }
    const onUp = () => window.removeEventListener('pointermove', onMove)
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp, { once: true })
  }

  const launcherItems = [
    { label: 'Review', shortcut: 'Ctrl+Shift+G', icon: '⊞', className: 'is-review', action: onOpenReview },
    { label: 'Terminal', shortcut: 'Ctrl`', icon: '〉_', className: 'is-terminal', action: onOpenTerminal },
    { label: 'Browser', shortcut: 'Ctrl+T', icon: '◎', className: 'is-browser', action: onOpenBrowser },
    { label: 'Files', shortcut: 'Ctrl+P', icon: '▱', className: 'is-files', action: onOpenFiles },
    { label: 'Side chat', shortcut: 'Ctrl+Alt+S', icon: '◉', className: 'is-side-chat', action: onOpenSideChat },
  ]

  const renderLauncher = (className = '') => (
    <div className={`tool-surface-launcher ${className}`.trim()} role="menu" aria-label="Open a tool">
      {launcherItems.map((item) => (
        <button type="button" role="menuitem" className="tool-surface-launcher-item" key={item.label} onClick={() => { item.action(); setSwitcherOpen(false) }}>
          <span className={`tool-surface-menu-icon ${item.className}`} aria-hidden="true">{item.icon}</span>
          <span>{item.label}</span><kbd>{item.shortcut}</kbd>
        </button>
      ))}
    </div>
  )

  return (
    <section className="tool-surface-page" aria-label="Open tools">
      <div
        className="tool-surface-resize-handle"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize tools panel"
        title="Drag to resize tools panel"
        onPointerDown={handlePanelResizeStart}
      />
      {!active && (
        <header className="tool-surface-launcher-header">
          <span>Tools</span>
          <button type="button" onClick={onClosePanel} aria-label="Close tools panel" title="Close tools panel">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 6 9 12l6 6" /></svg>
          </button>
        </header>
      )}
      {active && (
        <header className="tool-surface-tabs">
          <button type="button" className="tool-surface-switcher" onClick={() => setSwitcherOpen((open) => !open)} aria-label="Switch tool" aria-expanded={switcherOpen} title="Switch tool">
            <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="16" height="16" rx="2"/><path d="M8 8h8M8 12h5M8 16h8"/></svg>
          </button>
          {switcherOpen && <div className="tool-surface-switch-menu">{renderLauncher('is-switch-menu')}</div>}
          <div className="tool-surface-tab-list" role="tablist" aria-label="Open tool pages">
            {surfaces.map((surface) => (
              <div className={`tool-surface-tab ${surface.id === active.id ? 'is-active' : ''}`} key={surface.id}>
                <button type="button" role="tab" aria-selected={surface.id === active.id} onClick={() => { setOpenTabMenuId(null); onSelect(surface.id) }}>
                  <span className={`tool-surface-tab-icon is-${surface.kind}`} aria-hidden="true">{surfaceIcon(surface.kind)}</span>
                  <span className="tool-surface-tab-title">{surface.title}</span>
                </button>
                <button type="button" className="tool-surface-tab-menu-trigger" onClick={(event) => { event.stopPropagation(); setOpenTabMenuId((current) => current === surface.id ? null : surface.id) }} aria-label={`More actions for ${surface.title}`} aria-expanded={openTabMenuId === surface.id} title="Tab actions">⋯</button>
                <button type="button" className="tool-surface-tab-close" onClick={() => onClose(surface.id)} aria-label={`Close ${surface.title}`} title="Close">×</button>
                {openTabMenuId === surface.id && (
                  <div className="tool-surface-tab-menu" role="menu">
                    <button type="button" role="menuitem" onClick={() => { onDuplicate(surface.id); setOpenTabMenuId(null) }}>Duplicate tab</button>
                    <button type="button" role="menuitem" onClick={() => { onRename(surface.id); setOpenTabMenuId(null) }}>Rename tab</button>
                    <button type="button" role="menuitem" onClick={() => { onCloseOthers(surface.id); setOpenTabMenuId(null) }}>Close other tabs</button>
                  </div>
                )}
              </div>
            ))}
          </div>
          <button type="button" className="tool-surface-close-all" onClick={onClosePanel} title="Close tools panel" aria-label="Close tools panel">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M3 6h18" /><path d="M8 6V4h8v2M19 6l-1 14H6L5 6" /><path d="M10 11v5M14 11v5" /></svg>
          </button>
        </header>
      )}

      <div className={`tool-surface-content${active ? '' : ' is-launcher'}`}>
        {!active && renderLauncher()}
        {active?.kind === 'preview' && <ArtifactPreviewPanel artifact={active.artifact} onClose={() => onClose(active.id)} workspacePath={workspacePath || undefined} />}
        {active?.kind === 'review' && <CodeReviewPage title={active.reviewTitle} changes={active.changes} onBack={() => onClose(active.id)} />}
        {surfaces.filter((surface): surface is Extract<ToolSurface, { kind: 'terminal' }> => surface.kind === 'terminal').map((surface) => (
          <TerminalDock
            key={surface.id}
            visible={surface.id === active.id}
            page
            size={terminalSize}
            cwd={workspacePath}
            onClose={() => onClose(surface.id)}
            onResize={onResizeTerminal}
          />
        ))}
      </div>
    </section>
  )
}
