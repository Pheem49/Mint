import { useEffect, useRef, useState } from 'react'

type ResizeDirection = 'East' | 'North' | 'NorthEast' | 'NorthWest' | 'South' | 'SouthEast' | 'SouthWest' | 'West'

type MenuName = 'File' | 'Edit' | 'View' | 'Help'

interface DesktopTitlebarProps {
  sidebarCollapsed: boolean
  onToggleSidebar: () => void
  onNewChat: () => void
  onOpenWorkspace: () => void
  onOpenTerminal: () => void
  onToggleTerminal: () => void
  onOpenBrowser: () => void
  onOpenSettings: () => void
  onCheckForUpdates: () => void
  onShowAbout: () => void
}

const MENU_NAMES: MenuName[] = ['File', 'Edit', 'View', 'Help']
const RESIZE_DIRECTIONS: ResizeDirection[] = ['NorthWest', 'North', 'NorthEast', 'West', 'East', 'SouthWest', 'South', 'SouthEast']

export default function DesktopTitlebar({
  sidebarCollapsed,
  onToggleSidebar,
  onNewChat,
  onOpenWorkspace,
  onOpenTerminal,
  onToggleTerminal,
  onOpenBrowser,
  onOpenSettings,
  onCheckForUpdates,
  onShowAbout,
}: DesktopTitlebarProps) {
  const [openMenu, setOpenMenu] = useState<MenuName | null>(null)
  const [isMaximized, setIsMaximized] = useState(false)
  const [isFullscreen, setIsFullscreen] = useState(false)
  const [zoom, setZoom] = useState(() => Number(window.localStorage.getItem('mint:ui-zoom')) || 1)
  const [shortcutsOpen, setShortcutsOpen] = useState(false)
  const [hasEditableFocus, setHasEditableFocus] = useState(false)
  const rootRef = useRef<HTMLElement>(null)

  useEffect(() => {
    let mounted = true
    void import('@tauri-apps/api/window')
      .then(async ({ getCurrentWindow }) => {
        const currentWindow = getCurrentWindow()
        const [maximized, fullscreen] = await Promise.all([currentWindow.isMaximized(), currentWindow.isFullscreen()])
        if (mounted) {
          setIsMaximized(maximized)
          setIsFullscreen(fullscreen)
        }
      })
      .catch(() => {})
    return () => { mounted = false }
  }, [])

  const minimizeWindow = () => {
    void import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().minimize())
      .catch((error) => console.error('Failed to minimize Mint window:', error))
  }

  const toggleMaximizeWindow = () => {
    void import('@tauri-apps/api/window')
      .then(async ({ getCurrentWindow }) => {
        const currentWindow = getCurrentWindow()
        await currentWindow.toggleMaximize()
        setIsMaximized(await currentWindow.isMaximized())
      })
      .catch((error) => console.error('Failed to toggle Mint window size:', error))
  }

  const toggleFullscreenWindow = () => {
    void import('@tauri-apps/api/window')
      .then(async ({ getCurrentWindow }) => {
        const currentWindow = getCurrentWindow()
        const fullscreen = !await currentWindow.isFullscreen()
        await currentWindow.setFullscreen(fullscreen)
        setIsFullscreen(fullscreen)
      })
      .catch((error) => console.error('Failed to toggle Mint fullscreen:', error))
  }

  const updateZoom = (nextZoom: number) => {
    const clampedZoom = Math.min(1.5, Math.max(0.8, Math.round(nextZoom * 100) / 100))
    setZoom(clampedZoom)
    window.localStorage.setItem('mint:ui-zoom', String(clampedZoom))
  }

  useEffect(() => {
    if (zoom === 1) document.documentElement.style.removeProperty('zoom')
    else document.documentElement.style.setProperty('zoom', String(zoom))
  }, [zoom])

  const isEditable = (target: EventTarget | null) => target instanceof HTMLElement && (
    target.isContentEditable || target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement
  )

  const runEditCommand = (command: 'undo' | 'redo' | 'cut' | 'copy' | 'selectAll') => {
    if (!isEditable(document.activeElement) && command !== 'copy') return
    document.execCommand(command)
  }

  const pasteClipboard = async () => {
    if (!isEditable(document.activeElement)) return
    try {
      const text = await window.api.readClipboard()
      if (text) document.execCommand('insertText', false, text)
    } catch (error) {
      console.warn('Failed to paste clipboard text:', error)
    }
  }

  const startResize = (direction: ResizeDirection) => {
    void import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().startResizeDragging(direction))
      .catch((error) => console.error('Failed to start Mint window resize:', error))
  }

  useEffect(() => {
    const closeMenu = (event: MouseEvent) => {
      if (event.target instanceof Node && !rootRef.current?.contains(event.target)) setOpenMenu(null)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpenMenu(null)
    }
    window.addEventListener('mousedown', closeMenu)
    window.addEventListener('keydown', closeOnEscape)
    return () => {
      window.removeEventListener('mousedown', closeMenu)
      window.removeEventListener('keydown', closeOnEscape)
    }
  }, [])

  useEffect(() => {
    const updateEditableFocus = () => setHasEditableFocus(isEditable(document.activeElement))
    document.addEventListener('focusin', updateEditableFocus)
    document.addEventListener('focusout', updateEditableFocus)
    return () => {
      document.removeEventListener('focusin', updateEditableFocus)
      document.removeEventListener('focusout', updateEditableFocus)
    }
  }, [])

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      const modifier = event.ctrlKey || event.metaKey
      const editing = isEditable(event.target)
      if (event.key === 'F11') {
        event.preventDefault()
        toggleFullscreenWindow()
        return
      }
      if (!modifier || editing) return

      const key = event.key.toLowerCase()
      if (key === 'n') { event.preventDefault(); onNewChat() }
      else if (key === 'o') { event.preventDefault(); onOpenWorkspace() }
      else if (key === 'b' && event.shiftKey) { event.preventDefault(); onOpenBrowser() }
      else if (key === 'b') { event.preventDefault(); onToggleSidebar() }
      else if (key === ',') { event.preventDefault(); onOpenSettings() }
      else if (key === '/') { event.preventDefault(); setShortcutsOpen(true) }
      else if (event.code === 'Backquote') { event.preventDefault(); onToggleTerminal() }
      else if (key === '+' || key === '=') { event.preventDefault(); updateZoom(zoom + 0.1) }
      else if (key === '-') { event.preventDefault(); updateZoom(zoom - 0.1) }
      else if (key === '0') { event.preventDefault(); updateZoom(1) }
    }
    window.addEventListener('keydown', handleShortcut)
    return () => window.removeEventListener('keydown', handleShortcut)
  }, [zoom, onNewChat, onOpenWorkspace, onOpenBrowser, onOpenSettings, onToggleSidebar, onToggleTerminal])

  const runMenuAction = (action: () => void | Promise<void>) => {
    void action()
    setOpenMenu(null)
  }

  const menuItems: Record<MenuName, Array<{ label: string; shortcut?: string; disabled?: boolean; action: () => void | Promise<void> }>> = {
    File: [
      { label: 'New chat', shortcut: 'Ctrl+N', action: onNewChat },
      { label: 'Open workspace', shortcut: 'Ctrl+O', action: onOpenWorkspace },
      { label: 'New terminal', shortcut: 'Ctrl+Shift+`', action: onOpenTerminal },
      { label: 'Settings', shortcut: 'Ctrl+,', action: onOpenSettings },
      { label: 'Quit Mint', shortcut: 'Ctrl+Q', action: () => window.api.quitApp() },
    ],
    Edit: [
      { label: 'Undo', shortcut: 'Ctrl+Z', disabled: !hasEditableFocus, action: () => runEditCommand('undo') },
      { label: 'Redo', shortcut: 'Ctrl+Shift+Z', disabled: !hasEditableFocus, action: () => runEditCommand('redo') },
      { label: 'Cut', shortcut: 'Ctrl+X', disabled: !hasEditableFocus, action: () => runEditCommand('cut') },
      { label: 'Copy', shortcut: 'Ctrl+C', action: () => runEditCommand('copy') },
      { label: 'Paste', shortcut: 'Ctrl+V', disabled: !hasEditableFocus, action: pasteClipboard },
      { label: 'Select all', shortcut: 'Ctrl+A', disabled: !hasEditableFocus, action: () => runEditCommand('selectAll') },
    ],
    View: [
      { label: sidebarCollapsed ? 'Show sidebar' : 'Hide sidebar', shortcut: 'Ctrl+B', action: onToggleSidebar },
      { label: 'Toggle terminal', shortcut: 'Ctrl+`', action: onToggleTerminal },
      { label: 'Open Browser panel', shortcut: 'Ctrl+Shift+B', action: onOpenBrowser },
      { label: 'Zoom in', shortcut: 'Ctrl++', action: () => updateZoom(zoom + 0.1) },
      { label: 'Zoom out', shortcut: 'Ctrl+-', action: () => updateZoom(zoom - 0.1) },
      { label: 'Reset zoom', shortcut: 'Ctrl+0', disabled: zoom === 1, action: () => updateZoom(1) },
      { label: isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen', shortcut: 'F11', action: toggleFullscreenWindow },
    ],
    Help: [
      { label: 'Keyboard shortcuts', shortcut: 'Ctrl+/', action: () => setShortcutsOpen(true) },
      { label: 'Check for updates', action: onCheckForUpdates },
      { label: 'About Mint Agent', action: onShowAbout },
    ],
  }

  return (
    <>
      <header className="mint-titlebar" ref={rootRef} aria-label="Mint Agent window controls">
        <div className="mint-titlebar-left">
        <button
          className="mint-titlebar-icon-button"
          type="button"
          aria-label={sidebarCollapsed ? 'Show sidebar' : 'Hide sidebar'}
          title={sidebarCollapsed ? 'Show sidebar' : 'Hide sidebar'}
          onClick={onToggleSidebar}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="4" width="17" height="16" rx="2"/><path d="M9 4v16"/></svg>
        </button>
        <button className="mint-titlebar-icon-button" type="button" aria-label="Go back" title="Go back" disabled={window.history.length <= 1} onClick={() => window.history.back()}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m14.5 5-7 7 7 7M8 12h12"/></svg>
        </button>
        <button className="mint-titlebar-icon-button" type="button" aria-label="Go forward" title="Go forward" onClick={() => window.history.forward()}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9.5 5 7 7-7 7M16 12H4"/></svg>
        </button>
        <span className="mint-titlebar-divider" aria-hidden="true" />
        <nav className="mint-titlebar-menus" aria-label="Application menu">
          {MENU_NAMES.map((name) => (
            <div className="mint-titlebar-menu-wrap" key={name}>
              <button
                type="button"
                className={`mint-titlebar-menu-button${openMenu === name ? ' is-open' : ''}`}
                aria-haspopup="menu"
                aria-expanded={openMenu === name}
                onClick={() => setOpenMenu((current) => current === name ? null : name)}
              >
                {name}
              </button>
              {openMenu === name && (
                <div className="mint-titlebar-menu" role="menu">
                  {menuItems[name].map((item) => (
                    <button type="button" role="menuitem" key={item.label} disabled={item.disabled} onClick={() => runMenuAction(item.action)}>
                      <span>{item.label}</span>
                      {item.shortcut && <kbd>{item.shortcut}</kbd>}
                    </button>
                  ))}
                </div>
              )}
            </div>
          ))}
        </nav>
        </div>
        <div
          className="mint-titlebar-drag-region"
          data-tauri-drag-region
          aria-hidden="true"
          onDoubleClick={toggleMaximizeWindow}
        />
        <div className="mint-titlebar-window-controls">
          <button type="button" className="mint-window-control" aria-label="Minimize window" title="Minimize" onClick={minimizeWindow}>
            <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2 6h8"/></svg>
          </button>
          <button type="button" className="mint-window-control" aria-label={isMaximized ? 'Restore window' : 'Maximize window'} title={isMaximized ? 'Restore' : 'Maximize'} onClick={toggleMaximizeWindow}>
            <svg viewBox="0 0 12 12" aria-hidden="true">
              {isMaximized ? <><path d="M4 2.5h5.5V8"/><path d="M8 4H2.5v5.5H8z"/></> : <rect x="2.5" y="2.5" width="7" height="7" rx="0.7"/>}
            </svg>
          </button>
          <button type="button" className="mint-window-control is-close" aria-label="Close window" title="Close" onClick={() => window.api.closeWindow()}>
            <svg viewBox="0 0 12 12" aria-hidden="true"><path d="m3 3 6 6m0-6L3 9"/></svg>
          </button>
        </div>
      </header>
      {shortcutsOpen && (
        <div className="mint-shortcuts-backdrop" role="presentation" onMouseDown={() => setShortcutsOpen(false)}>
          <section className="mint-shortcuts-dialog" role="dialog" aria-modal="true" aria-labelledby="mint-shortcuts-title" onMouseDown={(event) => event.stopPropagation()}>
            <header><h2 id="mint-shortcuts-title">Keyboard shortcuts</h2><button type="button" aria-label="Close shortcuts" onClick={() => setShortcutsOpen(false)}>×</button></header>
            <dl>
              <div><dt>New chat</dt><dd><kbd>Ctrl+N</kbd></dd></div>
              <div><dt>Open workspace</dt><dd><kbd>Ctrl+O</kbd></dd></div>
              <div><dt>Toggle sidebar</dt><dd><kbd>Ctrl+B</kbd></dd></div>
              <div><dt>Toggle terminal</dt><dd><kbd>Ctrl+`</kbd></dd></div>
              <div><dt>Open Browser panel</dt><dd><kbd>Ctrl+Shift+B</kbd></dd></div>
              <div><dt>Settings</dt><dd><kbd>Ctrl+,</kbd></dd></div>
              <div><dt>Zoom</dt><dd><kbd>Ctrl++</kbd> <kbd>Ctrl+-</kbd> <kbd>Ctrl+0</kbd></dd></div>
              <div><dt>Fullscreen</dt><dd><kbd>F11</kbd></dd></div>
            </dl>
          </section>
        </div>
      )}
      <div className="mint-window-resize-handles" aria-hidden="true">
        {RESIZE_DIRECTIONS.map((direction) => (
          <span
            key={direction}
            className={`mint-window-resize-handle is-${direction.toLowerCase()}`}
            onMouseDown={(event) => {
              event.preventDefault()
              event.stopPropagation()
              startResize(direction)
            }}
          />
        ))}
      </div>
    </>
  )
}
