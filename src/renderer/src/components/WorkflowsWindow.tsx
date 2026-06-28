import React from 'react'
import WorkflowBuilderPanel from './WorkflowBuilderPanel'

export default function WorkflowsWindow() {
  const handleClose = () => {
    window.settingsApi?.closeSettings()
  }

  const showToast = (msg: string) => {
    console.log("workflows toast:", msg)
  }

  return (
    <div className="settings-container" style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden' }}>
      <header className="settings-header drag-region">
        <div className="header-left">
          <span className="settings-icon" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M4 12h16"></path>
              <path d="M12 4v16"></path>
              <rect x="2" y="9" width="4" height="6" rx="1"></rect>
              <rect x="18" y="9" width="4" height="6" rx="1"></rect>
              <rect x="10" y="2" width="4" height="4" rx="1"></rect>
              <rect x="10" y="18" width="4" height="4" rx="1"></rect>
            </svg>
          </span>
          <div>
            <h1>Workflow Builder</h1>
            <p>Design visual automated workflows for your Mint assistant.</p>
          </div>
        </div>
        <button className="close-btn" onClick={handleClose} aria-label="Close">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
              stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </header>

      <main style={{ flex: 1, position: 'relative', overflow: 'hidden', background: '#090d16' }}>
        <WorkflowBuilderPanel view="workflows" onShowToast={showToast} />
      </main>
    </div>
  )
}
