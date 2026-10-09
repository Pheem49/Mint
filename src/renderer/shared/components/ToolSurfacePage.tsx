import '../../src/css/tool-surfaces.css'
import { useState, useEffect, useRef, useCallback } from 'react'
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
  if (kind === 'terminal') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={{ verticalAlign: '-1px' }}>
        <path d="m5 7 5 5-5 5M12 17h7" />
      </svg>
    )
  }
  if (kind === 'review') {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={{ verticalAlign: '-1px' }}>
        <circle cx="5" cy="6" r="3" />
        <path d="M5 9v12" />
        <path d="m15 9-3-3 3-3" />
        <path d="M12 6h5a2 2 0 0 1 2 2v7" />
        <circle cx="19" cy="18" r="3" />
      </svg>
    )
  }
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={{ verticalAlign: '-1px' }}>
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
    </svg>
  )
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
  const active = surfaces.find((surface) => surface.id === activeSurfaceId) ?? surfaces[0] ?? null
  const [switcherOpen, setSwitcherOpen] = useState(false)
  const [tabMenuState, setTabMenuState] = useState<{ id: string; x: number; y: number } | null>(null)
  const [isMaximized, setIsMaximized] = useState(false)

  const [isEntering, setIsEntering] = useState(true)
  const [isResizing, setIsResizing] = useState(false)
  const resizeCleanupRef = useRef<(() => void) | null>(null)

  const handlePanelResizeStart = (event: React.MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0) return
    event.preventDefault()
    event.stopPropagation()

    const startX = event.clientX
    const startWidth = terminalSize || event.currentTarget.parentElement?.getBoundingClientRect().width || 480
    const prevCursor = document.body.style.cursor
    const prevUserSelect = document.body.style.userSelect

    document.body.style.cursor = 'col-resize'
    document.body.style.userSelect = 'none'
    document.body.classList.add('is-tool-resizing')
    setIsResizing(true)

    let frameId: number | null = null
    let latestWidth = startWidth

    const onMove = (moveEvent: MouseEvent) => {
      const deltaX = startX - moveEvent.clientX
      const maxWidth = Math.min(1200, window.innerWidth - 120)
      latestWidth = Math.max(280, Math.min(maxWidth, startWidth + deltaX))

      document.documentElement.style.setProperty('--tool-surface-width', `${Math.round(latestWidth)}px`)

      if (frameId == null) {
        frameId = requestAnimationFrame(() => {
          frameId = null
          onResizePanel(Math.round(latestWidth))
        })
      }
    }

    const onUp = () => {
      if (frameId != null) {
        cancelAnimationFrame(frameId)
        frameId = null
      }
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
      document.body.style.cursor = prevCursor
      document.body.style.userSelect = prevUserSelect
      document.body.classList.remove('is-tool-resizing')
      document.documentElement.style.removeProperty('--tool-surface-width')
      resizeCleanupRef.current = null
      setIsResizing(false)
      onResizePanel(Math.round(latestWidth))
    }

    resizeCleanupRef.current?.()
    resizeCleanupRef.current = onUp
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

  const handleResizeKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowLeft') {
      event.preventDefault()
      onResizePanel(Math.min(1200, (terminalSize || 480) + 24))
    } else if (event.key === 'ArrowRight') {
      event.preventDefault()
      onResizePanel(Math.max(280, (terminalSize || 480) - 24))
    }
  }

  useEffect(() => {
    return () => {
      resizeCleanupRef.current?.()
      document.body.classList.remove('is-tool-resizing')
    }
  }, [])

  const IconReview = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="5" cy="6" r="3" />
      <path d="M5 9v12" />
      <path d="m15 9-3-3 3-3" />
      <path d="M12 6h5a2 2 0 0 1 2 2v7" />
      <circle cx="19" cy="18" r="3" />
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

  const [menuAlignRight, setMenuAlignRight] = useState(false)
  const newTabWrapRef = useRef<HTMLDivElement>(null)

  const toggleSwitcher = () => {
    setSwitcherOpen((open) => {
      const next = !open
      if (next && newTabWrapRef.current) {
        const rect = newTabWrapRef.current.getBoundingClientRect()
        setMenuAlignRight(window.innerWidth - rect.left < 210)
      }
      return next
    })
  }

  useEffect(() => {
    if (!switcherOpen) return
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null
      if (target && !target.closest('.tool-surface-new-tab-wrap')) {
        setSwitcherOpen(false)
      }
    }
    window.addEventListener('mousedown', handleClickOutside)
    return () => window.removeEventListener('mousedown', handleClickOutside)
  }, [switcherOpen])

  useEffect(() => {
    if (!tabMenuState) return
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null
      if (target && !target.closest('.tool-surface-tab-menu')) {
        setTabMenuState(null)
      }
    }
    window.addEventListener('mousedown', handleClickOutside)
    return () => window.removeEventListener('mousedown', handleClickOutside)
  }, [tabMenuState])

  const tabListRef = useRef<HTMLDivElement>(null)
  const [canScrollLeft, setCanScrollLeft] = useState(false)
  const [canScrollRight, setCanScrollRight] = useState(false)

  const checkTabScroll = useCallback(() => {
    const el = tabListRef.current
    if (!el) return
    const hasOverflow = el.scrollWidth > el.clientWidth + 2
    setCanScrollLeft(hasOverflow && el.scrollLeft > 2)
    setCanScrollRight(hasOverflow && el.scrollLeft + el.clientWidth < el.scrollWidth - 2)
  }, [])

  useEffect(() => {
    checkTabScroll()
    const el = tabListRef.current
    if (!el) return
    el.addEventListener('scroll', checkTabScroll, { passive: true })
    const ro = new ResizeObserver(checkTabScroll)
    ro.observe(el)
    return () => {
      el.removeEventListener('scroll', checkTabScroll)
      ro.disconnect()
    }
  }, [checkTabScroll, surfaces.length])

  // Scroll active tab into view when active surface changes
  useEffect(() => {
    if (!active?.id || !tabListRef.current) return
    const activeEl = tabListRef.current.querySelector<HTMLElement>('.tool-surface-tab.is-active')
    if (activeEl) {
      activeEl.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' })
    }
    checkTabScroll()
  }, [active?.id, checkTabScroll])

  const scrollTabs = (direction: 'left' | 'right') => {
    if (!tabListRef.current) return
    const amount = direction === 'left' ? -180 : 180
    tabListRef.current.scrollBy({ left: amount, behavior: 'smooth' })
  }

  const handleTabWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (e.deltaY !== 0) {
      e.currentTarget.scrollLeft += e.deltaY
    }
  }

  const handleAddNewTab = () => {
    if (active?.kind === 'review') {
      onOpenReview()
    } else if (active?.kind === 'preview') {
      onDuplicate(active.id)
    } else {
      onOpenTerminal()
    }
  }

  const newTabTooltip = active?.kind === 'review'
    ? 'New Review Tab'
    : active?.kind === 'preview'
    ? 'Duplicate Preview Tab'
    : 'New Terminal Tab (Ctrl+`)'

  return (
    <section
      className={`tool-surface-page ${isEntering ? 'is-entering' : ''} ${isMaximized ? 'is-maximized' : ''} ${isResizing ? 'is-resizing' : ''}`}
      onAnimationEnd={() => setIsEntering(false)}
      aria-label="Open tools"
    >
      {!isMaximized && (
        <div
          className={`tool-surface-resize-handle ${isResizing ? 'is-resizing' : ''}`}
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize tools panel"
          aria-valuenow={terminalSize}
          tabIndex={0}
          title="Drag to resize tools panel"
          onMouseDown={handlePanelResizeStart}
          onKeyDown={handleResizeKeyDown}
        />
      )}
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
          <div className="tool-surface-tabs-container">
            {canScrollLeft && (
              <button
                type="button"
                className="tool-surface-tab-scroll-btn is-left"
                onClick={() => scrollTabs('left')}
                aria-label="Scroll tabs left"
                title="Scroll tabs left"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="m15 18-6-6 6-6" />
                </svg>
              </button>
            )}
            <div
              className="tool-surface-tab-list"
              ref={tabListRef}
              onWheel={handleTabWheel}
              role="tablist"
              aria-label="Open tool pages"
            >
              {surfaces.map((surface) => (
                <div
                  className={`tool-surface-tab ${surface.id === active.id ? 'is-active' : ''}`}
                  key={surface.id}
                  onContextMenu={(event) => {
                    event.preventDefault()
                    event.stopPropagation()
                    setTabMenuState({
                      id: surface.id,
                      x: Math.min(event.clientX, window.innerWidth - 180),
                      y: Math.min(event.clientY, window.innerHeight - 150),
                    })
                  }}
                >
                  <button type="button" role="tab" aria-selected={surface.id === active.id} onClick={() => { setTabMenuState(null); onSelect(surface.id) }}>
                    <span className={`tool-surface-tab-icon is-${surface.kind}`} aria-hidden="true">{surfaceIcon(surface.kind)}</span>
                    <span className="tool-surface-tab-title">{surface.title}</span>
                  </button>
                  <button type="button" className="tool-surface-tab-close" onClick={() => onClose(surface.id)} aria-label={`Close ${surface.title}`} title="Close">×</button>
                </div>
              ))}
            </div>
            {canScrollRight && (
              <button
                type="button"
                className="tool-surface-tab-scroll-btn is-right"
                onClick={() => scrollTabs('right')}
                aria-label="Scroll tabs right"
                title="Scroll tabs right"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="m9 18 6-6-6-6" />
                </svg>
              </button>
            )}
          </div>
          <div className="tool-surface-new-tab-wrap" ref={newTabWrapRef}>
            <button
              type="button"
              className="tool-surface-new-tab-btn"
              onClick={handleAddNewTab}
              onContextMenu={(e) => { e.preventDefault(); toggleSwitcher() }}
              title={newTabTooltip}
              aria-label={newTabTooltip}
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M12 5v14M5 12h14" />
              </svg>
            </button>
            <button
              type="button"
              className={`tool-surface-dropdown-btn ${switcherOpen ? 'is-active' : ''}`}
              onClick={(e) => { e.stopPropagation(); toggleSwitcher() }}
              title="Switch tool or open page"
              aria-label="Switch tool"
              aria-expanded={switcherOpen}
            >
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="m6 9 6 6 6-6" />
              </svg>
            </button>
            {switcherOpen && (
              <div
                className={`tool-surface-switch-menu ${menuAlignRight ? 'is-align-right' : ''}`}
                role="menu"
              >
                {renderLauncher('is-switch-menu')}
              </div>
            )}
          </div>
          <div className="tool-surface-header-right">
            <div className="tool-surface-header-actions">
              <button
                type="button"
                className={`tool-surface-action-btn ${isMaximized ? 'is-active' : ''}`}
                onClick={() => setIsMaximized((prev) => !prev)}
                title={isMaximized ? 'Restore side split' : 'Maximize terminal (⤢)'}
                aria-label={isMaximized ? 'Restore side split' : 'Maximize terminal'}
              >
                {isMaximized ? (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="M8 3v3a2 2 0 0 1-2 2H3m18 0h-3a2 2 0 0 1-2-2V3m0 18v-3a2 2 0 0 1 2-2h3M3 16h3a2 2 0 0 1 2 2v3" />
                  </svg>
                ) : (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />
                  </svg>
                )}
              </button>
              <button
                type="button"
                className="tool-surface-action-btn is-close"
                onClick={onClosePanel}
                title="Close tools panel"
                aria-label="Close tools panel"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M18 6 6 18M6 6l12 12" />
                </svg>
              </button>
            </div>
          </div>
          {tabMenuState && (
            <div
              className="tool-surface-tab-menu is-floating"
              style={{
                position: 'fixed',
                top: `${tabMenuState.y}px`,
                left: `${tabMenuState.x}px`,
                zIndex: 1000,
              }}
              role="menu"
            >
              <button type="button" role="menuitem" onClick={() => { onDuplicate(tabMenuState.id); setTabMenuState(null) }}>
                Duplicate tab
              </button>
              <button type="button" role="menuitem" onClick={() => { onRename(tabMenuState.id); setTabMenuState(null) }}>
                Rename tab
              </button>
              <button type="button" role="menuitem" onClick={() => { onCloseOthers(tabMenuState.id); setTabMenuState(null) }}>
                Close other tabs
              </button>
              <button type="button" role="menuitem" onClick={() => { onClose(tabMenuState.id); setTabMenuState(null) }}>
                Close tab
              </button>
            </div>
          )}
        </header>
      )}

      <div className={`tool-surface-content${active ? '' : ' is-launcher'}`}>
        {!active && renderLauncher()}
        {active?.kind === 'preview' && <ArtifactPreviewPanel key={active.id} artifact={active.artifact} onClose={() => active && onClose(active.id)} workspacePath={workspacePath || undefined} />}
        {active?.kind === 'review' && (
          <CodeReviewPage
            title={active.reviewTitle}
            changes={active.changes}
            workspacePath={workspacePath}
            onBack={() => active && onClose(active.id)}
            onOpenFiles={onOpenFiles}
          />
        )}
        {surfaces.filter((surface): surface is Extract<ToolSurface, { kind: 'terminal' }> => surface.kind === 'terminal').map((surface) => (
          <TerminalDock
            key={surface.id}
            visible={Boolean(active && surface.id === active.id)}
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
