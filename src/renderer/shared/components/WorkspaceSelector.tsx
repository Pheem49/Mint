import { useEffect, useMemo, useRef, useState } from 'react'

interface WorkspaceSelectorProps {
  currentPath: string
  recentPaths: string[]
  onSelectWorkspace: (path?: string) => void
}

function workspaceLabel(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path
}

export default function WorkspaceSelector({ currentPath, recentPaths, onSelectWorkspace }: WorkspaceSelectorProps) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const rootRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)

  const visiblePaths = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase()
    return recentPaths.filter((path) => {
      if (!normalizedQuery) return true
      return path.toLocaleLowerCase().includes(normalizedQuery)
        || workspaceLabel(path).toLocaleLowerCase().includes(normalizedQuery)
    })
  }, [query, recentPaths])

  useEffect(() => {
    if (!open) return
    searchRef.current?.focus()

    const closeOnOutsidePointer = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      setOpen(false)
      triggerRef.current?.focus()
    }
    document.addEventListener('mousedown', closeOnOutsidePointer)
    window.addEventListener('keydown', closeOnEscape)
    return () => {
      document.removeEventListener('mousedown', closeOnOutsidePointer)
      window.removeEventListener('keydown', closeOnEscape)
    }
  }, [open])

  const chooseWorkspace = (path?: string) => {
    setOpen(false)
    setQuery('')
    triggerRef.current?.focus()
    onSelectWorkspace(path)
  }

  return (
    <div className={`workspace-selector ${currentPath ? 'has-workspace' : 'needs-workspace'}`} ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        className={`workspace-select-btn ${currentPath ? 'has-workspace' : 'needs-workspace'}`}
        onClick={() => {
          setQuery('')
          setOpen((current) => !current)
        }}
        aria-label={`${currentPath ? 'Change' : 'Choose'} workspace: ${currentPath ? workspaceLabel(currentPath) : 'Select Project'}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls="workspace-select-menu"
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
        <div className="workspace-select-menu" id="workspace-select-menu" role="dialog" aria-label="Choose workspace">
          <label className="workspace-select-search">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
              <circle cx="11" cy="11" r="7" />
              <path d="m20 20-4-4" />
            </svg>
            <input
              ref={searchRef}
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search workspaces"
              aria-label="Search workspaces"
            />
          </label>

          <div className="workspace-select-list" role="group" aria-label="Recent workspaces">
            {visiblePaths.length > 0 ? visiblePaths.map((path) => {
              const isCurrent = path === currentPath
              return (
                <button
                  key={path}
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
              )
            }) : (
              <div className="workspace-select-empty">
                {query.trim() ? 'No matching workspaces' : 'No recent workspaces yet'}
              </div>
            )}
          </div>

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
    </div>
  )
}
