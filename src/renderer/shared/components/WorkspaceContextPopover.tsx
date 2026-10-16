import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { FileCode2, FolderOpen, GitBranch, RefreshCw, Terminal } from 'lucide-react'
import { conversationPlatform, workspacePlatform } from '../platform'
import type { AgentProgress, ConversationSources, FileChange, GitBranchInfo } from '../types'
import { collectSessionSources } from '../utils/sessionSources'
import SessionSources from './SessionSources'

interface WorkspaceContextPopoverProps {
  workspacePath: string
  terminalCount: number
  chatId: string
  liveProgress: AgentProgress[]
  recentChanges: FileChange[]
  onOpenWorkspace: () => void
  onOpenReview: (changes: FileChange[]) => void
  onRefreshWorkspace: () => void
}

function workspaceLabel(path: string) {
  const segments = path.replace(/\\/g, '/').split('/').filter(Boolean)
  return segments.at(-1) || 'No workspace'
}

export default function WorkspaceContextPopover({
  workspacePath,
  terminalCount,
  chatId,
  liveProgress,
  recentChanges,
  onOpenWorkspace,
  onOpenReview,
  onRefreshWorkspace,
}: WorkspaceContextPopoverProps) {
  const [open, setOpen] = useState(false)
  const [gitInfo, setGitInfo] = useState<GitBranchInfo | null>(null)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const [sourceData, setSourceData] = useState<{ chatId: string; value: ConversationSources } | null>(null)
  const [sourceError, setSourceError] = useState('')
  const [sourceLoading, setSourceLoading] = useState(false)
  const [sourceRetry, setSourceRetry] = useState(0)

  useEffect(() => {
    if (!open) return
    let disposed = false
    let busy = false
    setSourceData(null); setSourceError(''); setSourceLoading(true)
    const load = async () => {
      if (busy || document.hidden) return
      busy = true
      try {
        const value = await conversationPlatform.getConversationSources(chatId)
        if (!disposed) { setSourceData({ chatId, value }); setSourceError('') }
      } catch (error) { if (!disposed) setSourceError(String(error)) }
      finally { busy = false; if (!disposed) setSourceLoading(false) }
    }
    void load()
    const timer = window.setInterval(() => { void load() }, 2000)
    return () => { disposed = true; window.clearInterval(timer) }
  }, [open, chatId, sourceRetry])

  const refresh = useCallback(async () => {
    if (!workspacePath.trim()) {
      setGitInfo(null)
      return
    }
    setIsRefreshing(true)
    try {
      setGitInfo(await workspacePlatform.getGitBranchInfo(workspacePath))
    } catch {
      setGitInfo(null)
    } finally {
      setIsRefreshing(false)
    }
  }, [workspacePath])

  useEffect(() => {
    if (open) void refresh()
  }, [open, refresh])

  useEffect(() => {
    const closeOnOutsidePointer = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', closeOnOutsidePointer)
    window.addEventListener('keydown', closeOnEscape)
    return () => {
      document.removeEventListener('mousedown', closeOnOutsidePointer)
      window.removeEventListener('keydown', closeOnEscape)
    }
  }, [])

  const handleRefresh = async () => {
    onRefreshWorkspace()
    await refresh()
  }

  const additions = recentChanges.reduce((total, change) => total + Math.max(0, change.additions || 0), 0)
  const deletions = recentChanges.reduce((total, change) => total + Math.max(0, change.deletions || 0), 0)
  const hasReview = recentChanges.length > 0
  const sources = useMemo(() => open ? collectSessionSources(sourceData?.chatId === chatId ? sourceData.value : { attachments: [], webSearchProgress: [] }, liveProgress) : [], [open, sourceData, chatId, liveProgress])

  return (
    <div className="workspace-context" ref={rootRef}>
      <button
        type="button"
        className={`workspace-context-trigger${open ? ' is-open' : ''}`}
        aria-label="Open workspace context"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Workspace context"
        onClick={() => setOpen((current) => !current)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6.5h6l1.7 2H20v9.5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z"/><path d="M4 6.5v-1a2 2 0 0 1 2-2h4l1.7 2H18a2 2 0 0 1 2 2v1"/></svg>
      </button>
      {open && (
        <section className="workspace-context-popover" role="dialog" aria-label="Workspace context">
          <header className="workspace-context-heading">
            <span className="workspace-context-heading-title"><FolderOpen size={16} aria-hidden="true" /> Workspace context</span>
            <button type="button" onClick={handleRefresh} disabled={isRefreshing} title="Refresh workspace status" aria-label="Refresh workspace status">
              <RefreshCw className={isRefreshing ? 'is-refreshing' : ''} size={15} aria-hidden="true" />
            </button>
          </header>

          <button type="button" className="workspace-context-row" onClick={() => { onOpenWorkspace(); setOpen(false) }}>
            <span className="workspace-context-icon"><FolderOpen size={16} aria-hidden="true" /></span>
            <span className="workspace-context-copy"><strong>{workspaceLabel(workspacePath)}</strong><small title={workspacePath}>{workspacePath || 'Choose a project folder'}</small></span>
            <span className="workspace-context-action">Open</span>
          </button>

          <div className="workspace-context-row is-static">
            <span className="workspace-context-icon"><GitBranch size={16} aria-hidden="true" /></span>
            <span className="workspace-context-copy"><strong>{gitInfo?.isRepository ? (gitInfo.currentBranch || gitInfo.detachedHead || 'Detached HEAD') : 'No Git repository'}</strong><small>{gitInfo?.isRepository ? (gitInfo.isDirty ? 'Working tree has changes' : 'Working tree is clean') : 'Git status is unavailable'}</small></span>
            {gitInfo?.isRepository && <span className={`workspace-context-status${gitInfo.isDirty ? ' is-dirty' : ''}`}>{gitInfo.isDirty ? 'Changed' : 'Clean'}</span>}
          </div>

          <button type="button" className="workspace-context-row" disabled={!hasReview} onClick={() => { onOpenReview(recentChanges); setOpen(false) }}>
            <span className="workspace-context-icon"><FileCode2 size={16} aria-hidden="true" /></span>
            <span className="workspace-context-copy"><strong>Recent agent changes</strong><small>{hasReview ? `${recentChanges.length} file${recentChanges.length === 1 ? '' : 's'} changed in this run` : 'No changes in the current run'}</small></span>
            {hasReview && <span className="workspace-context-diff"><b>+{additions}</b><i>-{deletions}</i></span>}
          </button>

          <div className="workspace-context-row is-static">
            <span className="workspace-context-icon"><Terminal size={16} aria-hidden="true" /></span>
            <span className="workspace-context-copy"><strong>Terminal</strong><small>{terminalCount === 0 ? 'No terminal tabs open' : `${terminalCount} tab${terminalCount === 1 ? '' : 's'} open`}</small></span>
            <span className="workspace-context-status">{terminalCount}</span>
          </div>

          <SessionSources key={chatId} sources={sources} loading={sourceLoading} error={sourceError} onRetry={() => setSourceRetry(value => value + 1)} onOpenLibrary={url => {
            window.history.pushState({}, '', url)
            window.dispatchEvent(new PopStateEvent('popstate'))
            setOpen(false)
          }} />
        </section>
      )}
    </div>
  )
}
