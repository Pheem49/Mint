/**
 * shared/components/ThinkingBlock.tsx
 * Renders the collapsible reasoning thoughts block from the LLM.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import { useEffect, useRef, useState } from 'react'
import { Brain, ChevronRight } from 'lucide-react'
import { renderFormattedMessage } from '../utils/markdown'

const THINKING_LABELS = {
  live: 'Thinking…',
  completed: 'Thinking process',
  extendedLive: 'Thinking…',
  extendedCompleted: 'Thought',
  emptyHint: 'No thoughts recorded',
} as const

interface Props {
  thoughts: string[]
  isLive?: boolean
  blockKey: string
  expanded?: boolean
  onExpandedChange?: (key: string, open: boolean) => void
  showEmptyHint?: boolean
  /** 'default' for short agent thoughts, 'extended' for model reasoning tokens */
  variant?: 'default' | 'extended'
}

export function ThinkingBlock({
  thoughts,
  isLive = false,
  blockKey,
  expanded,
  onExpandedChange,
  showEmptyHint = false,
  variant = 'default',
}: Props) {
  const contentRef = useRef<HTMLDivElement>(null)
  const isControlled = expanded !== undefined && Boolean(onExpandedChange)
  const [localOpen, setLocalOpen] = useState(isLive)
  const isOpen = isControlled ? Boolean(expanded) : localOpen

  const setOpen = (open: boolean) => {
    if (isControlled && onExpandedChange) onExpandedChange(blockKey, open)
    else setLocalOpen(open)
  }

  useEffect(() => {
    if (isLive && !isControlled) setLocalOpen(true)
  }, [isLive, isControlled])

  useEffect(() => {
    if (!isLive || !isOpen || !contentRef.current) return
    contentRef.current.scrollTop = contentRef.current.scrollHeight
  }, [thoughts, isLive, isOpen])

  if (thoughts.length === 0 && !showEmptyHint) return null

  if (thoughts.length === 0) {
    return (
      <div className="thinking-block thinking-block-empty-state" data-live={isLive ? 'true' : 'false'}>
        <span className="thinking-block-empty">{THINKING_LABELS.emptyHint}</span>
      </div>
    )
  }

  const variantClass = variant === 'extended' ? ' is-extended' : ''

  return (
    <div className={`thinking-block${isLive ? ' is-live' : ''}${isOpen ? ' is-expanded' : ''}${variantClass}`}>
      <button
        type="button"
        className="thinking-block-header"
        onClick={() => setOpen(!isOpen)}
        aria-expanded={isOpen}
      >
        <Brain size={14} className="thinking-block-icon" style={{ flexShrink: 0 }} />
        <span className="thinking-block-label">
          {variant === 'extended'
            ? (isLive ? THINKING_LABELS.extendedLive : THINKING_LABELS.extendedCompleted)
            : (isLive ? THINKING_LABELS.live : THINKING_LABELS.completed)
          }
        </span>
        {isLive && <span className="thinking-block-live-dot" aria-hidden="true" />}
        <span className={`thinking-block-chevron${isOpen ? ' is-open' : ''}`} aria-hidden="true">
          <ChevronRight size={13} strokeWidth={2.2} />
        </span>
      </button>
      {isOpen && (
        <div className="thinking-block-body">
          <div className="thinking-block-content" ref={contentRef}>
            {thoughts.map((thought, index) => (
              <div className="thinking-block-step" key={`${blockKey}-${index}`}>
                <div className="thinking-block-step-body">{renderFormattedMessage(thought)}</div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
