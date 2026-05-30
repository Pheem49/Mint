import React, { useState, useEffect, useRef } from 'react'

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
        const mathVal = eval(trimmed)
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
        <div className="spotlight-icon">✨</div>
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
              <div className="result-icon">{cmd.icon}</div>
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
