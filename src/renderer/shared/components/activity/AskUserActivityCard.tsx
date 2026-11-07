/**
 * shared/components/activity/AskUserActivityCard.tsx
 * Sleek questions & answers card rendered in the agent activity timeline after
 * the user has answered an ask_user prompt.
 * Shared across Desktop and Web — do NOT duplicate.
 */
import React, { memo } from 'react'
import type { AskUserItem } from '../../utils/agentActivity'
import { renderFormattedMessage } from '../../utils/markdown'

export interface AskUserActivityCardProps {
  questions: AskUserItem[]
  header?: string
  className?: string
}

export const AskUserActivityCard = memo(function AskUserActivityCard({
  questions,
  header,
  className,
}: AskUserActivityCardProps) {
  if (!questions || questions.length === 0) return null

  const count = questions.length
  const countLabel = count === 1 ? '1 question' : `${count} questions`
  const headerText = header ? `${countLabel} · ${header}` : countLabel

  return (
    <div
      className={`modern-ask-user-card ${className || ''}`.trim()}
      data-testid="ask-user-activity-card"
    >
      <div className="modern-ask-user-header">
        <span className="modern-ask-user-header-icon" aria-hidden="true">
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="modern-ask-user-icon"
          >
            <rect x="3" y="3" width="18" height="18" rx="4" />
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
            <line x1="12" y1="17" x2="12.01" y2="17" strokeWidth="2.5" />
          </svg>
        </span>
        <span className="modern-ask-user-header-text">{headerText}</span>
      </div>

      <div className="modern-ask-user-items">
        {questions.map((item, index) => (
          <div key={index} className="modern-ask-user-item">
            <div className="modern-ask-user-question">
              {renderFormattedMessage(item.question) ?? item.question}
            </div>
            {item.answer && (
              <div className="modern-ask-user-answer">
                {renderFormattedMessage(item.answer) ?? item.answer}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  )
})
