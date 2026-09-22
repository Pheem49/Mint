import React, { useState, useEffect } from 'react'
import '../css/widget.css'
import { getCurrentWindow } from '@tauri-apps/api/window'

export default function WidgetWindow() {
  const [state, setState] = useState('idle')
  useEffect(() => {
    if (window.widgetAPI?.onStateChange) {
      window.widgetAPI.onStateChange((newState: string) => {
        setState(newState || 'idle')
      })
    }
  }, [])

  const stateLabel = state.charAt(0).toUpperCase() + state.slice(1)

  const handleMouseDown = (event: React.MouseEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return
    void getCurrentWindow().startDragging().catch((error) => {
      console.error('Failed to drag Assistant Presence:', error)
    })
  }

  return (
    <div id="widget-container" className={`state-${state}`}>
      <div className="aura-container">
        <div className="aura"></div>
      </div>
      <button
        type="button"
        className="character-body"
        aria-label={`Open Mint chat. Assistant is ${stateLabel.toLowerCase()}.`}
        title="Open Mint chat"
        onClick={() => void window.widgetAPI.openChat()}
        onMouseDown={handleMouseDown}
      >
        {/* Eyes / Face */}
        <div className="eyes">
          <div className="eye left"></div>
          <div className="eye right"></div>
        </div>
        {/* Mouth/Indicator */}
        <div className="mouth"></div>
      </button>
      <button
        type="button"
        className="widget-hide-btn"
        aria-label="Turn off Assistant Presence"
        title="Turn off Assistant Presence"
        onClick={() => void window.widgetAPI.setVisible(false)}
      >
        ×
      </button>
      {/* Status Badge */}
      <div className="status-badge" id="status-badge">
        {stateLabel}
      </div>
    </div>
  )
}
