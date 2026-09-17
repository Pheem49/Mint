import React, { useState, useMemo, useEffect, useRef } from 'react'
import type { ChatSession } from '../types'
import { parseUtcDate } from '../utils/ui'
import '../css/code-sessions.css'

export interface CliSessionsViewProps {
  chatSessions: ChatSession[]
  activeConversationId: string
  workspacePath?: string
  onSelectSession: (id: string) => void
  onDeleteSession: (id: string) => void
  onRenameSession: (id: string, newTitle: string) => void
  onRefreshSessions?: () => void
  onShowToast?: (message: string) => void
}

function formatRelativeTime(dateInput: unknown): string {
  const date = parseUtcDate(dateInput)
  const now = new Date()
  const diffSecs = Math.floor((now.getTime() - date.getTime()) / 1000)

  if (diffSecs < 60) return 'Just now'
  if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`
  if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`
  if (diffSecs < 86400 * 7) return `${Math.floor(diffSecs / 86400)}d ago`
  return date.toLocaleDateString([], { month: 'short', day: 'numeric' })
}

function formatBytes(bytes?: number): string {
  if (!bytes || bytes <= 0) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function getFolderBasename(path?: string | null): string {
  if (!path) return 'Global'
  const trimmed = path.replace(/[/\\]+$/, '')
  const parts = trimmed.split(/[/\\]/)
  return parts[parts.length - 1] || path
}

export const CliSessionsView: React.FC<CliSessionsViewProps> = React.memo(function CliSessionsView({
  chatSessions,
  activeConversationId,
  workspacePath,
  onSelectSession,
  onDeleteSession,
  onRenameSession,
  onRefreshSessions,
  onShowToast,
}) {
  const [searchQuery, setSearchQuery] = useState('')
  const [scopeFilter, setScopeFilter] = useState<'workspace' | 'all'>('workspace')
  const [sortBy, setSortBy] = useState<'recent' | 'messages' | 'oldest'>('recent')
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null)
  const [editTitleValue, setEditTitleValue] = useState('')

  const onRefreshRef = useRef(onRefreshSessions)
  useEffect(() => {
    onRefreshRef.current = onRefreshSessions
  }, [onRefreshSessions])

  useEffect(() => {
    onRefreshRef.current?.()
    const handleFocus = () => {
      onRefreshRef.current?.()
    }
    window.addEventListener('focus', handleFocus)
    return () => window.removeEventListener('focus', handleFocus)
  }, [])

  // Filter only CLI sessions (kind === 'cli' or id starts with 'cli')
  const allCliSessions = useMemo(() => {
    return chatSessions.filter((s) => s.kind === 'cli' || s.id.startsWith('cli'))
  }, [chatSessions])

  const filteredSessions = useMemo(() => {
    return allCliSessions
      .filter((s) => {
        if (scopeFilter === 'workspace' && workspacePath && s.workspacePath) {
          if (s.workspacePath !== workspacePath) return false
        }

        if (!searchQuery.trim()) return true
        const q = searchQuery.toLowerCase()
        return (
          s.title.toLowerCase().includes(q) ||
          s.id.toLowerCase().includes(q) ||
          (s.workspacePath && s.workspacePath.toLowerCase().includes(q)) ||
          (s.gitBranch && s.gitBranch.toLowerCase().includes(q)) ||
          (s.mainLanguage && s.mainLanguage.toLowerCase().includes(q))
        )
      })
      .sort((a, b) => {
        if (sortBy === 'messages') {
          return (b.messageCount || 0) - (a.messageCount || 0)
        }
        const timeA = parseUtcDate(a.updatedAt || a.createdAt).getTime()
        const timeB = parseUtcDate(b.updatedAt || b.createdAt).getTime()
        if (sortBy === 'oldest') {
          return timeA - timeB
        }
        return timeB - timeA
      })
  }, [allCliSessions, scopeFilter, workspacePath, searchQuery, sortBy])

  const handleCopyResumeCommand = (e: React.MouseEvent, sessionId: string) => {
    e.stopPropagation()
    const cmd = `mint --resume ${sessionId}`
    if (navigator?.clipboard?.writeText) {
      navigator.clipboard.writeText(cmd)
      onShowToast?.(`Copied: ${cmd}`)
    } else {
      onShowToast?.(`Resume command: ${cmd}`)
    }
  }

  const handleStartRename = (e: React.MouseEvent, session: ChatSession) => {
    e.stopPropagation()
    setEditingSessionId(session.id)
    setEditTitleValue(session.title)
  }

  const handleSaveRename = (e?: React.FormEvent) => {
    if (e) e.preventDefault()
    if (!editingSessionId) return
    const trimmed = editTitleValue.trim()
    if (trimmed) {
      onRenameSession(editingSessionId, trimmed)
      onShowToast?.('Session renamed')
    }
    setEditingSessionId(null)
  }

  const handleDelete = (e: React.MouseEvent, session: ChatSession) => {
    e.stopPropagation()
    onDeleteSession(session.id)
  }

  return (
    <div className="code-sessions-container">
      {/* Header */}
      <div className="code-sessions-header">
        <div className="code-header-main">
          <div className="code-header-title-row">
            <h1 className="code-header-title">
              <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.2"
                strokeLinecap="round"
                strokeLinejoin="round"
                style={{ color: 'var(--accent, #10b981)' }}
              >
                <polyline points="4 17 10 11 4 5"></polyline>
                <line x1="12" y1="19" x2="20" y2="19"></line>
              </svg>
              <span>Code Sessions</span>
            </h1>
            <span className="code-header-count">
              {allCliSessions.length} {allCliSessions.length === 1 ? 'session' : 'sessions'}
            </span>
          </div>
          <p className="code-header-subtitle">
            Terminal conversations and agent runs from your active repositories.
          </p>
        </div>

        {/* Right side controls: Scope switch & refresh */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
          <div className="code-segmented-toggle">
            <button
              type="button"
              className={`code-segmented-btn ${scopeFilter === 'workspace' ? 'active' : ''}`}
              onClick={() => setScopeFilter('workspace')}
              title="Show sessions from current workspace"
            >
              Current Project
            </button>
            <button
              type="button"
              className={`code-segmented-btn ${scopeFilter === 'all' ? 'active' : ''}`}
              onClick={() => setScopeFilter('all')}
              title="Show all sessions across all repositories"
            >
              All Repositories
            </button>
          </div>

          <select
            className="code-sort-select"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as any)}
            title="Sort sessions"
          >
            <option value="recent">Recently Active</option>
            <option value="messages">Most Turns</option>
            <option value="oldest">Oldest First</option>
          </select>

          {onRefreshSessions && (
            <button
              type="button"
              className="code-icon-btn"
              onClick={onRefreshSessions}
              title="Refresh sessions list"
              style={{ width: '32px', height: '32px' }}
            >
              <svg
                width="14"
                height="14"
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
          )}
        </div>
      </div>

      {/* Search Input Bar */}
      <div className="code-controls-bar">
        <div className="code-search-box">
          <svg
            className="code-search-icon"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
          <input
            type="text"
            className="code-search-input"
            placeholder="Filter sessions by branch, prompt, repository..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
      </div>

      {/* Sessions Content: Precision Developer List */}
      {filteredSessions.length === 0 ? (
        <div className="code-empty-terminal">
          <div className="code-terminal-header">
            <span className="code-terminal-dot" />
            <span className="code-terminal-dot" />
            <span className="code-terminal-dot" />
            <span className="code-terminal-title">mint-cli ~ interactive sessions</span>
          </div>
          <div className="code-terminal-body">
            <p className="code-terminal-text">
              {searchQuery
                ? 'No terminal sessions match your query.'
                : scopeFilter === 'workspace' && allCliSessions.length > 0
                ? 'No sessions found in this project. Switch to "All Repositories" or start one in this directory:'
                : 'No terminal sessions recorded yet. Launch Mint in any repository to begin:'}
            </p>
            <div className="code-terminal-codeblock">
              <span>$ mint</span>
              <button
                type="button"
                className="code-resume-chip"
                onClick={() => {
                  navigator?.clipboard?.writeText('mint')
                  onShowToast?.('Copied: mint')
                }}
              >
                Copy
              </button>
            </div>
          </div>
        </div>
      ) : (
        <div className="code-sessions-list">
          {filteredSessions.map((session) => {
            const isActive = session.id === activeConversationId
            const folderName = getFolderBasename(session.workspacePath)
            const isEditing = editingSessionId === session.id
            const displayId = session.id.replace(/^cli::/, '')
            const shortId = displayId === 'cli' ? 'cli' : `cli::${displayId.slice(0, 7)}`

            return (
              <div
                key={session.id}
                className={`code-session-row ${isActive ? 'is-active' : ''}`}
                onClick={() => onSelectSession(session.id)}
              >
                {/* Column 1: Identity & Git Metadata */}
                <div className="code-row-meta">
                  <div className="code-row-meta-top">
                    <span className="code-session-id" title={session.id}>
                      {shortId}
                    </span>
                    {isActive && (
                      <span className="code-active-badge">active</span>
                    )}
                  </div>

                  <div className="code-row-meta-sub">
                    {session.gitBranch && (
                      <span className="code-branch-tag" title={`Branch: ${session.gitBranch}`}>
                        <svg
                          width="11"
                          height="11"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2.4"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        >
                          <line x1="6" y1="3" x2="6" y2="15" />
                          <circle cx="18" cy="6" r="3" />
                          <circle cx="6" cy="18" r="3" />
                          <path d="M18 9a9 9 0 0 1-9 9" />
                        </svg>
                        <span>{session.gitBranch}</span>
                      </span>
                    )}
                    <span className="code-folder-name" title={session.workspacePath || 'Global'}>
                      {folderName}
                    </span>
                    {session.mainLanguage && (
                      <span style={{ opacity: 0.6, fontSize: '0.7rem' }}>
                        · {session.mainLanguage}
                      </span>
                    )}
                  </div>
                </div>

                {/* Column 2: Content / Task Title */}
                <div className="code-row-content">
                  {isEditing ? (
                    <div
                      onClick={(e) => e.stopPropagation()}
                      style={{ display: 'flex', gap: '6px' }}
                    >
                      <input
                        type="text"
                        autoFocus
                        value={editTitleValue}
                        onChange={(e) => setEditTitleValue(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') handleSaveRename()
                          if (e.key === 'Escape') setEditingSessionId(null)
                        }}
                        style={{
                          flex: 1,
                          background: 'rgba(0, 0, 0, 0.4)',
                          border: '1px solid var(--border-light, rgba(255, 255, 255, 0.2))',
                          borderRadius: 'var(--radius-xs, 6px)',
                          color: '#fff',
                          padding: '4px 8px',
                          fontSize: '0.88rem',
                          outline: 'none',
                        }}
                      />
                      <button
                        type="button"
                        onClick={handleSaveRename}
                        style={{
                          background: 'var(--surface-strong, #3f3f46)',
                          color: '#fff',
                          border: 'none',
                          borderRadius: 'var(--radius-xs, 6px)',
                          padding: '4px 10px',
                          fontSize: '0.78rem',
                          cursor: 'pointer',
                        }}
                      >
                        Save
                      </button>
                    </div>
                  ) : (
                    <>
                      <div className="code-session-title" title={session.title}>
                        {session.title || 'Untitled Session'}
                      </div>
                      <div className="code-session-snippet">
                        <span style={{ color: 'var(--accent, #10b981)', opacity: 0.8, marginRight: '4px' }}>
                          &gt;
                        </span>
                        <span>{session.title || 'Interactive shell'}</span>
                      </div>
                    </>
                  )}
                </div>

                {/* Column 3: Stats, Resume Chip, Actions */}
                <div className="code-row-actions">
                  <span className="code-metric-turns">
                    {session.messageCount ?? 0}t · {formatBytes(session.totalBytes)}
                  </span>

                  <span className="code-metric-time">
                    {formatRelativeTime(session.updatedAt || session.createdAt)}
                  </span>

                  {/* 1-Click Copy Resume Command */}
                  <button
                    type="button"
                    className="code-resume-chip"
                    onClick={(e) => handleCopyResumeCommand(e, session.id)}
                    title={`Copy command: mint --resume ${session.id}`}
                  >
                    <span>$</span>
                    <span>resume</span>
                  </button>

                  {/* Rename Icon */}
                  <button
                    type="button"
                    className="code-icon-btn"
                    onClick={(e) => handleStartRename(e, session)}
                    title="Rename session"
                  >
                    <svg
                      width="13"
                      height="13"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    >
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                    </svg>
                  </button>

                  {/* Delete Icon */}
                  <button
                    type="button"
                    className="code-icon-btn destructive"
                    onClick={(e) => handleDelete(e, session)}
                    title="Delete session"
                  >
                    <svg
                      width="13"
                      height="13"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    >
                      <polyline points="3 6 5 6 21 6" />
                      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                  </button>

                  {/* Arrow Indicator */}
                  <span className="code-open-arrow">
                    <svg
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2.2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    >
                      <polyline points="9 18 15 12 9 6" />
                    </svg>
                  </span>
                </div>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
})

export default CliSessionsView
