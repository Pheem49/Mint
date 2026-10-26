import '../../../src/css/tool-surfaces.css'
import { useState, useEffect, useRef, useCallback, lazy, Suspense } from 'react'
import type { PointerEvent as ReactPointerEvent, ReactNode, DragEvent as ReactDragEvent } from 'react'
import { detectArtifactType, type ArtifactFile } from '../../utils/artifacts'
import TerminalDock from './TerminalDock'
import type { FileChange } from '../../types'
import { ListTodo, Paperclip, Puzzle, Image as ImageIcon, Layers, ChevronLeft, ChevronRight } from 'lucide-react'
import { WORKSPACE_FILE_MIME, parseWorkspaceFileDrag, type WorkspaceFileDrag } from '../../utils/workspaceDrag'

const ArtifactPreviewPanel = lazy(() => import('./ArtifactPreviewPanel').then(m => ({ default: m.ArtifactPreviewPanel })))
const CodeReviewPage = lazy(() => import('./CodeReviewPage'))

export type ToolSurface =
  | { id: string; kind: 'preview'; title: string; artifact: ArtifactFile }
  | { id: string; kind: 'review'; title: string; reviewTitle: string; changes: FileChange[] }
  | { id: string; kind: 'terminal'; title: string }
  | { id: string; kind: 'sources' | 'mods' | 'background'; title: string }

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
  desktopTools?: boolean
  backgroundTaskCount?: number
  sourceCount?: number
  sourcesContent?: ReactNode
  onOpenSources: () => void
  onOpenMods: () => void
  onOpenBackgroundTasks: () => void
  onModsPanelTarget: (target: HTMLDivElement | null) => void
  onBackgroundPanelTarget: (target: HTMLDivElement | null) => void
  onDropWorkspaceFile?: (file: WorkspaceFileDrag) => Promise<void>
}


