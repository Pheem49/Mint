import React from 'react'

export default function ShortcutsTab() {
  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Keyboard</p>
            <h2 className="section-title">Shortcuts</h2>
          </div>
        </div>
        <div className="shortcut-list">
          <div className="shortcut-item">
            <span>Show / Hide Mint Window</span>
            <div className="keys"><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>Space</kbd></div>
          </div>
          <div className="shortcut-item">
            <span>Open Spotlight</span>
            <div className="keys"><kbd>Alt</kbd><kbd>Space</kbd></div>
          </div>
          <div className="shortcut-item">
            <span>Close / Dismiss</span>
            <div className="keys"><kbd>Esc</kbd></div>
          </div>
        </div>
      </section>
    </div>
  )
}
