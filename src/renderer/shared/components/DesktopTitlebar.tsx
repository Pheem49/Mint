import { useEffect, useRef, useState } from 'react'

type ResizeDirection = 'East' | 'North' | 'NorthEast' | 'NorthWest' | 'South' | 'SouthEast' | 'SouthWest' | 'West'

type MenuName = 'File' | 'Edit' | 'View' | 'Help'

interface DesktopTitlebarProps {
  sidebarCollapsed: boolean
  onToggleSidebar: () => void
  onNewChat: () => void
  onOpenWorkspace: () => void
  onToggleTerminal: () => void
  onShowAbout: () => void
}

const MENU_NAMES: MenuName[] = ['File', 'Edit', 'View', 'Help']
const RESIZE_DIRECTIONS: ResizeDirection[] = ['NorthWest', 'North', 'NorthEast', 'West', 'East', 'SouthWest', 'South', 'SouthEast']

export default function DesktopTitlebar({
  sidebarCollapsed,
  onToggleSidebar,
  onNewChat,
  onOpenWorkspace,
  onToggleTerminal,
  onShowAbout,
}: DesktopTitlebarProps) {
  const [openMenu, setOpenMenu] = useState<MenuName | null>(null)
  const [isMaximized, setIsMaximized] = useState(false)
  const rootRef = useRef<HTMLElement>(null)

  useEffect(() => {
    let mounted = true
    void import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().isMaximized())
      .then((maximized) => {
        if (mounted) setIsMaximized(maximized)
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

  const runMenuAction = (action: () => void) => {
    action()
    setOpenMenu(null)
  }

  const menuItems: Record<MenuName, Array<{ label: string; shortcut?: string; action: () => void }>> = {
    File: [
      { label: 'New chat', shortcut: 'Ctrl+N', action: onNewChat },
      { label: 'Open workspace', action: onOpenWorkspace },
    ],
    Edit: [
      { label: 'Select all', shortcut: 'Ctrl+A', action: () => document.execCommand('selectAll') },
      { label: 'Copy', shortcut: 'Ctrl+C', action: () => document.execCommand('copy') },
    ],
    View: [
      { label: sidebarCollapsed ? 'Show sidebar' : 'Hide sidebar', shortcut: 'Ctrl+B', action: onToggleSidebar },
      { label: 'Toggle terminal', shortcut: 'Ctrl+`', action: onToggleTerminal },
    ],
    Help: [
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
                    <button type="button" role="menuitem" key={item.label} onClick={() => runMenuAction(item.action)}>
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
