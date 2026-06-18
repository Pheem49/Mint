import React, { useState, useEffect, useRef } from 'react'
import { evaluateArithmetic } from '../calculator'

const COMMANDS = [
    { label: 'Open YouTube', desc: 'เปิดเว็บไซต์ YouTube', icon: '📺', action: { type: 'open_url', target: 'https://youtube.com' } },
    { label: 'Open Facebook', desc: 'เปิดเว็บไซต์ Facebook', icon: '📘', action: { type: 'open_url', target: 'https://facebook.com' } },
    { label: 'Open Instagram', desc: 'เปิดเว็บไซต์ Instagram', icon: '📸', action: { type: 'open_url', target: 'https://instagram.com' } },
    { label: 'Open GitHub', desc: 'เปิดเว็บไซต์ GitHub', icon: '🐙', action: { type: 'open_url', target: 'https://github.com' } },
    { label: 'System Info', desc: 'ดูข้อมูลระบบ', icon: '💻', action: { type: 'chat', query: 'ขอข้อมูลระบบหน่อย' } },
    { label: 'Weather', desc: 'เช็คสภาพอากาศ', icon: '🌤️', action: { type: 'chat', query: 'อากาศที่กรุงเทพเป็นยังไง' } },
    { label: 'Open Spotify', desc: 'เปิดโปรแกรม Spotify', icon: '🎵', action: { type: 'open_app', target: 'spotify' } },
    { label: 'Open VS Code', desc: 'เปิดโปรแกรม VS Code', icon: '💻', action: { type: 'open_app', target: 'code' } },
]

function getCommandIcon(iconName: string) {
  const props = { width: 16, height: 16, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 2.5, strokeLinecap: "round" as const, strokeLinejoin: "round" as const, style: { display: 'inline-block', verticalAlign: 'middle' } }
  switch (iconName) {
    case '📺':
      return (
        <svg {...props}>
          <path d="M22.54 6.42a2.78 2.78 0 0 0-1.94-2C18.88 4 12 4 12 4s-6.88 0-8.6.46a2.78 2.78 0 0 0-1.94 2A29 29 0 0 0 1 11.75a29 29 0 0 0 .46 5.33A2.78 2.78 0 0 0 3.4 19c1.72.46 8.6.46 8.6.46s6.88 0 8.6-.46a2.78 2.78 0 0 0 1.94-2 29 29 0 0 0 .46-5.25 29 29 0 0 0-.46-5.33z"></path>
          <polygon points="9.75 15.02 15.5 11.75 9.75 8.48 9.75 15.02"></polygon>
        </svg>
      )
    case '📘':
      return (
        <svg {...props}>
          <path d="M18 2h-3a5 5 0 0 0-5 5v3H7v4h3v8h4v-8h3l1-4h-4V7a1 1 0 0 1 1-1h3z"></path>
        </svg>
      )
    case '📸':
      return (
        <svg {...props}>
          <rect x="2" y="2" width="20" height="20" rx="5" ry="5"></rect>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="17.5" y1="6.5" x2="17.51" y2="6.5"></line>
        </svg>
      )
    case '🐙':
      return (
        <svg {...props}>
          <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"></path>
        </svg>
      )
    case '💻':
      return (
        <svg {...props}>
          <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
          <line x1="8" y1="21" x2="16" y2="21"></line>
          <line x1="12" y1="17" x2="12" y2="21"></line>
        </svg>
      )
    case '🌤️':
      return (
        <svg {...props}>
          <path d="M12 2v2"></path>
          <path d="M12 20v2"></path>
          <path d="M4.93 4.93l1.41 1.41"></path>
          <path d="M17.66 17.66l1.41 1.41"></path>
          <path d="M2 12h2"></path>
          <path d="M20 12h2"></path>
          <path d="M6.34 17.66l-1.41 1.41"></path>
          <path d="M19.07 4.93l-1.41 1.41"></path>
          <circle cx="12" cy="12" r="4"></circle>
        </svg>
      )
    case '🎵':
      return (
        <svg {...props}>
          <path d="M9 18V5l12-2v13"></path>
          <circle cx="6" cy="18" r="3"></circle>
          <circle cx="18" cy="16" r="3"></circle>
        </svg>
      )
    case '🧮':
      return (
        <svg {...props}>
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
          <line x1="9" y1="9" x2="15" y2="15"></line>
          <line x1="15" y1="9" x2="9" y2="15"></line>
        </svg>
      )
    case '✨':
      return (
        <svg {...props}>
          <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z"></path>
        </svg>
      )
    default:
      return <span>{iconName}</span>
  }
}

