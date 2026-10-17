import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { createPortal } from 'react-dom'

import { normalizeWorkspacePath, workspacePaths } from '../utils/workspaces'
import { EMPTY_REMOVED_WORKSPACES, subscribeWorkspaceListPreferences, workspaceListPreferences } from '../utils/workspaceListPreferences'
import './workspace-selector.css'

interface WorkspaceSelectorProps {
  placement?: 'above' | 'below'
  currentPath: string
  recentPaths: string[]
  onSelectWorkspace: (path?: string) => void
}

function workspaceLabel(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path
}

export default function WorkspaceSelector({ currentPath, recentPaths, onSelectWorkspace, placement = 'above' }: WorkspaceSelectorProps) {
  const menuId = useId()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [actionsPath, setActionsPath] = useState<string | null>(null)
  const [notice, setNotice] = useState('')
  const removedPaths = useSyncExternalStore(subscribeWorkspaceListPreferences, workspaceListPreferences.getSnapshot, () => EMPTY_REMOVED_WORKSPACES)
  const rootRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const moreTriggerRef = useRef<HTMLButtonElement | null>(null)
  const removeRef = useRef<HTMLButtonElement>(null)
  const actionsMenuRef = useRef<HTMLDivElement>(null)

  const visiblePaths = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase()
    return workspacePaths(recentPaths, currentPath, [], removedPaths).filter((path) => {
      if (!normalizedQuery) return true
      return path.toLocaleLowerCase().includes(normalizedQuery)
        || workspaceLabel(path).toLocaleLowerCase().includes(normalizedQuery)
    })
  }, [query, recentPaths, currentPath, removedPaths])

  useLayoutEffect(() => {
    if (!open || !actionsPath) return
    const positionDropdown = () => {
      const trigger = moreTriggerRef.current
      const menu = actionsMenuRef.current
      if (!trigger || !menu) return
      const anchor = trigger.getBoundingClientRect()
      const bounds = menu.getBoundingClientRect()
      const width = document.documentElement.clientWidth
      const left = Math.max(8, Math.min(anchor.right - bounds.width, width - bounds.width - 8))
      const top = anchor.bottom + bounds.height + 4 <= window.innerHeight
        ? anchor.bottom + 4 : Math.max(8, anchor.top - bounds.height - 4)
      menu.style.left = `${left}px`
      menu.style.top = `${top}px`
    }
    positionDropdown()
    removeRef.current?.focus()
    window.addEventListener('resize', positionDropdown)
    window.addEventListener('scroll', positionDropdown, true)
    return () => {
      window.removeEventListener('resize', positionDropdown)
      window.removeEventListener('scroll', positionDropdown, true)
    }
  }, [open, actionsPath])

  useEffect(() => {
    if (!open) return
    searchRef.current?.focus()
    const fitMenuToViewport = () => {
      const root = rootRef.current
      if (!root) return
      const width = Math.min(360, document.documentElement.clientWidth - 32)
      const rootLeft = root.getBoundingClientRect().left
      const left = Math.max(16, Math.min(rootLeft, document.documentElement.clientWidth - width - 16))
      root.style.setProperty('--workspace-menu-width', `${width}px`)
      root.style.setProperty('--workspace-menu-offset', `${left - rootLeft}px`)
    }
    fitMenuToViewport()

    const closeOnOutsidePointer = (event: MouseEvent) => {
      if (actionsMenuRef.current?.contains(event.target as Node)) return
      if (!moreTriggerRef.current?.contains(event.target as Node)) setActionsPath(null)
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) return
      event.preventDefault()
      setOpen(false)
      triggerRef.current?.focus()
    }
    document.addEventListener('mousedown', closeOnOutsidePointer)
    window.addEventListener('keydown', closeOnEscape)
    window.addEventListener('resize', fitMenuToViewport)
    return () => {
      document.removeEventListener('mousedown', closeOnOutsidePointer)
      window.removeEventListener('keydown', closeOnEscape)
      window.removeEventListener('resize', fitMenuToViewport)
    }
  }, [open])

  const chooseWorkspace = (path?: string) => {
    setOpen(false)
    setQuery('')
    triggerRef.current?.focus()
    onSelectWorkspace(path)
  }

  return (
    <div className={`workspace-selector ${currentPath ? 'has-workspace' : 'needs-workspace'} opens-${placement}`} ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        className={`workspace-select-btn ${currentPath ? 'has-workspace' : 'needs-workspace'}`}
        onClick={() => {
          setQuery('')
          setActionsPath(null)
          setNotice('')
          setOpen((current) => !current)
        }}
        aria-label={`${currentPath ? 'Change' : 'Choose'} workspace: ${currentPath ? workspaceLabel(currentPath) : 'Select Project'}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={menuId}
        title={currentPath || 'Choose a workspace folder'}
      >
        <span className="workspace-select-icon" aria-hidden="true">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 6h7l2 2h9v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
          </svg>
        </span>
        <span className="workspace-select-copy">
          <span className="workspace-select-name">{currentPath ? workspaceLabel(currentPath) : 'Select Project'}</span>
        </span>
        <svg className="workspace-select-chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      {open && (
        <div className="workspace-select-menu" id={menuId} role="dialog" aria-label="Choose workspace">
          <label className="workspace-select-search">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
              <circle cx="11" cy="11" r="7" />
              <path d="m20 20-4-4" />
            </svg>
            <input
              ref={searchRef}
              type="search"
              value={query}
              onChange={(event) => { setQuery(event.target.value); setActionsPath(null) }}
              placeholder="Search workspaces"
              aria-label="Search workspaces"
            />
          </label>

          <div className="workspace-select-list" role="group" aria-label="Recent workspaces">
            {visiblePaths.length > 0 ? visiblePaths.map((path) => {
              const isCurrent = path === normalizeWorkspacePath(currentPath)
              return (
                <div className="workspace-select-row" key={path}>
                <button
                  type="button"
                  className="workspace-select-option"
                  onClick={() => chooseWorkspace(path)}
                  aria-current={isCurrent ? 'true' : undefined}
                  title={path}
                >
                  <svg className="workspace-select-option-icon" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="M3 6h7l2 2h9v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
                  </svg>
                  <span className="workspace-select-option-copy">
                    <span className="workspace-select-option-name">{workspaceLabel(path)}</span>
                    <span className="workspace-select-option-path">{path}</span>
                  </span>
                  {isCurrent && (
                    <svg className="workspace-select-current" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <path d="m5 12 4 4L19 6" />
                    </svg>
                  )}
                </button>
                <button
                  type="button"
                  className="workspace-select-more"
                  aria-label={`Workspace actions: ${workspaceLabel(path)}`}
                  title="Workspace actions"
                  aria-haspopup="menu"
                  aria-expanded={actionsPath === path}
                  onClick={(event) => {
                    moreTriggerRef.current = event.currentTarget
                    setActionsPath(current => current === path ? null : path)
                  }}
                  onKeyDown={(event) => {
                    if (event.key === 'ArrowDown') { event.preventDefault(); moreTriggerRef.current = event.currentTarget; setActionsPath(path) }
                  }}
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                    <circle cx="5" cy="12" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="19" cy="12" r="1.8" />
                  </svg>
                </button>

                </div>
              )
            }) : (
              <div className="workspace-select-empty">
                {query.trim() ? 'No matching workspaces' : 'No recent workspaces yet'}
              </div>
            )}
          </div>

          {notice && <p className="workspace-select-notice" role="status">{notice}</p>}

          <div className="workspace-select-menu-footer">
            <button type="button" className="workspace-select-open-folder" onClick={() => chooseWorkspace()}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M3 6h7l2 2h9v12H3z" />
                <path d="M12 10v6m-3-3h6" />
              </svg>
              <span>Open folder…</span>
            </button>
          </div>
        </div>
      )}
      {open && actionsPath && visiblePaths.includes(actionsPath) && createPortal(
        <div ref={actionsMenuRef} className="workspace-select-row-menu" role="menu" aria-label={`Workspace actions: ${workspaceLabel(actionsPath)}`}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault(); event.stopPropagation(); setActionsPath(null); moreTriggerRef.current?.focus()
            }
            if (event.key === 'ArrowDown' || event.key === 'ArrowUp') event.preventDefault()
          }}>
          <button ref={removeRef} type="button" role="menuitem" className="workspace-select-remove"
            onClick={() => {
              workspaceListPreferences.remove(actionsPath)
              setActionsPath(null)
              setNotice('Removed from list. Use Open folder… to add it again.')
              searchRef.current?.focus()
            }}>
            <span>Remove from list</span>
            <small>Does not delete files or folders.</small>
          </button>
        </div>, document.body,
      )}
    </div>
  )
}
