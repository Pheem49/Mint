import { useEffect, useRef, useState } from 'react'
import '../../src/css/tool-surfaces.css'
import type { ArtifactFile } from './ArtifactPreviewPanel'
import { ArtifactPreviewPanel } from './ArtifactPreviewPanel'
import CodeReviewPage from './CodeReviewPage'
import TerminalDock from './TerminalDock'
import BrowserSurface from './BrowserSurface'
import type { FileChange } from '../types'

export type ToolSurface =
  | { id: string; kind: 'preview'; title: string; artifact: ArtifactFile }
  | { id: string; kind: 'review'; title: string; reviewTitle: string; changes: FileChange[] }
  | { id: string; kind: 'terminal'; title: string }
  | { id: string; kind: 'browser'; title: string; url: string }

interface Props {
  surfaces: ToolSurface[]
  activeSurfaceId: string
  workspacePath?: string | null
  terminalPosition: 'bottom' | 'right'
  terminalSize: number
  onSelect: (id: string) => void
  onClose: (id: string) => void
  onCloseAll: () => void
  onToggleTerminalPosition: () => void
  onResizeTerminal: (size: number) => void
  onOpenTerminal: () => void
  onOpenBrowser: () => void
  onNavigateBrowser: (id: string, url: string) => void
  onOpenExternalBrowser: (url: string) => void
}

function surfaceIcon(kind: ToolSurface['kind']) {
  if (kind === 'terminal') return '〉_'
  if (kind === 'review') return '▣'
  if (kind === 'browser') return '◎'
  return '◫'
}

export default function ToolSurfacePage({
  surfaces,
  activeSurfaceId,
  workspacePath,
  terminalPosition,
  terminalSize,
  onSelect,
  onClose,
  onCloseAll,
  onToggleTerminalPosition,
  onResizeTerminal,
  onOpenTerminal,
  onOpenBrowser,
  onNavigateBrowser,
  onOpenExternalBrowser,
}: Props) {
  const active = surfaces.find((surface) => surface.id === activeSurfaceId) ?? surfaces[0]
  const [addMenuOpen, setAddMenuOpen] = useState(false)
  const addMenuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!addMenuOpen) return
    const handlePointerDown = (event: PointerEvent) => {
      if (!addMenuRef.current?.contains(event.target as Node)) setAddMenuOpen(false)
    }
    document.addEventListener('pointerdown', handlePointerDown)
    return () => document.removeEventListener('pointerdown', handlePointerDown)
  }, [addMenuOpen])

  if (!active) return null

  const chooseAddAction = (action: () => void) => {
    setAddMenuOpen(false)
    action()
  }

  return (
    <section className="tool-surface-page" aria-label="Open tools">
      <header className="tool-surface-tabs">
        <div className="tool-surface-add-wrap" ref={addMenuRef}>
          <button
            type="button"
            className={`tool-surface-add ${addMenuOpen ? 'is-open' : ''}`}
            onClick={() => setAddMenuOpen((open) => !open)}
            aria-label="Open a new tool"
            aria-expanded={addMenuOpen}
            title="Open a new tool"
          >
            +
          </button>
          {addMenuOpen && (
            <div className="tool-surface-add-menu" role="menu" aria-label="Open a new tool">
              <button type="button" role="menuitem" onClick={() => chooseAddAction(onOpenTerminal)}>
                <span className="tool-surface-menu-icon is-terminal" aria-hidden="true">〉_</span>
                <span>Terminal</span><kbd>Ctrl`</kbd>
              </button>
              <button type="button" role="menuitem" onClick={() => chooseAddAction(onOpenBrowser)}>
                <span className="tool-surface-menu-icon is-browser" aria-hidden="true">◎</span>
                <span>Browser</span><kbd>Ctrl T</kbd>
              </button>
            </div>
          )}
        </div>
        <div className="tool-surface-tab-list" role="tablist" aria-label="Open tool pages">
          {surfaces.map((surface) => (
            <div className={`tool-surface-tab ${surface.id === active.id ? 'is-active' : ''}`} key={surface.id}>
              <button type="button" role="tab" aria-selected={surface.id === active.id} onClick={() => onSelect(surface.id)}>
                <span className={`tool-surface-tab-icon is-${surface.kind}`} aria-hidden="true">{surfaceIcon(surface.kind)}</span>
                <span className="tool-surface-tab-title">{surface.title}</span>
              </button>
              <button type="button" className="tool-surface-tab-close" onClick={() => onClose(surface.id)} aria-label={`Close ${surface.title}`} title="Close">
                ×
              </button>
            </div>
          ))}
        </div>
        <button type="button" className="tool-surface-close-all" onClick={onCloseAll} title="Close all open tools" aria-label="Close all open tools">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M3 6h18" /><path d="M8 6V4h8v2M19 6l-1 14H6L5 6" /><path d="M10 11v5M14 11v5" />
          </svg>
        </button>
      </header>

      <div className="tool-surface-content">
        {active.kind === 'preview' && <ArtifactPreviewPanel artifact={active.artifact} onClose={() => onClose(active.id)} workspacePath={workspacePath || undefined} />}
        {active.kind === 'review' && <CodeReviewPage title={active.reviewTitle} changes={active.changes} onBack={() => onClose(active.id)} />}
        {surfaces.filter((surface): surface is Extract<ToolSurface, { kind: 'terminal' }> => surface.kind === 'terminal').map((surface) => (
          <TerminalDock
            key={surface.id}
            visible={surface.id === active.id}
            page
            position={terminalPosition}
            size={terminalSize}
            cwd={workspacePath}
            onClose={() => onClose(surface.id)}
            onTogglePosition={onToggleTerminalPosition}
            onResize={onResizeTerminal}
          />
        ))}
        {active.kind === 'browser' && (
          <BrowserSurface
            key={active.id}
            url={active.url}
            onNavigate={(url) => onNavigateBrowser(active.id, url)}
            onClose={() => onClose(active.id)}
            onOpenExternal={onOpenExternalBrowser}
          />
        )}
      </div>
    </section>
  )
}
