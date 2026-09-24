import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { PointerEvent as ReactPointerEvent } from 'react'
import type { FileChange, GitBranchInfo } from '../types'
import { workspacePlatform } from '../platform'
import GitBranchSelector from './GitBranchSelector'
import {
  materialFolderIcon,
  materialFileIcon,
  getExtension,
  folderOpenIcon,
  folderIcon,
  documentIcon,
} from '../utils/fileIcons'
import '../css/code-review.css'

interface Props {
  title: string
  changes?: FileChange[]
  workspacePath?: string | null
  onBack: () => void
  onOpenFiles?: () => void
}

function diffLines(text: string) {
  if (!text) return []
  return text.replace(/\n$/, '').split('\n')
}

interface TreeNode {
  name: string
  path: string
  isDir: boolean
  children?: TreeNode[]
  change?: FileChange
}

function buildTree(changesList: FileChange[]): TreeNode[] {
  const root: { [key: string]: any } = {}

  for (const change of changesList) {
    const parts = change.path.split('/').filter(Boolean)
    let current = root
    let accumulatedPath = ''

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]
      accumulatedPath = accumulatedPath ? `${accumulatedPath}/${part}` : part
      const isFile = i === parts.length - 1

      if (!current[part]) {
        current[part] = {
          name: part,
          path: accumulatedPath,
          isDir: !isFile,
          children: isFile ? undefined : {},
          change: isFile ? change : undefined,
        }
      }

      if (!isFile) {
        current = current[part].children
      }
    }
  }

  function convert(obj: { [key: string]: any }): TreeNode[] {
    return Object.values(obj).map((node) => {
      if (node.isDir && node.children) {
        return {
          ...node,
          children: convert(node.children),
        }
      }
      return node
    })
  }

  return convert(root)
}

