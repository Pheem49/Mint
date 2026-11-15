import React, { useState, useRef, useEffect, useId } from 'react'
import './ChatModeSelector.css'

export interface ChatModeSelectorProps {
  agentMode: boolean
  onSetAgentMode: (enabled: boolean) => void
  planMode?: boolean
  onSetPlanMode?: (enabled: boolean) => void
  disabled?: boolean
}

export type ChatModeId = 'agent' | 'plan' | 'chat'

interface ModeOption {
  id: ChatModeId
  label: string
  shortLabel: string
  title: string
  desc: string
  icon: React.ReactNode
}

export default function ChatModeSelector({
  agentMode,
  onSetAgentMode,
  planMode,
  onSetPlanMode,
  disabled = false,
}: ChatModeSelectorProps) {
  const [isOpen, setIsOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const menuId = useId()

  const currentMode: ChatModeId = planMode
    ? 'plan'
    : agentMode
    ? 'agent'
    : 'chat'

  const options: ModeOption[] = [
    {
      id: 'agent',
      label: 'Agent mode',
      shortLabel: 'Agent',
      title: 'Agent mode: autonomous execution with tool use',
      desc: 'Runs tools and edits files automatically',
      icon: (
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" />
        </svg>
      ),
    },
    ...(onSetPlanMode ? [{
      id: 'plan' as ChatModeId,
      label: 'Plan mode',
      shortLabel: 'Plan',
      title: 'Plan mode: investigate read-only and require plan approval before editing',
      desc: 'Researches read-only and requires approval for edits',
      icon: (
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M9 5H7a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-2" />
          <rect x="9" y="3" width="6" height="4" rx="2" />
          <path d="M9 12h6m-6 4h4" />
        </svg>
      ),
    }] : []),
    {
      id: 'chat',
      label: 'Chat mode',
      shortLabel: 'Chat',
      title: 'Chat mode: standard conversation without tools',
      desc: 'Direct conversation without autonomous changes',
      icon: (
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
        </svg>
      ),
    },
  ]

  const activeOption = options.find((opt) => opt.id === currentMode) || options[0]

  const handleSelect = (mode: ChatModeId) => {
    if (disabled) return
    if (mode === 'agent') {
      onSetAgentMode(true)
      onSetPlanMode?.(false)
    } else if (mode === 'plan') {
      onSetAgentMode(true)
      onSetPlanMode?.(true)
    } else {
      onSetAgentMode(false)
      onSetPlanMode?.(false)
    }
    setIsOpen(false)
  }

  // Close when clicking outside
  useEffect(() => {
    if (!isOpen) return
    const onMouseDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setIsOpen(false)
      }
    }
    window.addEventListener('mousedown', onMouseDown)
    return () => window.removeEventListener('mousedown', onMouseDown)
  }, [isOpen])

  // Close on Escape
  useEffect(() => {
    if (!isOpen) return
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsOpen(false)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [isOpen])

  return (
    <div className="chat-mode-selector-wrap" ref={rootRef}>
      <button
        type="button"
        id="chat-mode-btn"
        className={`chat-mode-trigger is-${currentMode} ${isOpen ? 'is-active' : ''}`}
        onClick={() => setIsOpen((prev) => !prev)}
        disabled={disabled}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-controls={menuId}
        title={activeOption.title}
      >
        <span className="chat-mode-icon" aria-hidden="true">
          {activeOption.icon}
        </span>
        <span className="chat-mode-label">{activeOption.shortLabel}</span>
        <svg
          className={`chat-mode-chevron ${isOpen ? 'is-open' : ''}`}
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>

      {isOpen && (
        <div id={menuId} className="chat-mode-popover" role="menu">
          <div className="chat-mode-popover-header">Interaction Mode</div>
          {options.map((opt) => {
            const isSelected = opt.id === currentMode
            return (
              <button
                key={opt.id}
                type="button"
                role="menuitem"
                className={`chat-mode-option is-${opt.id} ${isSelected ? 'is-selected' : ''}`}
                onClick={() => handleSelect(opt.id)}
              >
                <span className="chat-mode-option-icon" aria-hidden="true">
                  {opt.icon}
                </span>
                <div className="chat-mode-option-text">
                  <span className="chat-mode-option-title">{opt.label}</span>
                  <span className="chat-mode-option-desc">{opt.desc}</span>
                </div>
                {isSelected && (
                  <svg
                    className="chat-mode-option-check"
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    aria-hidden="true"
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                )}
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}
