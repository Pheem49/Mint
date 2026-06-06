import { useEffect, useState } from 'react'
import { getWorkspaceTree, type WorkspaceTreeEntry } from '../tauri'

interface WorkspacePanelProps {
  agentMode: boolean
  sending: boolean
  workspacePath: string
  onEnableAgentMode: () => void
  onSetMessage: (message: string) => void
  onWorkspaceReady: (path: string) => void
}

const FILE_ICON: Record<string, string> = {
  css: '▣',
  html: '◇',
  js: 'JS',
  json: '{}',
  md: 'i',
  ts: 'TS',
  tsx: '✣',
}

function extension(name: string) {
  const index = name.lastIndexOf('.')
  return index === -1 ? '' : name.slice(index + 1).toLowerCase()
}

function TreeNode({ entry, level }: { entry: WorkspaceTreeEntry; level: number }) {
  const [open, setOpen] = useState(level < 1)
  const isDirectory = entry.kind === 'directory'
  const hasChildren = entry.children.length > 0
  const icon = isDirectory ? (open ? '▾' : '▸') : FILE_ICON[extension(entry.name)] || '•'
  const dragText = `@${entry.path}`

  return (
    <div className="workspace-tree-node">
      <button
        type="button"
        className={`workspace-tree-row ${isDirectory ? 'is-directory' : 'is-file'}`}
        style={{ paddingLeft: `${level * 16 + 8}px` }}
        onClick={() => isDirectory && hasChildren && setOpen((current) => !current)}
        title={entry.path}
        draggable
        onDragStart={(event) => {
          event.dataTransfer.setData('application/x-mint-workspace-path', dragText)
          event.dataTransfer.setData('text/plain', dragText)
          event.dataTransfer.effectAllowed = 'copy'
        }}
      >
        <span className="workspace-tree-chevron" aria-hidden="true">{isDirectory ? icon : ''}</span>
        <span className={`workspace-tree-icon ${isDirectory ? 'folder' : extension(entry.name)}`} aria-hidden="true">
          {isDirectory ? '▰' : icon}
        </span>
        <span className="workspace-tree-name">{entry.name}</span>
      </button>
      {isDirectory && open && hasChildren && (
        <div className="workspace-tree-children">
          {entry.children.map((child) => (
            <TreeNode key={child.path} entry={child} level={level + 1} />
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
  }, [workspacePath])

  const startWorkspacePrompt = () => {
    onEnableAgentMode()
    onSetMessage('ดู workspace นี้แล้วช่วยวางแผนงานต่อไปให้หน่อย')
  }

  return (
    <section className="workspace-panel">
      <header className="workspace-panel-header">
        <div>
          <span className="workspace-kicker">Agent Workspace</span>
          <h2>Workspeac</h2>
        </div>
        <span className="workspace-agent-pill" data-state={sending ? 'thinking' : agentMode ? 'agent' : 'idle'}>
          {sending ? 'Running' : agentMode ? 'Agent mode' : 'Manual'}
        </span>
      </header>

      <div className="workspace-panel-actions">
        <button type="button" onClick={startWorkspacePrompt}>Use Agent</button>
        <button type="button" onClick={refresh} disabled={!workspacePath.trim()}>Refresh</button>
      </div>

      <div className="workspace-tree-shell">
        {error ? (
          <div className="workspace-tree-empty">{error}</div>
        ) : tree ? (
          <>
            <div className="workspace-root-row">
              <span className="workspace-tree-icon folder" aria-hidden="true">▰</span>
              <span>{tree.name}</span>
            </div>
            <div className="workspace-tree">
              {tree.children.map((entry) => (
                <TreeNode key={entry.path} entry={entry} level={0} />
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
