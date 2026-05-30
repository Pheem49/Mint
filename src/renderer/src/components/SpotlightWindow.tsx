import React, { useEffect, useRef, useState } from 'react'
import '../styles/spotlight.css'

interface Command {
    label: string
    desc: string
    icon: string
    action: any
}

const COMMANDS: Command[] = [
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
    const [results, setResults] = useState<Command[]>([])
    const [selectedIndex, setSelectedIndex] = useState(-1)
    const inputRef = useRef<HTMLInputElement>(null)

    useEffect(() => {
        // Auto-focus on show
        inputRef.current?.focus()
        
        const handleGlobalFocus = () => {
            inputRef.current?.focus()
            inputRef.current?.select()
        }
        window.addEventListener('focus', handleGlobalFocus)

        // Load settings and theme variables
        if (window.spotlightAPI) {
            window.spotlightAPI.getSettings().then(applyTheme)
            window.spotlightAPI.onSettingsChanged(applyTheme)
        }

        return () => {
            window.removeEventListener('focus', handleGlobalFocus)
        }
    }, [])

    const applyTheme = (config: any) => {
        document.documentElement.setAttribute('data-theme', config.theme || 'dark')
        if (config.systemTextColor) {
            document.documentElement.style.setProperty('--text-main', config.systemTextColor)
        }
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

    const hexToRgb = (hex: string) => {
        const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
        return result ? {
            r: parseInt(result[1], 16),
            g: parseInt(result[2], 16),
            b: parseInt(result[3], 16)
        } : { r: 15, g: 23, b: 42 }
    }

    const handleQueryChange = (val: string) => {
        setQuery(val)
        const trimmed = val.toLowerCase().trim()
        if (!trimmed) {
            updateResults([])
            return
        }

        // Calculation helper
        if (/^[0-9+\-*/().\s]+$/.test(trimmed) && /[0-9]/.test(trimmed)) {
            try {
                // eslint-disable-next-line no-eval
                const calcResult = eval(trimmed)
                updateResults([{
                    label: `Result: ${calcResult}`,
                    desc: 'Calculation result (Press Enter to copy)',
                    icon: '🧮',
                    action: { type: 'clipboard_write', target: calcResult.toString() }
                }])
                return
            } catch {
                // Ignore calculation errors
            }
        }

        const matches = COMMANDS.filter(c =>
            c.label.toLowerCase().includes(trimmed) ||
            c.desc.toLowerCase().includes(trimmed)
        )

        // Add a default "Ask Gemini" option
        matches.push({
            label: `Ask Mint: "${val}"`,
            desc: 'Send query to AI Chat',
            icon: '✨',
            action: { type: 'chat', query: val }
        })

        updateResults(matches)
    }

    const updateResults = (newResults: Command[]) => {
        setResults(newResults)
        if (newResults.length === 0) {
            setSelectedIndex(-1)
            window.spotlightAPI?.resize(600, 80)
            return
        }

        // Calculate height
        const newHeight = Math.min(80 + (newResults.length * 64) + 16, 500)
        window.spotlightAPI?.resize(600, newHeight)
    }

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
            const nextIdx = Math.min(selectedIndex + 1, results.length - 1)
            setSelectedIndex(nextIdx)
        }

        if (e.key === 'ArrowUp') {
            e.preventDefault()
            const prevIdx = Math.max(selectedIndex - 1, -1)
            setSelectedIndex(prevIdx)
        }

        if (e.key === 'Enter') {
            if (selectedIndex >= 0 && results[selectedIndex]) {
                handleAction(results[selectedIndex].action)
            } else {
                const text = query.trim()
                if (text) {
                    window.spotlightAPI?.submit(text)
                }
            }
        }
    }

    return (
        <div id="spotlight-bar" className="drag-region">
            <div className="input-container">
                <span className="search-icon">🔍</span>
                <input
                    ref={inputRef}
                    type="text"
                    id="spotlight-input"
                    placeholder="Search actions, run commands, calculate..."
                    value={query}
                    onChange={(e) => handleQueryChange(e.target.value)}
                    onKeyDown={handleKeyDown}
                    autoComplete="off"
                />
            </div>
            {results.length > 0 && (
                <div id="spotlight-results" style={{ display: 'block' }}>
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
