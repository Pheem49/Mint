import React, { useEffect, useState } from 'react'
import '../styles/widget.css'

export default function WidgetWindow() {
  const [state, setState] = useState('idle')

  useEffect(() => {
    if (window.widgetAPI) {
      window.widgetAPI.onStateChange((newState) => {
        setState(newState || 'idle')
      })
    }
  }, [])

  return (
    <div id="widget-container" className={`state-${state} drag-region`}>
      <div className="aura-container">
        <div className="aura" />
      </div>
      <div className="character-body">
        <div className="eyes">
          <div className="eye" />
          <div className="eye" />
        </div>
        <div className="mouth" />
      </div>
      <div className="status-badge" id="status-badge">
        {state.charAt(0).toUpperCase() + state.slice(1)}
      </div>
    </div>
  )
}
