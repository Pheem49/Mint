import { useState } from 'react'

export type DashboardView = 'chat' | 'pictures' | 'model' | 'imagine'

interface ChatSessionItem {
  id: string
  title: string
  kind: string
}

interface DashboardSidebarProps {
  view: DashboardView
  sidebarCollapsed: boolean
  sending: boolean
  chatSessions: ChatSessionItem[]
  activeConversationId: string
  onToggleSidebar: () => void
  onClearHistory: (action: 'New chat' | 'Clear history') => void
  onSelectConversation: (id: string) => void
  onDeleteConversation: (id: string) => void
  onRenameConversation?: (id: string, newTitle: string) => void
  onSetView: (view: DashboardView) => void
}

export default function DashboardSidebar({
  view,
  sidebarCollapsed,
  sending,
  chatSessions,
  activeConversationId,
  onToggleSidebar,
  onClearHistory,
  onSelectConversation,
  onDeleteConversation,
  onRenameConversation,
  onSetView,
}: DashboardSidebarProps) {
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null)
  const [editTitleValue, setEditTitleValue] = useState('')

  const handleSaveRename = (id: string) => {
    if (editTitleValue.trim() && editTitleValue.trim() !== chatSessions.find(s => s.id === id)?.title) {
      onRenameConversation?.(id, editTitleValue.trim())
    }
    setEditingSessionId(null)
  }
  const conversationSessions = chatSessions.filter((session) => session.kind !== 'cli' && session.id !== 'conversation-default')
  const cliSession = chatSessions.find((session) => session.kind === 'cli' || session.id === 'cli') ?? {
    id: 'cli',
    title: 'cli',
    kind: 'cli',
  }

  return (
    <aside className="workspace-sidebar">
      <div
        className="sidebar-brand clickable"
        onClick={onToggleSidebar}
        title={sidebarCollapsed ? "ขยายแถบด้านข้าง" : "ยุบแถบด้านข้าง"}
        role="button"
        tabIndex={0}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') onToggleSidebar()
        }}
      >
        <img src="./assets/icon.png" alt="Mint Agent Logo" className="sidebar-logo" />
        <span className="sidebar-brand-name">Mint Agent</span>
      </div>

      <button className="sidebar-new-chat" onClick={() => onClearHistory('New chat')}>
        <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <line x1="12" y1="5" x2="12" y2="19"></line>
            <line x1="5" y1="12" x2="19" y2="12"></line>
          </svg>
        </span>
        <span>New Chat</span>
      </button>

      <button className={`sidebar-top-action ${view === 'chat' ? 'is-active' : ''}`} onClick={() => onSetView('chat')}>
        <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
          </svg>
        </span>
        <span>Chat</span>
      </button>
      <button className={`sidebar-top-action ${view === 'pictures' ? 'is-active' : ''}`} onClick={() => onSetView('pictures')}>
        <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
            <circle cx="8.5" cy="8.5" r="1.5"></circle>
            <polyline points="21 15 16 10 5 21"></polyline>
          </svg>
        </span>
        <span>Pictures</span>
      </button>
      <button className={`sidebar-top-action ${view === 'imagine' ? 'is-active' : ''}`} onClick={() => onSetView('imagine')}>
        <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
          </svg>
        </span>
        <span>Image Studio</span>
      </button>

      <div className="sidebar-section">
        <div className="sidebar-section-title">Conversation CLI</div>
        <div className="sidebar-chat-list sidebar-cli-list">
          <button
            className={`sidebar-project sidebar-chat-item ${cliSession.id === activeConversationId ? 'active' : ''}`}
            onClick={() => onSelectConversation(cliSession.id)}
            title={cliSession.title}
          >
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M4 17l6-6-6-6"></path>
                <path d="M12 19h8"></path>
              </svg>
            </span>
            <span className="sidebar-chat-title">{cliSession.title || 'cli'}</span>
            {cliSession.id === activeConversationId && (
              <span className="mint-status-pill" data-state={sending ? "thinking" : "idle"}>
                <span className="mint-status-dot" />
                <span className="mint-status-label">{sending ? "Thinking" : "Idle"}</span>
              </span>
            )}
          </button>
        </div>
        <div className="sidebar-section-title sidebar-subsection-title">Conversations</div>
        <div className="sidebar-chat-list">
          {conversationSessions.map((session) => (
            <button
              key={session.id}
              className={`sidebar-project sidebar-chat-item ${session.id === activeConversationId ? 'active' : ''}`}
              onClick={() => onSelectConversation(session.id)}
              title={session.title}
            >
              <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
                </svg>
              </span>
              {editingSessionId === session.id ? (
                <input
                  type="text"
                  className="sidebar-chat-rename-input"
                  value={editTitleValue}
                  onChange={(e) => setEditTitleValue(e.target.value)}
                  onBlur={() => handleSaveRename(session.id)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      handleSaveRename(session.id)
                    } else if (e.key === 'Escape') {
                      setEditingSessionId(null)
                    }
                  }}
                  autoFocus
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <span className="sidebar-chat-title">{session.title || 'New chat'}</span>
              )}
              {session.id === activeConversationId && (
                <span className="mint-status-pill" data-state={sending ? "thinking" : "idle"}>
                  <span className="mint-status-dot" />
                  <span className="mint-status-label">{sending ? "Thinking" : "Idle"}</span>
                </span>
              )}
              {editingSessionId !== session.id && (
                <>
                  <span
                    className="sidebar-chat-edit"
                    role="button"
                    tabIndex={0}
                    title="Rename conversation"
                    onClick={(event) => {
                      event.stopPropagation()
                      setEditingSessionId(session.id)
                      setEditTitleValue(session.title || '')
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault()
                        event.stopPropagation()
                        setEditingSessionId(session.id)
                        setEditTitleValue(session.title || '')
                      }
                    }}
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M12 20h9"></path>
                      <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
                    </svg>
                  </span>
                  <span
                    className="sidebar-chat-delete"
                    role="button"
                    tabIndex={0}
                    title="Delete conversation"
                    onClick={(event) => {
                      event.stopPropagation()
                      onDeleteConversation(session.id)
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault()
                        event.stopPropagation()
                        onDeleteConversation(session.id)
                      }
                    }}
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="3 6 5 6 21 6"></polyline>
                      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"></path>
                      <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                    </svg>
                  </span>
                </>
              )}
            </button>
          ))}
        </div>
      </div>

      <div className="sidebar-bottom-actions">
        <button className="clear-btn" onClick={() => onClearHistory('Clear history')}>
          <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
              <line x1="10" y1="11" x2="10" y2="17"></line>
              <line x1="14" y1="11" x2="14" y2="17"></line>
            </svg>
          </span>
          <span>Clear Messages</span>
        </button>
        <button className="settings-btn" onClick={() => window.api.openSettings()}>
          <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="3"></circle>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
            </svg>
          </span>
          <span>Settings</span>
        </button>
      </div>
    </aside>
  )
}
