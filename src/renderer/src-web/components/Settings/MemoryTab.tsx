import React from 'react'

interface MemoryTabProps {
  userName: string
  setUserName: (val: string) => void
  userPreferences: string
  setUserPreferences: (val: string) => void
}

export default function MemoryTab({
  userName,
  setUserName,
  userPreferences,
  setUserPreferences
}: MemoryTabProps) {
  return (
    <div className="tab-pane active animate-fade-in" style={{ display: 'flex', flexDirection: 'column', gap: '24px' }}>
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Cross-session Memory</p>
            <h2 className="section-title">User Profile & Preferences</h2>
          </div>
          <p className="section-description">
            Information and preferences stored here will be remembered across all conversations. Mint automatically learns and updates your profile details from your chats in the background, but you can also edit them manually.
          </p>
        </div>

        <div className="form-grid single">
          <div className="setting-row">
            <label>Your Name / Nickname</label>
            <div className="memory-field-container">
              <input 
                type="text" 
                value={userName} 
                onChange={(e) => setUserName(e.target.value)} 
                placeholder="e.g. Pheem, Jane" 
              />
              <p className="hint">Used by the assistant to address you in conversation.</p>
            </div>
          </div>

          <div className="setting-row">
            <label>Custom Instructions & Preferences</label>
            <div className="memory-field-container">
              <textarea 
                value={userPreferences} 
                onChange={(e) => setUserPreferences(e.target.value)} 
                placeholder="e.g. Explain coding concepts step-by-step. Prefer TypeScript. Talk in Thai. Keep explanations concise." 
                style={{ minHeight: '120px', resize: 'vertical' }}
              />
              <p className="hint">Preferences, guidelines, or persona instructions for the assistant.</p>
            </div>
          </div>
        </div>
      </section>
    </div>
  )
}