export default function SpotlightWindow() {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<any[]>([])
  const [selectedIndex, setSelectedIndex] = useState(-1)
  const inputRef = useRef<HTMLInputElement>(null)

  // Sync theme
  useEffect(() => {
    const applyThemeVariables = (config: any) => {
      if (config.systemTextColor) {
        document.documentElement.style.setProperty('--text-main', config.systemTextColor)
      }
      document.documentElement.setAttribute('data-theme', config.theme || 'dark')
      if (config.theme === 'custom') {
        if (config.customBgStart && config.customBgEnd) {
          const gradient = `linear-gradient(135deg, ${config.customBgStart} 0%, ${config.customBgEnd} 100%)`
          document.documentElement.style.setProperty('--bg-gradient', gradient)
        }
        if (config.customPanelBg) {
          const rgb = hexToRgb(config.customPanelBg)
          document.documentElement.style.setProperty('--panel-bg', `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.75)`)
        }
      } else {
        document.documentElement.style.removeProperty('--bg-gradient')
        document.documentElement.style.removeProperty('--panel-bg')
      }
    }

    if (window.spotlightAPI) {
      window.spotlightAPI.getSettings().then(applyThemeVariables)
      window.spotlightAPI.onSettingsChanged(applyThemeVariables)
    }

    const handleFocus = () => {
      inputRef.current?.focus()
      inputRef.current?.select()
    }
    window.addEventListener('focus', handleFocus)
    handleFocus()

    return () => window.removeEventListener('focus', handleFocus)
  }, [])

  const hexToRgb = (hex: string) => {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
    return result ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16)
    } : { r: 15, g: 23, b: 42 }
  }

  // Calculate matches when query text changes
  useEffect(() => {
    const trimmed = query.trim()
    if (!trimmed) {
      setResults([])
      setSelectedIndex(-1)
      window.spotlightAPI?.resize(600, 80)
      return
    }

    // Try math evaluation
    if (/^[0-9+\-*/().\s]+$/.test(trimmed) && /[0-9]/.test(trimmed)) {
      try {
        const mathVal = evaluateArithmetic(trimmed)
        const mathResult = [{
          label: `Result: ${mathVal}`,
          desc: 'Calculation result (Press Enter to copy)',
          icon: '🧮',
          action: { type: 'clipboard_write', target: mathVal.toString() }
        }]
        setResults(mathResult)
        setSelectedIndex(0)
        window.spotlightAPI?.resize(600, 80 + 64 + 16)
        return
      } catch {}
    }

    // Normal commands
    const matches = COMMANDS.filter(c => 
      c.label.toLowerCase().includes(trimmed.toLowerCase()) || 
      c.desc.toLowerCase().includes(trimmed.toLowerCase())
    )

    // Add default chat fallback
    matches.push({
      label: `Ask Mint: "${trimmed}"`,
      desc: 'Send query to AI Chat',
      icon: '✨',
      action: { type: 'chat', query: trimmed }
    })

    setResults(matches)
    setSelectedIndex(0)
    const newHeight = Math.min(80 + (matches.length * 64) + 16, 500)
    window.spotlightAPI?.resize(600, newHeight)
  }, [query])

  const handleAction = async (action: any) => {
    if (action.type === 'chat') {
      window.spotlightAPI?.submit(action.query)
      return
    }

    if (window.spotlightAPI?.executeAction) {
      const result = await window.spotlightAPI.executeAction(action)
      if (!result || result.success === false) {
        window.spotlightAPI.submit(`Spotlight action failed: ${result?.message || 'Unknown error'}`)
      }
      return
    }

    window.spotlightAPI?.submit(action.target || action.value || '')
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      window.spotlightAPI?.hide()
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex(prev => Math.min(prev + 1, results.length - 1))
    }

    if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex(prev => Math.max(prev - 1, -1))
    }

    if (e.key === 'Enter') {
      if (selectedIndex >= 0 && results[selectedIndex]) {
        handleAction(results[selectedIndex].action)
      } else {
        const trimmed = query.trim()
        if (trimmed) {
          window.spotlightAPI?.submit(trimmed)
        }
      }
    }
  }

  return (
    <div className="spotlight-container">
      <div className="input-wrapper">
        <div className="spotlight-icon" style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z"></path>
          </svg>
        </div>
        <input 
          type="text" 
          id="spotlight-input" 
          placeholder="What can I help with?" 
          autoComplete="off"
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          autoFocus
        />
        <div className="shortcut-tip">ESC to close</div>
      </div>
      {results.length > 0 && (
        <div id="spotlight-results" className="results-container">
          {results.map((cmd, i) => (
            <div 
              key={i}
              className={`result-item ${i === selectedIndex ? 'selected' : ''}`}
              onClick={() => handleAction(cmd.action)}
              onMouseEnter={() => setSelectedIndex(i)}
            >
              <div className="result-icon" style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
                {getCommandIcon(cmd.icon)}
              </div>
              <div className="result-content">
                <div className="result-title">{cmd.label}</div>
                <div className="result-desc">{cmd.desc}</div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
