import React, { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { workspacePlatform } from '@shared/platform'
import { announceWorkspaceChange, useWorkspaceUndo } from '@shared/workspaceUndo'
import type { WorkspaceSnapshot, WorkspaceTreeEntry } from '@shared/types'
import {
  materialFolderIcon,
  materialFileIcon,
  getExtension as extension,
  folderOpenIcon
} from '../../shared/utils/fileIcons'


interface WorkspacePanelProps {
  onOpenHtml?: (root: string, path: string, mode: 'external' | 'mint') => Promise<void>
  agentMode: boolean
  sending: boolean
  workspacePath: string
  onEnableAgentMode: () => void
  onSetMessage: (message: string) => void
  onWorkspaceReady: (path: string) => void
  refreshRevision?: number
}

const FILE_LABEL: Record<string, string> = {
  css: '#',
  html: '<>',
  js: 'JS',
  json: '{}',
  md: 'MD',
  rs: 'RS',
  ts: 'TS',
  tsx: 'TS',
}


interface WorkspaceMenu {
  entry: WorkspaceTreeEntry
  x: number
  y: number
  trigger: HTMLButtonElement
}

function TreeNode({ entry, level, onContextMenu }: { entry: WorkspaceTreeEntry; level: number; onContextMenu: (event: React.MouseEvent<HTMLButtonElement>, entry: WorkspaceTreeEntry) => void; key?: string }) {
  const [open, setOpen] = useState(level < 1)
  const isDirectory = entry.kind === 'directory'
  const hasChildren = entry.children.length > 0
  const fileExtension = extension(entry.name)
  const fileLabel = FILE_LABEL[fileExtension] || ''
  const materialIcon = isDirectory ? materialFolderIcon(entry.name, open) : materialFileIcon(entry.name, fileExtension)
  const dragText = `@${entry.path}`

  return (
    <div className="workspace-tree-node">
      <button
        type="button"
        className={`workspace-tree-row ${isDirectory ? 'is-directory' : 'is-file'}`}
        style={{ paddingLeft: `${level * 16 + 8}px` }}
        onClick={() => isDirectory && hasChildren && setOpen((current) => !current)}
        onContextMenu={(event) => onContextMenu(event, entry)}
        aria-haspopup="menu"
        title={entry.path}
        draggable
        onDragStart={(event) => {
          event.dataTransfer.setData('application/x-mint-workspace-path', dragText)
          event.dataTransfer.setData('text/plain', dragText)
          event.dataTransfer.effectAllowed = 'copy'
        }}
      >
        <span
          className={`workspace-tree-chevron ${isDirectory && hasChildren ? '' : 'is-spacer'}`}
          data-open={open}
          aria-hidden="true"
        >
          {isDirectory && hasChildren ? '' : null}
        </span>
        <span className={`workspace-tree-icon material-icon ${isDirectory ? 'folder' : fileExtension || 'file'}`} aria-hidden="true">
          {materialIcon ? <img src={materialIcon} alt="" draggable={false} /> : isDirectory ? '' : fileLabel}
        </span>
        <span className="workspace-tree-name">{entry.name}</span>
      </button>
      {isDirectory && open && hasChildren && (
        <div className="workspace-tree-children">
          {entry.children.map((child) => (
            <TreeNode key={child.path} entry={child} level={level + 1} onContextMenu={onContextMenu} />
          ))}
        </div>
      )}
    </div>
  )
}

export default function WorkspacePanel({ agentMode, sending, workspacePath, onEnableAgentMode, onSetMessage, onWorkspaceReady, refreshRevision = 0, onOpenHtml }: WorkspacePanelProps) {
  const [tree, setTree] = useState<WorkspaceTreeEntry | null>(null)
  const [revision, setRevision] = useState(0)
  const revisionRef = useRef(0)
  const refreshInFlightRef = useRef(false)
  const [error, setError] = useState('')
  const [menu, setMenu] = useState<WorkspaceMenu | null>(null)
  const [editTarget, setEditTarget] = useState<{ entry: WorkspaceTreeEntry; mode: 'rename' | 'move'; value: string } | null>(null)
  const [mutationPending, setMutationPending] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)
  const trashInFlightRef = useRef(false)
  const workspacePathRef = useRef(workspacePath)
  workspacePathRef.current = workspacePath

  const closeMenu = useCallback(() => {
    menu?.trigger.focus()
    setMenu(null)
  }, [menu])

  useEffect(() => { setMenu(null); setEditTarget(null) }, [workspacePath])

  useEffect(() => {
    if (!menu) return
    menuRef.current?.querySelector('button')?.focus()
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) setMenu(null)
    }
    const closeOnKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' || event.key === 'Tab') {
        if (event.key === 'Escape') event.preventDefault()
        closeMenu()
      }
    }
    const dismiss = () => setMenu(null)
    document.addEventListener('pointerdown', closeOnOutsidePointer)
    document.addEventListener('keydown', closeOnKey)
    window.addEventListener('resize', dismiss)
    window.addEventListener('blur', dismiss)
    document.addEventListener('scroll', dismiss, true)
    return () => {
      document.removeEventListener('pointerdown', closeOnOutsidePointer)
      document.removeEventListener('keydown', closeOnKey)
      window.removeEventListener('resize', dismiss)
      window.removeEventListener('blur', dismiss)
      document.removeEventListener('scroll', dismiss, true)
    }
  }, [menu, closeMenu])

  const handleContextMenu = (event: React.MouseEvent<HTMLButtonElement>, entry: WorkspaceTreeEntry) => {
    event.preventDefault()
    event.stopPropagation()
    if (trashInFlightRef.current) return
    const bounds = event.currentTarget.getBoundingClientRect()
    setMenu({
      entry,
      x: Math.max(8, Math.min(event.clientX || bounds.left, window.innerWidth - 200)),
      y: Math.max(8, Math.min(event.clientY || bounds.bottom, window.innerHeight - (entry.kind === 'file' && /\.html?$/i.test(entry.name) ? 200 : 132))),
      trigger: event.currentTarget,
    })
  }

  const handleMoveToTrash = async () => {
    if (!menu || trashInFlightRef.current) return
    const { entry } = menu
    const root = workspacePath
    trashInFlightRef.current = true
    setMutationPending(true)
    closeMenu()
    try {
      const { confirm } = await import('@tauri-apps/plugin-dialog')
      const confirmed = await confirm(`Move "${entry.name}"${entry.kind === 'directory' ? ' and all its contents' : ''} to Trash? You can restore it from your system Trash.`, {
        title: 'Move to Trash',
        kind: 'warning',
        okLabel: 'Move to Trash',
        cancelLabel: 'Cancel',
      })
      if (!confirmed || workspacePathRef.current !== root) return
      setError('')
      const snapshot = await workspacePlatform.deleteWorkspaceItem({ root, relativePath: entry.path, revision: revisionRef.current })
      if (workspacePathRef.current === root) {
        applySnapshot(snapshot)
      }
      announceWorkspaceChange(snapshot)
    } catch (reason) {
      if (workspacePathRef.current === root) setError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      trashInFlightRef.current = false
      setMutationPending(false)
    }
  }

  const applySnapshot = useCallback((snapshot: WorkspaceSnapshot) => {
    setTree(snapshot.tree)
    revisionRef.current = snapshot.revision
    setRevision(snapshot.revision)
    if (snapshot.path !== workspacePath) onWorkspaceReady(snapshot.path)
  }, [onWorkspaceReady, workspacePath])
  const history = useWorkspaceUndo(workspacePath, refreshRevision, applySnapshot)

  const openHtml = async (mode: 'external' | 'mint') => {
    if (!menu || !onOpenHtml || mutationPending) return
    const { entry } = menu
    const root = workspacePath
    closeMenu()
    setMutationPending(true)
    setError('')
    try { await onOpenHtml(root, entry.path, mode) }
    catch (reason) { if (workspacePathRef.current === root) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { setMutationPending(false) }
  }

  const openMoveDialog = (mode: 'rename' | 'move') => {
    if (!menu || mutationPending) return
    const { entry } = menu
    setEditTarget({ entry, mode, value: mode === 'rename' ? entry.name : entry.path })
    setError('')
    closeMenu()
  }

  const handleMove = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!editTarget || trashInFlightRef.current) return
    const { entry, mode, value } = editTarget
    const root = workspacePath
    const name = value.trim()
    if (!name || (mode === 'rename' && /[\\/]/.test(name))) {
      setError('Enter a valid name without path separators.'); return
    }
    const parent = entry.path.split('/').slice(0, -1).join('/')
    const destination = mode === 'rename' ? [parent, name].filter(Boolean).join('/') : name
    if (destination === entry.path) { setEditTarget(null); return }
    trashInFlightRef.current = true
    setMutationPending(true)
    try {
      const snapshot = await workspacePlatform.moveWorkspaceItem({ root, relativePath: entry.path, revision: revisionRef.current }, destination)
      if (workspacePathRef.current === root) {
        applySnapshot(snapshot)
        setEditTarget(null)
        setError('')
      }
      announceWorkspaceChange(snapshot)
    } catch (reason) {
      if (workspacePathRef.current === root) setError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      trashInFlightRef.current = false
      setMutationPending(false)
    }
  }
  const refresh = useCallback(async () => {
    if (!workspacePath.trim()) {
      setError('')
      setTree(null)
      return
    }
    if (refreshInFlightRef.current) return

    refreshInFlightRef.current = true
    try {
      setError('')
      applySnapshot(await workspacePlatform.getWorkspaceSnapshot({ root: workspacePath, relativePath: '', revision: revisionRef.current }))
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      refreshInFlightRef.current = false
    }
  }, [applySnapshot, workspacePath])

  useEffect(() => {
    void refresh()

    // There is no filesystem event stream behind this native snapshot API.
    // Catch up immediately when returning to the app and use a low-frequency
    // fallback only while this panel is actually visible and focused.
    const refreshWhenActive = () => {
      if (document.visibilityState === 'visible' && document.hasFocus()) void refresh()
    }
    window.addEventListener('focus', refreshWhenActive)
    document.addEventListener('visibilitychange', refreshWhenActive)

    const interval = window.setInterval(refreshWhenActive, 30000)

    return () => {
      window.removeEventListener('focus', refreshWhenActive)
      document.removeEventListener('visibilitychange', refreshWhenActive)
      clearInterval(interval)
    }
  }, [workspacePath, refreshRevision, refresh])

  const handleCreateFile = async () => {
    if (!workspacePath.trim()) return
    const name = prompt('Enter name of new file:')
    if (!name || !name.trim()) return

    try {
      setError('')
      const snapshot = await workspacePlatform.createWorkspaceFile({ root: workspacePath, relativePath: name.trim(), revision })
      applySnapshot(snapshot)
      announceWorkspaceChange(snapshot)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const handleCreateFolder = async () => {
    if (!workspacePath.trim()) return
    const name = prompt('Enter name of new folder:')
    if (!name || !name.trim()) return

    try {
      setError('')
      const snapshot = await workspacePlatform.createWorkspaceFolder({ root: workspacePath, relativePath: name.trim(), revision })
      applySnapshot(snapshot)
      announceWorkspaceChange(snapshot)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <section className="workspace-panel">
      <header className="workspace-panel-header">
        <div className="workspace-title-group">
          <span className="workspace-title-icon material-icon folder" aria-hidden="true">
            <img src={folderOpenIcon} alt="" draggable={false} />
          </span>
          <div className="workspace-title-copy">
            <h2 title={workspacePath || 'Workspace'}>{tree?.name || workspacePath.split(/[\\/]/).filter(Boolean).pop() || 'Workspace'}</h2>
          </div>
          <span className="workspace-agent-pill" data-state={sending ? 'thinking' : agentMode ? 'agent' : 'idle'}>
            {sending ? 'Running' : agentMode ? 'Agent mode' : 'Manual'}
          </span>
        </div>
        <div className="workspace-panel-actions" aria-label="Workspace actions">
          <button type="button" onClick={handleCreateFile} disabled={!workspacePath.trim()} aria-label="New file" title="New file">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6M12 11v6M9 14h6"/></svg>
          </button>
          <button type="button" onClick={handleCreateFolder} disabled={!workspacePath.trim()} aria-label="New folder" title="New folder">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M12 10v6M9 13h6"/></svg>
          </button>
          <button type="button" onClick={refresh} disabled={!workspacePath.trim()} aria-label="Refresh workspace" title="Refresh workspace">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5"/><path d="M5.6 9a7 7 0 0 1 11.55-2.6L20 12M4 12l2.85 5.6A7 7 0 0 0 18.4 15"/></svg>
          </button>
        </div>
      </header>

      <div className="workspace-tree-shell">
        {(error || history.error) && <div className="workspace-history-error" role="alert">{error || history.error}</div>}
        {tree ? (
          <>
            <div className="workspace-root-row">
              <span className="workspace-tree-chevron" data-open="true" aria-hidden="true" />
              <span className="workspace-tree-icon material-icon folder" aria-hidden="true">
                <img src={folderOpenIcon} alt="" draggable={false} />
              </span>
              <span>{tree.name}</span>
            </div>
            <div className="workspace-tree">
              {tree.children.map((entry) => (
                <TreeNode key={entry.path} entry={entry} level={0} onContextMenu={handleContextMenu} />
              ))}
            </div>
          </>
        ) : (
          <div className="workspace-tree-empty">Select a project to show workspace files.</div>
        )}

      </div>
      {menu && createPortal(
        <div ref={menuRef} className="workspace-file-menu" role="menu" aria-label={`Actions for ${menu.entry.name}`} style={{ left: menu.x, top: menu.y }} onContextMenu={(event) => event.preventDefault()}>
          {onOpenHtml && menu.entry.kind === 'file' && /\.html?$/i.test(menu.entry.name) && <>
            <button type="button" role="menuitem" disabled={mutationPending} onClick={() => { void openHtml('external') }}>Open in Browser</button>
            <button type="button" role="menuitem" disabled={mutationPending} onClick={() => { void openHtml('mint') }}>Preview in Mint</button>
          </>}
          <button type="button" role="menuitem" onClick={() => openMoveDialog('rename')}>Rename…</button>
          <button type="button" role="menuitem" onClick={() => openMoveDialog('move')}>Move…</button>
          <button type="button" role="menuitem" onClick={handleMoveToTrash}>Move to Trash</button>
        </div>,
        document.body,
      )}
      {editTarget && createPortal(
        <div className="workspace-file-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget && !mutationPending) setEditTarget(null) }}>
          <form className="workspace-file-dialog" role="dialog" aria-modal="true" aria-labelledby="workspace-file-dialog-title" onSubmit={handleMove} onKeyDown={(event) => { if (event.key === 'Escape' && !mutationPending) setEditTarget(null) }}>
            <h3 id="workspace-file-dialog-title">{editTarget.mode === 'rename' ? 'Rename' : 'Move'} {editTarget.entry.name}</h3>
            <label>{editTarget.mode === 'rename' ? 'New name' : 'Destination path relative to workspace'}
              <input autoFocus value={editTarget.value} disabled={mutationPending} onChange={(event) => setEditTarget({ ...editTarget, value: event.target.value })} />
            </label>
            {error && <p role="alert">{error}</p>}
            <div><button type="button" disabled={mutationPending} onClick={() => setEditTarget(null)}>Cancel</button><button type="submit" disabled={mutationPending}>{mutationPending ? 'Saving…' : editTarget.mode === 'rename' ? 'Rename' : 'Move'}</button></div>
          </form>
        </div>, document.body,
      )}
    </section>
  )
}
