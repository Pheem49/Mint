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
  onOpenSideChat?: () => void
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

  const IconReview = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="3" y="3" width="18" height="18" rx="3" />
      <path d="M3 9h18M9 21V9" />
    </svg>
  )

  const IconTerminal = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="3" y="4" width="18" height="16" rx="3" />
      <path d="m7 10 3 2-3 2M13 14h4" />
    </svg>
  )

  const IconBrowser = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 3a15 15 0 0 0 0 18M12 3a15 15 0 0 1 0 18M3 12h18" />
    </svg>
  )

  const IconFiles = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </svg>
  )

  const launcherItems = [
    { label: 'Review', title: 'Review changes', shortcut: 'Ctrl+Shift+G', icon: <IconReview />, className: 'is-review', action: onOpenReview },
    { label: 'Terminal', title: 'Integrated terminal', shortcut: 'Ctrl`', icon: <IconTerminal />, className: 'is-terminal', action: onOpenTerminal },
    { label: 'Browser', title: 'Browser (Co-browsing with AI)', shortcut: 'Ctrl+T', icon: <IconBrowser />, className: 'is-browser', action: onOpenBrowser },
    { label: 'Workspace', title: 'Workspace files', shortcut: 'Ctrl+P', icon: <IconFiles />, className: 'is-files', action: onOpenFiles },
  ]

  const renderLauncher = (className = '') => (
    <div className={`tool-surface-launcher ${className}`.trim()} role="menu" aria-label="Open a tool">
      {launcherItems.map((item) => (
        <button type="button" role="menuitem" className="tool-surface-launcher-item" key={item.label} title={item.title} onClick={() => { item.action(); setSwitcherOpen(false) }}>
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
        {active?.kind === 'review' && (
          <CodeReviewPage
            title={active.reviewTitle}
            changes={active.changes}
            workspacePath={workspacePath}
            onBack={() => onClose(active.id)}
            onOpenFiles={onOpenFiles}
          />
        )}
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
