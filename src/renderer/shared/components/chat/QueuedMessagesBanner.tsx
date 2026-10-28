import { useState } from 'react'
import type { QueuedPrompt } from '../../types'
import '../../css/queued-messages.css'

export interface QueuedMessagesBannerProps {
  queuedPrompts: QueuedPrompt[]
  queuePaused: boolean
  sending: boolean
  onRemove: (id: string) => void
  onEdit: (id: string) => void
  onClear: () => void
  onSteer: (id: string) => void
  onResumeQueue?: () => void
}

export default function QueuedMessagesBanner({
  queuedPrompts,
  queuePaused,
  sending,
  onRemove,
  onEdit,
  onClear,
  onSteer,
  onResumeQueue,
}: QueuedMessagesBannerProps) {
  const [isCollapsed, setIsCollapsed] = useState(false)

  if (!queuedPrompts || queuedPrompts.length === 0) return null

  return (
    <div className="queued-messages-tray" role="region" aria-label="Queued messages">
      <div className="queued-messages-header">
        <div className="queued-messages-title-wrap">
          <span className="queued-messages-title">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <line x1="8" y1="6" x2="21" y2="6"></line>
              <line x1="8" y1="12" x2="21" y2="12"></line>
              <line x1="8" y1="18" x2="21" y2="18"></line>
              <line x1="3" y1="6" x2="3.01" y2="6"></line>
              <line x1="3" y1="12" x2="3.01" y2="12"></line>
              <line x1="3" y1="18" x2="3.01" y2="18"></line>
            </svg>
            <span>Queued</span>
            <span className="queued-messages-badge">{queuedPrompts.length}</span>
          </span>
          <span className={`queued-messages-status ${queuePaused ? 'is-paused' : ''}`}>
            {queuePaused ? (
              <>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" style={{ display: 'inline', verticalAlign: '-1px', marginRight: 4 }}>
                  <rect x="6" y="4" width="4" height="16" rx="1"></rect>
                  <rect x="14" y="4" width="4" height="16" rx="1"></rect>
                </svg>
                Queue paused · Click arrow to steer or resume
              </>
            ) : sending ? (
              'Sends automatically after current turn completes'
            ) : (
              'Ready to send'
            )}
          </span>
        </div>
        <div className="queued-messages-actions">
          {queuePaused && onResumeQueue && (
            <button
              type="button"
              className="queued-messages-btn btn-resume"
              onClick={onResumeQueue}
              title="Resume queue and send first message"
              aria-label="Resume queue"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <polygon points="5 3 19 12 5 21 5 3"></polygon>
              </svg>
              <span>Resume</span>
            </button>
          )}
          <button
            type="button"
            className="queued-messages-btn"
            onClick={onClear}
            title="Clear all queued messages"
            aria-label="Clear all queued messages"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            </svg>
            <span>Clear</span>
          </button>
          <button
            type="button"
            className="queued-messages-btn"
            onClick={() => setIsCollapsed((prev) => !prev)}
            title={isCollapsed ? 'Expand queue' : 'Collapse queue'}
            aria-label={isCollapsed ? 'Expand queue' : 'Collapse queue'}
          >
            {isCollapsed ? (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <polyline points="18 15 12 9 6 15"></polyline>
              </svg>
            ) : (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <polyline points="6 9 12 15 18 9"></polyline>
              </svg>
            )}
          </button>
        </div>
      </div>
      {!isCollapsed && (
        <div className="queued-messages-list">
          {queuedPrompts.map((prompt, index) => {
            const imageCount = prompt.imageAttachments?.length || 0
            const videoCount = prompt.videoAttachments?.length || 0
            const docName = prompt.documentAttachment?.filename
            return (
              <div key={prompt.id} className="queued-message-item">
                <span className="queued-message-index">#{index + 1}</span>
                <div className="queued-message-content">
                  <div className="queued-message-text" title={prompt.text}>
                    {prompt.text || '(No text prompt)'}
                  </div>
                  {(imageCount > 0 || videoCount > 0 || docName) && (
                    <div className="queued-message-attachments">
                      {imageCount > 0 && (
                        <span className="queued-attachment-pill">
                          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                            <circle cx="8.5" cy="8.5" r="1.5"></circle>
                            <polyline points="21 15 16 10 5 21"></polyline>
                          </svg>
                          {imageCount} {imageCount === 1 ? 'image' : 'images'}
                        </span>
                      )}
                      {videoCount > 0 && (
                        <span className="queued-attachment-pill">
                          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                            <polygon points="23 7 16 12 23 17 23 7"></polygon>
                            <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
                          </svg>
                          {videoCount} {videoCount === 1 ? 'video' : 'videos'}
                        </span>
                      )}
                      {docName && (
                        <span className="queued-attachment-pill" title={docName}>
                          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                            <polyline points="14 2 14 8 20 8"></polyline>
                          </svg>
                          {docName}
                        </span>
                      )}
                    </div>
                  )}
                </div>
                <div className="queued-item-actions">
                  <button
                    type="button"
                    className="queued-icon-btn btn-steer"
                    onClick={() => onSteer(prompt.id)}
                    title="Send now (interrupt current turn and steer agent)"
                    aria-label="Send now"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <line x1="5" y1="12" x2="19" y2="12"></line>
                      <polyline points="12 5 19 12 12 19"></polyline>
                    </svg>
                  </button>
                  <button
                    type="button"
                    className="queued-icon-btn"
                    onClick={() => onEdit(prompt.id)}
                    title="Edit (restore to composer)"
                    aria-label="Edit message"
                  >
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z"></path>
                    </svg>
                  </button>
                  <button
                    type="button"
                    className="queued-icon-btn btn-delete"
                    onClick={() => onRemove(prompt.id)}
                    title="Remove from queue"
                    aria-label="Remove from queue"
                  >
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <polyline points="3 6 5 6 21 6"></polyline>
                      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                      <line x1="10" y1="11" x2="10" y2="17"></line>
                      <line x1="14" y1="11" x2="14" y2="17"></line>
                    </svg>
                  </button>
                </div>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}
