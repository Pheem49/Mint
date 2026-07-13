import React, { useEffect, useState } from 'react'
import { getWorkspaceTree, type WorkspaceTreeEntry, createWorkspaceFile, createWorkspaceFolder, deleteWorkspaceItem } from '../tauri'
import {
  materialFolderIcon,
  materialFileIcon,
  getExtension as extension,
  folderOpenIcon
} from '../../shared/utils/fileIcons'


interface WorkspacePanelProps {
  agentMode: boolean
  sending: boolean
  workspacePath: string
  onEnableAgentMode: () => void
  onSetMessage: (message: string) => void
  onWorkspaceReady: (path: string) => void
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


function TreeNode({ entry, level, onRefresh, workspacePath }: { entry: WorkspaceTreeEntry; level: number; onRefresh: () => void; workspacePath: string; key?: string }) {
  const [open, setOpen] = useState(level < 1)
  const isDirectory = entry.kind === 'directory'
  const hasChildren = entry.children.length > 0
  const fileExtension = extension(entry.name)
  const fileLabel = FILE_LABEL[fileExtension] || ''
  const materialIcon = isDirectory ? materialFolderIcon(entry.name, open) : materialFileIcon(entry.name, fileExtension)
  const dragText = `@${entry.path}`

  const handleContextMenu = async (event: React.MouseEvent) => {
    event.preventDefault()
    const confirmed = confirm(`Are you sure you want to delete "${entry.name}"?`)
    if (!confirmed) return

    try {
      const absolutePath = `${workspacePath}/${entry.path}`
      await deleteWorkspaceItem(absolutePath)
      onRefresh()
    } catch (err) {
      alert(`Failed to delete item: ${err instanceof Error ? err.message : String(err)}`)
    }
  }

  return (
    <div className="workspace-tree-node">
      <button
        type="button"
        className={`workspace-tree-row ${isDirectory ? 'is-directory' : 'is-file'}`}
        style={{ paddingLeft: `${level * 16 + 8}px` }}
        onClick={() => isDirectory && hasChildren && setOpen((current) => !current)}
        onContextMenu={handleContextMenu}
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
            <TreeNode key={child.path} entry={child} level={level + 1} onRefresh={onRefresh} workspacePath={workspacePath} />
          ))}
        </div>
      )}
    </div>
  )
}

export default function WorkspacePanel({ agentMode, sending, workspacePath, onEnableAgentMode, onSetMessage, onWorkspaceReady }: WorkspacePanelProps) {
  const [tree, setTree] = useState<WorkspaceTreeEntry | null>(null)
  const [error, setError] = useState('')

  const refresh = async () => {
    if (!workspacePath.trim()) {
      setError('')
      setTree(null)
      return
    }

    try {
      setError('')
      const nextTree = await getWorkspaceTree(workspacePath)
      setTree(nextTree)
      if (nextTree.path !== workspacePath) onWorkspaceReady(nextTree.path)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    }
  }

  useEffect(() => {
    refresh()

    // Refresh when the window gains focus (e.g., switching back from VS Code)
    const handleFocus = () => {
      refresh()
    }
    window.addEventListener('focus', handleFocus)

    // Poll every 15 seconds to catch edits/updates in real-time
    const interval = setInterval(() => {
      refresh()
    }, 15000)

    return () => {
      window.removeEventListener('focus', handleFocus)
      clearInterval(interval)
    }
  }, [workspacePath])

  const handleCreateFile = async () => {
    if (!workspacePath.trim()) return
    const name = prompt('Enter name of new file:')
    if (!name || !name.trim()) return

    try {
      setError('')
      const fullPath = `${workspacePath}/${name.trim()}`
      await createWorkspaceFile(fullPath)
      await refresh()
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
      const fullPath = `${workspacePath}/${name.trim()}`
      await createWorkspaceFolder(fullPath)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <section className="workspace-panel">
      <header className="workspace-panel-header">
        <div>
          <span className="workspace-kicker">Agent Workspace</span>
          <h2>Workspace</h2>
        </div>
        <span className="workspace-agent-pill" data-state={sending ? 'thinking' : agentMode ? 'agent' : 'idle'}>
          {sending ? 'Running' : agentMode ? 'Agent mode' : 'Manual'}
        </span>
      </header>

      <div className="workspace-panel-actions">
        <button type="button" onClick={handleCreateFile} disabled={!workspacePath.trim()}>New File</button>
        <button type="button" onClick={handleCreateFolder} disabled={!workspacePath.trim()}>New Folder</button>
        <button type="button" onClick={refresh} disabled={!workspacePath.trim()}>Refresh</button>
      </div>

      <div className="workspace-tree-shell">
        {error ? (
          <div className="workspace-tree-empty">{error}</div>
        ) : tree ? (
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
                <TreeNode key={entry.path} entry={entry} level={0} onRefresh={refresh} workspacePath={workspacePath} />
              ))}
            </div>
          </>
        ) : (
          <div className="workspace-tree-empty">Select a project to show workspace files.</div>
        )}
      </div>
    </section>
  )
}