function surfaceIcon(surface: ToolSurface | ToolSurface['kind']) {
  const kind = typeof surface === 'string' ? surface : surface.kind
  if (kind === 'sources') return <Paperclip size={13} aria-hidden="true" />
  if (kind === 'mods') return <Puzzle size={13} aria-hidden="true" />
  if (kind === 'background') return <ListTodo size={13} aria-hidden="true" />
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
  if (typeof surface === 'object' && surface.kind === 'preview') {
    const type = surface.artifact.type || detectArtifactType(surface.artifact.path)
    if (type === 'image') {
      return <ImageIcon size={13} strokeWidth={2.2} aria-hidden="true" style={{ verticalAlign: '-1px' }} />
    }
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
  desktopTools = true,
  backgroundTaskCount = 0,
  sourceCount = 0,
  sourcesContent,
  onOpenSources,
  onOpenMods,
  onOpenBackgroundTasks,
  onModsPanelTarget,
  onBackgroundPanelTarget,
  onDropWorkspaceFile,
}: Props) {
  const active = surfaces.find((surface) => surface.id === activeSurfaceId) ?? surfaces[0] ?? null
  const [switcherOpen, setSwitcherOpen] = useState(false)
  const [tabsSwitcherOpen, setTabsSwitcherOpen] = useState(false)
  const [tabsSwitcherAlignRight, setTabsSwitcherAlignRight] = useState(false)
  const tabsSwitcherWrapRef = useRef<HTMLDivElement>(null)
  const [tabMenuState, setTabMenuState] = useState<{ id: string; x: number; y: number } | null>(null)
  const [isMaximized, setIsMaximized] = useState(false)
  const panelRef = useRef<HTMLElement>(null)
  const [draggingFile, setDraggingFile] = useState(false)
  const [workspaceDragActive, setWorkspaceDragActive] = useState(false)
  const [dropError, setDropError] = useState('')
  const [openingFiles, setOpeningFiles] = useState(0)
  const dragDepth = useRef(0)
  const acceptsFile = (event: ReactDragEvent) => Boolean(onDropWorkspaceFile && event.dataTransfer.types.includes(WORKSPACE_FILE_MIME))
  const resetDrag = () => { dragDepth.current = 0; setDraggingFile(false); setWorkspaceDragActive(false) }
  useEffect(() => {
    const start = (event: DragEvent) => {
      if (onDropWorkspaceFile && event.dataTransfer?.types.includes(WORKSPACE_FILE_MIME)) setWorkspaceDragActive(true)
    }
    window.addEventListener('dragstart', start)
    window.addEventListener('dragend', resetDrag)
    window.addEventListener('drop', resetDrag)
    return () => { window.removeEventListener('dragstart', start); window.removeEventListener('dragend', resetDrag); window.removeEventListener('drop', resetDrag) }
  }, [onDropWorkspaceFile])
  const dropFile = async (event: ReactDragEvent) => {
    event.preventDefault()
    const raw = event.dataTransfer.getData(WORKSPACE_FILE_MIME)
    resetDrag()
    if (!onDropWorkspaceFile || !raw) return
    event.stopPropagation()
    setDropError('')
    setOpeningFiles(count => count + 1)
    try { await onDropWorkspaceFile(parseWorkspaceFileDrag(raw)) }
    catch (reason) { setDropError(reason instanceof Error ? reason.message : String(reason)) }
    finally { setOpeningFiles(count => count - 1) }
  }

  useEffect(() => {
    panelRef.current?.querySelector<HTMLButtonElement>('[role="tab"][aria-selected="true"], [role="menuitem"]')?.focus({ preventScroll: true })
  }, [active?.id])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return

      if (event.key === 'Escape') {
        if (!panelRef.current?.contains(event.target as Node)) return
        event.preventDefault()
        if (tabsSwitcherOpen) {
          setTabsSwitcherOpen(false)
          tabsSwitcherWrapRef.current?.querySelector<HTMLButtonElement>('.tool-surface-tabs-switcher-btn')?.focus()
        } else if (switcherOpen) {
          setSwitcherOpen(false)
          newTabWrapRef.current?.querySelector<HTMLButtonElement>('.tool-surface-dropdown-btn')?.focus()
        } else if (tabMenuState) {
          setTabMenuState(null)
        } else {
          onClosePanel()
        }
        return
      }

      if (surfaces.length > 1) {
        const isPrev = (event.altKey && event.key === '[') || (event.ctrlKey && event.key === 'PageUp')
        const isNext = (event.altKey && event.key === ']') || (event.ctrlKey && event.key === 'PageDown')
        if (isPrev || isNext) {
          const activeEl = document.activeElement
          const isInsidePanel = panelRef.current?.contains(activeEl) || activeEl === document.body
          if (isInsidePanel) {
            event.preventDefault()
            const currentIndex = surfaces.findIndex((s) => s.id === (active?.id ?? ''))
            if (currentIndex !== -1) {
              const targetIndex = isNext
                ? (currentIndex + 1) % surfaces.length
                : (currentIndex - 1 + surfaces.length) % surfaces.length
              onSelect(surfaces[targetIndex].id)
            }
          }
        }
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [onClosePanel, switcherOpen, tabMenuState, tabsSwitcherOpen, surfaces, active?.id, onSelect])

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
    { label: 'Sources', title: 'Session sources', shortcut: '', icon: <Paperclip />, className: 'is-sources', action: onOpenSources },
    { label: 'Mods', title: 'Workspace mods', shortcut: '', icon: <Puzzle />, className: 'is-mods', action: onOpenMods },
    { label: 'Background tasks', title: 'Background tasks', shortcut: '', icon: <ListTodo />, className: 'is-background', action: onOpenBackgroundTasks },
    ...(desktopTools ? [
    { label: 'Review', title: 'Review changes', shortcut: 'Ctrl+Shift+G', icon: <IconReview />, className: 'is-review', action: onOpenReview },
    { label: 'Terminal', title: 'Integrated terminal', shortcut: 'Ctrl + `', icon: <IconTerminal />, className: 'is-terminal', action: onOpenTerminal },
    { label: 'Browser', title: 'Browser (Co-browsing with AI)', shortcut: 'Ctrl+T', icon: <IconBrowser />, className: 'is-browser', action: onOpenBrowser },
    { label: 'Workspace', title: 'Workspace files', shortcut: 'Ctrl+P', icon: <IconFiles />, className: 'is-files', action: onOpenFiles },
    ] : []),
  ]

  const renderLauncher = (className = '') => (
    <div className={`tool-surface-launcher ${className}`.trim()} role="menu" aria-label="Open a tool">
      {launcherItems.map((item) => (
        <button type="button" role="menuitem" className="tool-surface-launcher-item" key={item.label} title={item.title} onClick={() => { item.action(); setSwitcherOpen(false) }}>
          <span className={`tool-surface-menu-icon ${item.className}`} aria-hidden="true">{item.icon}</span>
          <span className="tool-surface-menu-copy"><span className="tool-surface-menu-label">{item.label}</span></span>
          {item.className === 'is-sources' && <span className="tools-task-count" aria-label={`${sourceCount} session sources`}>{sourceCount}</span>}
          {item.className === 'is-background' && backgroundTaskCount > 0 && <span className="tools-task-count" aria-label={`${backgroundTaskCount} active background tasks`}>{backgroundTaskCount}</span>}
          {item.shortcut && <kbd>{item.shortcut}</kbd>}
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
        setMenuAlignRight(window.innerWidth - rect.left < 260)
      }
      return next
    })
  }

  const toggleTabsSwitcher = () => {
    setTabsSwitcherOpen((open) => {
      const next = !open
      if (next && tabsSwitcherWrapRef.current) {
        const rect = tabsSwitcherWrapRef.current.getBoundingClientRect()
        setTabsSwitcherAlignRight(window.innerWidth - rect.left < 310)
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
    if (!tabsSwitcherOpen) return
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null
      if (target && !target.closest('.tool-surface-tabs-switcher-wrap')) {
        setTabsSwitcherOpen(false)
      }
    }
    window.addEventListener('mousedown', handleClickOutside)
    return () => window.removeEventListener('mousedown', handleClickOutside)
  }, [tabsSwitcherOpen])

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
  const [hasOverflow, setHasOverflow] = useState(false)
  const [canScrollLeft, setCanScrollLeft] = useState(false)
  const [canScrollRight, setCanScrollRight] = useState(false)

  const checkTabScroll = useCallback(() => {
    const el = tabListRef.current
    if (!el) return
    const overflow = el.scrollWidth > el.clientWidth + 2
    setHasOverflow(overflow)
    setCanScrollLeft(overflow && el.scrollLeft > 2)
    setCanScrollRight(overflow && el.scrollLeft + el.clientWidth < el.scrollWidth - 2)
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
    const amount = direction === 'left' ? -200 : 200
    tabListRef.current.scrollBy({ left: amount, behavior: 'smooth' })
  }

  const handleTabWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (e.deltaY !== 0) {
      e.currentTarget.scrollLeft += e.deltaY
    } else if (e.deltaX !== 0) {
      e.currentTarget.scrollLeft += e.deltaX
    }
  }

  const handleAddNewTab = () => {
    if (active?.kind === 'sources' || active?.kind === 'mods' || active?.kind === 'background') {
      toggleSwitcher()
      return
    }
    if (active?.kind === 'review') {
      onOpenReview()
    } else if (active?.kind === 'preview') {
      onDuplicate(active.id)
    } else {
      onOpenTerminal()
    }
  }

  const newTabTooltip = active?.kind === 'sources' || active?.kind === 'mods' || active?.kind === 'background' ? 'Open a tool' : active?.kind === 'review'
    ? 'New Review Tab'
    : active?.kind === 'preview'
    ? 'Duplicate Preview Tab'
    : 'New Terminal Tab (Ctrl+`)'

  return (
    <section
      ref={panelRef}
      className={`tool-surface-page ${isEntering ? 'is-entering' : ''} ${isMaximized ? 'is-maximized' : ''} ${isResizing ? 'is-resizing' : ''}`}
      onAnimationEnd={() => setIsEntering(false)}
      aria-label="Tools"
      onDragEnter={event => {
        if (!acceptsFile(event)) return
        event.preventDefault(); dragDepth.current++; setDraggingFile(true)
      }}
      onDragOver={event => {
        if (!acceptsFile(event)) return
        event.preventDefault(); event.dataTransfer.dropEffect = 'copy'
      }}
      onDragLeave={() => {
        dragDepth.current = Math.max(0, dragDepth.current - 1)
        if (!dragDepth.current) setDraggingFile(false)
      }}
      onDrop={event => { void dropFile(event) }}
    >
      {workspaceDragActive && <div className="tool-surface-drag-shield" aria-hidden="true" />}
      {draggingFile && <div className="tool-surface-drop-overlay" role="status"><Paperclip size={24} aria-hidden="true" /><span>Drop to preview</span></div>}
      {openingFiles > 0 && <div className="tool-surface-drop-status" role="status">Opening file…</div>}
      {dropError && <div className="tool-surface-drop-error" role="alert">Could not open preview: {dropError}</div>}
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
            {hasOverflow && (
              <button
                type="button"
                className={`tool-surface-tab-scroll-btn is-left ${!canScrollLeft ? 'is-disabled' : ''}`}
                onClick={() => scrollTabs('left')}
                disabled={!canScrollLeft}
                aria-label="Scroll tabs left"
                title="Scroll tabs left"
              >
                <ChevronLeft size={14} strokeWidth={2.4} aria-hidden="true" />
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
                  <button
                    type="button"
                    role="tab"
                    aria-selected={surface.id === active.id}
                    title={surface.title}
                    aria-label={surface.title}
                    onClick={() => { setTabMenuState(null); onSelect(surface.id) }}
                  >
                    <span className={`tool-surface-tab-icon is-${surface.kind}`} aria-hidden="true">{surfaceIcon(surface)}</span>
                    <span className="tool-surface-tab-title">{surface.title}</span>
                  </button>
                  <button type="button" className="tool-surface-tab-close" onClick={() => onClose(surface.id)} aria-label={`Close ${surface.title}`} title="Close">×</button>
                </div>
              ))}
            </div>
            {hasOverflow && (
              <button
                type="button"
                className={`tool-surface-tab-scroll-btn is-right ${!canScrollRight ? 'is-disabled' : ''}`}
                onClick={() => scrollTabs('right')}
                disabled={!canScrollRight}
                aria-label="Scroll tabs right"
                title="Scroll tabs right"
              >
                <ChevronRight size={14} strokeWidth={2.4} aria-hidden="true" />
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
              aria-haspopup="menu"
              aria-expanded={switcherOpen}
            >
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="m6 9 6 6 6-6" />
              </svg>
            </button>
            {switcherOpen && (
              <div
                className={`tool-surface-switch-menu ${menuAlignRight ? 'is-align-right' : ''}`}
                aria-label="Tool menu"
              >
                <div className="tool-surface-menu-heading">OPEN TOOL</div>
                {renderLauncher('is-switch-menu')}
              </div>
            )}
          </div>
          {surfaces.length > 0 && (
            <div className="tool-surface-tabs-switcher-wrap" ref={tabsSwitcherWrapRef}>
              <button
                type="button"
                className={`tool-surface-tabs-switcher-btn ${tabsSwitcherOpen ? 'is-active' : ''}`}
                onClick={(e) => { e.stopPropagation(); toggleTabsSwitcher() }}
                title={`All open tabs (${surfaces.length})`}
                aria-label={`All open tabs (${surfaces.length})`}
                aria-haspopup="menu"
                aria-expanded={tabsSwitcherOpen}
              >
                <Layers size={13} strokeWidth={2.2} aria-hidden="true" />
                <span className="tool-surface-tabs-badge">{surfaces.length}</span>
              </button>
              {tabsSwitcherOpen && (
                <div
                  className={`tool-surface-tabs-dropdown-menu ${tabsSwitcherAlignRight ? 'is-align-right' : ''}`}
                  aria-label="Open tabs list"
                  role="menu"
                >
                  <div className="tool-surface-tabs-dropdown-header">
                    <span>Open Tabs ({surfaces.length})</span>
                    <span className="tool-surface-tabs-dropdown-hint">Alt+[ / Alt+]</span>
                  </div>
                  <div className="tool-surface-tabs-dropdown-list">
                    {surfaces.map((s) => {
                      const isActive = s.id === active.id
                      return (
                        <div
                          key={s.id}
                          className={`tool-surface-tabs-dropdown-item ${isActive ? 'is-active' : ''}`}
                          onClick={() => {
                            onSelect(s.id)
                            setTabsSwitcherOpen(false)
                          }}
                          role="menuitem"
                          tabIndex={0}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' || e.key === ' ') {
                              e.preventDefault()
                              onSelect(s.id)
                              setTabsSwitcherOpen(false)
                            }
                          }}
                        >
                          <span className={`tool-surface-tab-icon is-${s.kind} tool-surface-tabs-dropdown-item-icon`} aria-hidden="true">
                            {surfaceIcon(s)}
                          </span>
                          <span className="tool-surface-tabs-dropdown-item-title" title={s.title}>
                            {s.title}
                          </span>
                          <button
                            type="button"
                            className="tool-surface-tabs-dropdown-item-close"
                            onClick={(e) => {
                              e.stopPropagation()
                              onClose(s.id)
                            }}
                            title={`Close ${s.title}`}
                            aria-label={`Close ${s.title}`}
                          >
                            ×
                          </button>
                        </div>
                      )
                    })}
                  </div>
                  <div className="tool-surface-tabs-dropdown-footer">
                    <button
                      type="button"
                      className="tool-surface-tabs-dropdown-footer-btn"
                      onClick={() => {
                        onCloseAll()
                        setTabsSwitcherOpen(false)
                      }}
                    >
                      Close all tabs
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}
          <div className="tool-surface-header-right">
            <div className="tool-surface-header-actions">
              <button
                type="button"
                className={`tool-surface-action-btn ${isMaximized ? 'is-active' : ''}`}
                onClick={() => setIsMaximized((prev) => !prev)}
                title={isMaximized ? 'Restore side split' : 'Maximize tools (⤢)'}
                aria-label={isMaximized ? 'Restore side split' : 'Maximize tools'}
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
          {tabMenuState && (() => {
            const currentIdx = surfaces.findIndex(s => s.id === tabMenuState.id)
            const hasTabsToRight = currentIdx >= 0 && currentIdx < surfaces.length - 1
            const targetSurface = surfaces.find(surface => surface.id === tabMenuState.id)
            return (
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
                {targetSurface?.kind !== 'sources' && targetSurface?.kind !== 'mods' && targetSurface?.kind !== 'background' && (
                  <button type="button" role="menuitem" onClick={() => { onDuplicate(tabMenuState.id); setTabMenuState(null) }}>
                    Duplicate tab
                  </button>
                )}
                <button type="button" role="menuitem" onClick={() => { onRename(tabMenuState.id); setTabMenuState(null) }}>
                  Rename tab
                </button>
                {hasTabsToRight && (
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      const tabsToRight = surfaces.slice(currentIdx + 1)
                      tabsToRight.forEach(s => onClose(s.id))
                      setTabMenuState(null)
                    }}
                  >
                    Close tabs to the right
                  </button>
                )}
                <button type="button" role="menuitem" onClick={() => { onCloseOthers(tabMenuState.id); setTabMenuState(null) }}>
                  Close other tabs
                </button>
                <button type="button" role="menuitem" onClick={() => { onClose(tabMenuState.id); setTabMenuState(null) }}>
                  Close tab
                </button>
                <hr />
                <button
                  type="button"
                  role="menuitem"
                  className="is-destructive"
                  onClick={() => { onCloseAll(); setTabMenuState(null) }}
                >
                  Close all tabs
                </button>
              </div>
            )
          })()}
        </header>
      )}

      <div className={`tool-surface-content${active ? '' : ' is-launcher'}`}>
        {!active && renderLauncher()}
        {active?.kind === 'sources' && <div className="tool-surface-sources">{sourcesContent}</div>}
        {active?.kind === 'mods' && <div key="mods" className="tool-surface-panel-host" ref={onModsPanelTarget} />}
        {active?.kind === 'background' && <div key="background" className="tool-surface-panel-host" ref={onBackgroundPanelTarget} />}
        {active?.kind === 'preview' && (
          <Suspense fallback={null}>
            <ArtifactPreviewPanel key={active.id} artifact={active.artifact} onClose={() => active && onClose(active.id)} workspacePath={workspacePath || undefined} />
          </Suspense>
        )}
        {active?.kind === 'review' && (
          <Suspense fallback={null}>
            <CodeReviewPage
              title={active.reviewTitle}
              changes={active.changes}
              workspacePath={workspacePath}
              onBack={() => active && onClose(active.id)}
              onOpenFiles={onOpenFiles}
            />
          </Suspense>
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
