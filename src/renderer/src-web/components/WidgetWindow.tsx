import React, { useState, useEffect } from 'react'

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

  return (
    <div id="widget-container" className={`state-${state}`}>
      <div className="aura-container">
        <div className="aura"></div>
      </div>
      <div className="character-body">
        {/* Eyes / Face */}
        <div className="eyes">
          <div className="eye left"></div>
          <div className="eye right"></div>
        </div>
        {/* Mouth/Indicator */}
        <div className="mouth"></div>
      </div>
      {/* Status Badge */}
      <div className="status-badge" id="status-badge">
        {stateLabel}
      </div>
    </div>
  )
}
