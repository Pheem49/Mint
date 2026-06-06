export type DashboardView = 'chat' | 'pictures' | 'model'

interface DashboardSidebarProps {
  view: DashboardView
  sidebarCollapsed: boolean
  sending: boolean
  onToggleSidebar: () => void
  onClearHistory: (action: 'New chat' | 'Clear history') => void
  onSetView: (view: DashboardView) => void
  onShowToast: (message: string) => void
}

export default function DashboardSidebar({
  view,
  sidebarCollapsed,
  sending,
  onToggleSidebar,
  onClearHistory,
  onSetView,
  onShowToast,
}: DashboardSidebarProps) {
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
        <img src="./assets/icon.png" alt="Agent Mint Logo" className="sidebar-logo" />
        <span className="sidebar-brand-name">Agent Mint</span>
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

      <div className="sidebar-section">
        <div className="sidebar-section-title">Assistant</div>
        <button className="sidebar-project active">
          <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
            </svg>
          </span>
          <span>Mint</span>
          <span className="mint-status-pill" data-state={sending ? "thinking" : "idle"}>
            <span className="mint-status-dot" />
            <span className="mint-status-label">{sending ? "Thinking" : "Idle"}</span>
          </span>
        </button>
        <button className="sidebar-project" onClick={() => onShowToast("เปิดใช้งาน Smart Tools: พร้อมช่วยสแกนและวิเคราะห์แล้วค่ะ! ∽")}>
          <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="4" y="4" width="16" height="16" rx="2" ry="2"></rect>
              <rect x="9" y="9" width="6" height="6"></rect>
              <line x1="9" y1="1" x2="9" y2="4"></line>
              <line x1="15" y1="1" x2="15" y2="4"></line>
              <line x1="9" y1="20" x2="9" y2="23"></line>
              <line x1="15" y1="20" x2="15" y2="23"></line>
              <line x1="20" y1="9" x2="23" y2="9"></line>
              <line x1="20" y1="15" x2="23" y2="15"></line>
              <line x1="1" y1="9" x2="4" y2="9"></line>
              <line x1="1" y1="15" x2="4" y2="15"></line>
            </svg>
          </span>
          <span>Smart Tools</span>
        </button>
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
          <span>Clear</span>
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
