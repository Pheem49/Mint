import React, { useId, useRef, useState } from 'react'
import '../../css/settings/memory.css'

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
  const id = useId()
  const [editing, setEditing] = useState(false)
  const modeButton = useRef<HTMLButtonElement>(null)
  const hasPreferences = Boolean(userPreferences.trim())

  return (
    <div className="tab-pane active memory-tab">
      <header className="memory-intro">
        <p className="section-kicker">Across all sessions and projects</p>
        <h2>What Mint remembers about you</h2>
        <p>Your name and response preferences follow you in CLI, Web, and Desktop.</p>
      </header>

      <section className="memory-name-section" aria-labelledby={`${id}-name-label`}>
        <label id={`${id}-name-label`} htmlFor={`${id}-name`}>What should Mint call you?</label>
        <input
          id={`${id}-name`}
          type="text"
          autoComplete="nickname"
          value={userName}
          onChange={(e) => setUserName(e.target.value)}
          placeholder="Your name or nickname"
          aria-describedby={`${id}-name-hint`}
        />
        <p id={`${id}-name-hint`} className="memory-helper">The name Mint uses when talking with you.</p>
      </section>

      <section className="memory-instructions-section" aria-labelledby={`${id}-instructions-heading`}>
        <div className="memory-section-heading">
          <div>
            <h3 id={`${id}-instructions-heading`}>How Mint should respond</h3>
            <p className="memory-helper">Your preferred language, tone, and everyday instructions.</p>
          </div>
          <button
            ref={modeButton}
            type="button"
            className="btn-secondary memory-mode-button"
            aria-controls={`${id}-instructions`}
            onClick={() => {
              setEditing(!editing)
              if (editing) modeButton.current?.focus()
            }}
          >
            {editing ? 'Reading view' : hasPreferences ? 'Edit instructions' : 'Add instructions'}
          </button>
        </div>

        <div id={`${id}-instructions`} className="memory-instructions-content">
          {editing ? (
            <>
              <label className="memory-editor-label" htmlFor={`${id}-editor`}>Response preferences & instructions</label>
              <textarea
                id={`${id}-editor`}
                className="memory-editor"
                autoFocus
                rows={10}
                value={userPreferences}
                onChange={(e) => setUserPreferences(e.target.value)}
                placeholder={'Talk in Thai.\nKeep explanations concise.\nExplain coding concepts step by step.'}
                aria-describedby={`${id}-edit-hint`}
              />
              <p id={`${id}-edit-hint`} className="memory-helper">Write one instruction per line for easier reading. Select Save Settings to save your changes.</p>
            </>
          ) : hasPreferences ? (
            <div className="memory-reading-frame" tabIndex={0} role="region" aria-label="Response preferences and instructions">{userPreferences}</div>
          ) : (
            <div className="memory-empty">
              <p>No response preferences added yet.</p>
              <p className="memory-helper">Add the language, tone, or habits you want Mint to remember.</p>
            </div>
          )}
        </div>
      </section>

      <aside className="memory-scope-note" aria-label="About this memory">
        <strong>About this memory</strong>
        <p>This page contains your shared profile and preferences. Saved facts, project memory, and conversation history are stored separately.</p>
      </aside>
    </div>
  )
}