export default function CodeReviewPage({
  changes,
  workspacePath,
  onBack,
  onOpenFiles,
}: Props) {
  const [workspaceDiffs, setWorkspaceDiffs] = useState<FileChange[]>([])
  const [isLoadingDiff, setIsLoadingDiff] = useState(false)
  const [gitInfo, setGitInfo] = useState<GitBranchInfo | null>(null)
  const hasAgentChanges = Boolean(changes && changes.length > 0)

  const [diffScope, setDiffScope] = useState<'workspace' | 'agent'>(
    hasAgentChanges ? 'agent' : 'workspace'
  )
  const [filterQuery, setFilterQuery] = useState('')
  const [collapsedFolders, setCollapsedFolders] = useState<Record<string, boolean>>({})

  // Fetch real Git diff and branch info from workspace
  const loadWorkspaceDiff = useCallback(async () => {
    if (!workspacePath) return
    setIsLoadingDiff(true)
    try {
      const [diffs, branch] = await Promise.all([
        workspacePlatform.getWorkspaceGitDiff(workspacePath),
        workspacePlatform.getGitBranchInfo(workspacePath).catch(() => null),
      ])
      setWorkspaceDiffs(diffs || [])
      if (branch) setGitInfo(branch)
    } catch (e) {
      console.error('Failed to load workspace git diff:', e)
    } finally {
      setIsLoadingDiff(false)
    }
  }, [workspacePath])

  useEffect(() => {
    loadWorkspaceDiff()
  }, [loadWorkspaceDiff])

  const activeChanges = useMemo(() => {
    if (diffScope === 'agent' && hasAgentChanges) {
      return changes!
    }
    if (workspaceDiffs.length > 0) {
      return workspaceDiffs
    }
    if (hasAgentChanges) {
      return changes!
    }
    return []
  }, [diffScope, hasAgentChanges, changes, workspaceDiffs])

  const filteredChanges = useMemo(() => {
    if (!filterQuery.trim()) return activeChanges
    const q = filterQuery.toLowerCase()
    return activeChanges.filter((c) => c.path.toLowerCase().includes(q))
  }, [activeChanges, filterQuery])

  const [selectedPath, setSelectedPath] = useState(filteredChanges[0]?.path ?? '')

  useEffect(() => {
    if (filteredChanges.length > 0 && !filteredChanges.some((c) => c.path === selectedPath)) {
      setSelectedPath(filteredChanges[0].path)
    }
  }, [filteredChanges, selectedPath])

  const currentIndex = Math.max(0, filteredChanges.findIndex((c) => c.path === selectedPath))
  const selectedChange = filteredChanges[currentIndex] || filteredChanges[0]

  const totalAdditions = activeChanges.reduce((sum, change) => sum + change.additions, 0)
  const totalDeletions = activeChanges.reduce((sum, change) => sum + change.deletions, 0)

  const tree = useMemo(() => buildTree(filteredChanges), [filteredChanges])

  // Split resizer state
  const splitContainerRef = useRef<HTMLDivElement | null>(null)
  const [treeWidth, setTreeWidth] = useState(250)
  const [isDragging, setIsDragging] = useState(false)
  const [isTreeCollapsed, setIsTreeCollapsed] = useState(false)

  const handleSplitResizeStart = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return
    event.preventDefault()
    setIsDragging(true)

    const startX = event.clientX
    const startWidth = isTreeCollapsed ? 0 : treeWidth
    const container = splitContainerRef.current
    const containerWidth = container?.getBoundingClientRect().width ?? 600

    const onMove = (moveEvent: PointerEvent) => {
      const deltaX = startX - moveEvent.clientX
      const proposedWidth = startWidth + deltaX

      if (proposedWidth < 70) {
        setIsTreeCollapsed(true)
      } else {
        setIsTreeCollapsed(false)
        const minW = 100
        const maxW = Math.max(minW, containerWidth - 100)
        const nextWidth = Math.max(minW, Math.min(maxW, proposedWidth))
        setTreeWidth(Math.round(nextWidth))
      }
    }

    const onUp = () => {
      setIsDragging(false)
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }

    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp, { once: true })
  }

  const handleResetSplit = () => {
    if (isTreeCollapsed) {
      setIsTreeCollapsed(false)
      setTreeWidth(250)
    } else if (treeWidth === 250) {
      setTreeWidth(320)
    } else {
      setTreeWidth(250)
    }
  }

  const handleResizerKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'ArrowLeft') {
      setIsTreeCollapsed(false)
      setTreeWidth((w) => Math.min(500, w + 20))
    } else if (e.key === 'ArrowRight') {
      setTreeWidth((w) => {
        if (w <= 110) {
          setIsTreeCollapsed(true)
          return 100
        }
        return Math.max(100, w - 20)
      })
    } else if (e.key === 'Enter' || e.key === ' ') {
      handleResetSplit()
    }
  }

  const toggleFolder = (path: string) => {
    setCollapsedFolders((prev) => ({ ...prev, [path]: !prev[path] }))
  }

  const handlePrevFile = () => {
    if (currentIndex > 0) {
      setSelectedPath(filteredChanges[currentIndex - 1].path)
    }
  }

  const handleNextFile = () => {
    if (currentIndex < filteredChanges.length - 1) {
      setSelectedPath(filteredChanges[currentIndex + 1].path)
    }
  }

  const renderTreeNodes = (nodes: TreeNode[], depth = 0) => {
    return nodes.map((node) => {
      if (node.isDir) {
        const isCollapsed = Boolean(collapsedFolders[node.path])
        const folderIconSrc = materialFolderIcon(node.name, !isCollapsed) || (isCollapsed ? folderIcon : folderOpenIcon)
        return (
          <div className="code-review-tree-folder" key={node.path}>
            <button
              type="button"
              className="code-review-tree-folder-btn"
              style={{ paddingLeft: `${depth * 14 + 10}px` }}
              onClick={() => toggleFolder(node.path)}
              aria-expanded={!isCollapsed}
            >
              <span className="code-review-chevron">{isCollapsed ? '›' : '⌄'}</span>
              <span className="code-review-tree-icon material-icon folder" aria-hidden="true">
                {folderIconSrc && <img src={folderIconSrc} alt="" draggable={false} />}
              </span>
              <span className="code-review-folder-name">{node.name}</span>
              <span className="code-review-mod-dot" title="Modified in this folder" aria-hidden="true">•</span>
            </button>
            {!isCollapsed && node.children && (
              <div className="code-review-tree-sub">
                {renderTreeNodes(node.children, depth + 1)}
              </div>
            )}
          </div>
        )
      }

      const isSelected = node.path === selectedPath
      const ext = getExtension(node.name)
      const fileIconSrc = materialFileIcon(node.name, ext) || documentIcon
      const isCreated = Boolean(node.change?.created)

      return (
        <button
          type="button"
          key={node.path}
          className={`code-review-tree-file${isSelected ? ' is-selected' : ''}`}
          style={{ paddingLeft: `${depth * 14 + 12}px` }}
          onClick={() => setSelectedPath(node.path)}
          title={node.path}
        >
          <span className="code-review-tree-icon material-icon file" aria-hidden="true">
            {fileIconSrc && <img src={fileIconSrc} alt="" draggable={false} />}
          </span>
          <span className="code-review-tree-file-name">{node.name}</span>
          <span className={`code-review-file-status-icon ${isCreated ? 'is-created' : 'is-modified'}`} aria-hidden="true">
            {isCreated ? 'A' : 'M'}
          </span>
        </button>
      )
    })
  }

  return (
    <main className="code-review-page">
      {/* Top branch & actions toolbar */}
      <header className="code-review-top-bar">
        <div className="code-review-branch-group">
          {workspacePath ? (
            <GitBranchSelector
              workspacePath={workspacePath}
              onBranchChanged={loadWorkspaceDiff}
            />
          ) : (
            <span className="code-review-branch-static">
              {gitInfo?.currentBranch || 'Review'}
            </span>
          )}
          <span className="code-review-branch-stat-add">+{totalAdditions.toLocaleString()}</span>
          <span className="code-review-branch-stat-del">-{totalDeletions.toLocaleString()}</span>
        </div>

        <div className="code-review-top-actions">
          {/* Refresh button */}
          <button
            type="button"
            className="code-review-icon-btn"
            onClick={loadWorkspaceDiff}
            disabled={isLoadingDiff}
            title="Refresh changes"
            aria-label="Refresh changes"
          >
            <svg
              className={isLoadingDiff ? 'code-review-spin' : ''}
              width="15"
              height="15"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
            </svg>
          </button>

          {/* Workspace files button */}
          {onOpenFiles && (
            <button
              type="button"
              className="code-review-icon-btn"
              onClick={onOpenFiles}
              title="Open workspace files (Ctrl+P)"
              aria-label="Open workspace files"
            >
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M3 6h7l2 2h9v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
                <path d="M3 6v12" />
              </svg>
            </button>
          )}

          {/* Sidebar tree toggle button */}
          <button
            type="button"
            className={`code-review-icon-btn ${!isTreeCollapsed ? 'is-active' : ''}`}
            onClick={() => setIsTreeCollapsed((prev) => !prev)}
            aria-label={isTreeCollapsed ? 'Show file tree' : 'Hide file tree'}
            title={isTreeCollapsed ? 'Show file tree' : 'Hide file tree'}
          >
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <path d="M15 3v18" />
            </svg>
          </button>

          {/* Close button */}
          <button type="button" className="code-review-close-btn" onClick={onBack} aria-label="Close review" title="Close review">
            ×
          </button>
        </div>
      </header>

      {/* Large diff / status banner */}
      <div className="code-review-banner">
        <div className="code-review-banner-left">
          <svg className="code-review-banner-info-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="10" />
            <path d="M12 16v-4M12 8h.01" />
          </svg>

          {hasAgentChanges && workspaceDiffs.length > 0 ? (
            <div className="code-review-scope-group">
              <button
                type="button"
                className={`code-review-scope-btn ${diffScope === 'workspace' ? 'is-active' : ''}`}
                onClick={() => setDiffScope('workspace')}
              >
                Workspace Git ({workspaceDiffs.length})
              </button>
              <button
                type="button"
                className={`code-review-scope-btn ${diffScope === 'agent' ? 'is-active' : ''}`}
                onClick={() => setDiffScope('agent')}
              >
                AI Edits ({changes?.length || 0})
              </button>
            </div>
          ) : (
            <span>
              {isLoadingDiff
                ? 'Scanning workspace Git changes...'
                : filteredChanges.length > 0
                ? `${filteredChanges.length} changed file${filteredChanges.length === 1 ? '' : 's'} in workspace`
                : 'Working tree is clean · No uncommitted changes'}
            </span>
          )}
        </div>

        {filteredChanges.length > 1 && (
          <div className="code-review-pagination">
            <button
              type="button"
              className="code-review-page-arrow"
              onClick={handlePrevFile}
              disabled={currentIndex <= 0}
              aria-label="Previous file"
              title="Previous file"
            >
              ‹
            </button>
            <button
              type="button"
              className="code-review-page-arrow"
              onClick={handleNextFile}
              disabled={currentIndex >= filteredChanges.length - 1}
              aria-label="Next file"
              title="Next file"
            >
              ›
            </button>
          </div>
        )}
      </div>

      {/* Main split view: Diff code (Left) + Resizer + File Tree (Right) */}
      <div
        className={`code-review-split ${isDragging ? 'is-resizing' : ''}`}
        ref={splitContainerRef}
      >
        {/* Left: Code Diff Pane */}
        <section className="code-review-diff-pane" aria-label="Diff viewer">
          {selectedChange ? (
            <article className="code-review-file-view" key={selectedChange.path}>
              <div className="code-review-file-bar">
                <span className="code-review-tree-icon material-icon file" aria-hidden="true">
                  <img src={materialFileIcon(selectedChange.path.split('/').pop() || '', getExtension(selectedChange.path)) || documentIcon} alt="" draggable={false} />
                </span>
                <span className={`code-review-file-bar-status ${selectedChange.created ? 'is-created' : 'is-modified'}`}>
                  {selectedChange.created ? 'A' : 'M'}
                </span>
                <span className="code-review-file-bar-path" title={selectedChange.path}>
                  {selectedChange.path}
                </span>
                <span className="code-review-file-bar-stat-add">+{selectedChange.additions}</span>
                <span className="code-review-file-bar-stat-del">-{selectedChange.deletions}</span>
              </div>

              <div className="code-review-code-container">
                {selectedChange.hunks && selectedChange.hunks.length > 0 ? (
                  selectedChange.hunks.map((hunk, hunkIdx) => {
                    const oldLines = diffLines(hunk.oldText)
                    const newLines = diffLines(hunk.newText)
                    let lineCounter = 1
                    return (
                      <div className="code-review-hunk-block" key={`${selectedChange.path}-hunk-${hunkIdx}`}>
                        {oldLines.map((line, idx) => (
                          <div className="code-review-line is-del" key={`del-${idx}`}>
                            <span className="code-review-gutter">{lineCounter++}</span>
                            <span className="code-review-sign">−</span>
                            <code className="code-review-code-text">{line || ' '}</code>
                          </div>
                        ))}
                        {newLines.map((line, idx) => (
                          <div className="code-review-line is-add" key={`add-${idx}`}>
                            <span className="code-review-gutter">{lineCounter++}</span>
                            <span className="code-review-sign">+</span>
                            <code className="code-review-code-text">{line || ' '}</code>
                          </div>
                        ))}
                      </div>
                    )
                  })
                ) : (
                  <div className="code-review-empty-diff">No diff hunks available for this file.</div>
                )}
              </div>
            </article>
          ) : (
            <div className="code-review-clean-state">
              <svg width="44" height="44" viewBox="0 0 24 24" fill="none" stroke="#22c55e" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                <polyline points="22 4 12 14.01 9 11.01" />
              </svg>
              <h3>Working tree is clean</h3>
              <p>No uncommitted changes found in this workspace.</p>
              <button type="button" className="code-review-clean-refresh-btn" onClick={loadWorkspaceDiff}>
                ↻ Refresh Git status
              </button>
            </div>
          )}
        </section>

        {/* Resizer Divider */}
        {filteredChanges.length > 0 && (
          <div
            className={`code-review-resizer ${isDragging ? 'is-dragging' : ''} ${isTreeCollapsed ? 'is-collapsed' : ''}`}
            onPointerDown={handleSplitResizeStart}
            onDoubleClick={handleResetSplit}
            onKeyDown={handleResizerKeyDown}
            role="separator"
            tabIndex={0}
            aria-orientation="vertical"
            aria-valuenow={isTreeCollapsed ? 0 : treeWidth}
            aria-label="Resize panel divider"
            title={isTreeCollapsed ? 'Drag or double-click to expand file list' : 'Drag to resize file list / Double click to reset'}
          >
            <div className="code-review-resizer-line" />
            <div className="code-review-resizer-handle" />
          </div>
        )}

        {/* Right: File Tree Pane */}
        {filteredChanges.length > 0 && !isTreeCollapsed && (
          <aside
            className="code-review-tree-pane"
            style={{ width: `${treeWidth}px` }}
            aria-label="Changed files tree"
          >
            <div className="code-review-search-wrap">
              <svg className="code-review-search-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.3-4.3" />
              </svg>
              <input
                type="text"
                className="code-review-search-input"
                placeholder="Filter files..."
                value={filterQuery}
                onChange={(e) => setFilterQuery(e.target.value)}
                spellCheck={false}
              />
              <button
                type="button"
                className="code-review-tree-collapse-btn"
                onClick={() => setIsTreeCollapsed(true)}
                title="Collapse file list"
                aria-label="Collapse file list"
              >
                ›
              </button>
            </div>

            <div className="code-review-tree-list" role="tree">
              {renderTreeNodes(tree)}
            </div>
          </aside>
        )}
      </div>
    </main>
  )
}
