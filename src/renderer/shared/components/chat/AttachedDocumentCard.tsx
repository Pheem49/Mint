import React, { memo, useState } from 'react'
import { materialFileIcon, getExtension } from '../../utils/fileIcons'
import type { AttachedDocumentPayload } from '../../utils/attachedDocument'

export interface AttachedDocumentCardProps {
  document: AttachedDocumentPayload
  defaultOpen?: boolean
}

export const AttachedDocumentCard = memo(function AttachedDocumentCard({
  document,
  defaultOpen = false,
}: AttachedDocumentCardProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen)
  const [isCopied, setIsCopied] = useState(false)

  const extension = getExtension(document.filename)
  const iconUrl = materialFileIcon(document.filename, extension)

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (!document.content) return
    navigator.clipboard.writeText(document.content).then(() => {
      setIsCopied(true)
      setTimeout(() => setIsCopied(false), 1500)
    }).catch(() => {})
  }

  const lineLabel = `${document.lineCount} ${document.lineCount === 1 ? 'line' : 'lines'}`

  return (
    <div
      className="chat-attached-document-card"
      style={{
        marginTop: '8px',
        borderRadius: 'var(--radius-md, 8px)',
        border: '1px solid color-mix(in srgb, var(--border, rgba(255, 255, 255, 0.15)) 80%, transparent)',
        background: 'color-mix(in srgb, var(--surface-bg, #18181b) 75%, rgba(0, 0, 0, 0.25))',
        overflow: 'hidden',
        boxSizing: 'border-box',
        width: '100%',
        maxWidth: '100%',
        transition: 'border-color 0.15s ease',
      }}
    >
      <div
        className="chat-attached-document-header"
        onClick={() => setIsOpen((prev) => !prev)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            setIsOpen((prev) => !prev)
          }
        }}
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '7px 12px',
          cursor: 'pointer',
          userSelect: 'none',
          gap: '10px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0, flex: 1 }}>
          {iconUrl ? (
            <img
              src={iconUrl}
              alt=""
              width="14"
              height="14"
              style={{ width: 14, height: 14, flexShrink: 0, objectFit: 'contain' }}
              aria-hidden="true"
              draggable={false}
            />
          ) : (
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              style={{ color: 'var(--text-muted, #a1a1aa)', flexShrink: 0 }}
              aria-hidden="true"
            >
              <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
              <polyline points="14 2 14 8 20 8" />
            </svg>
          )}

          <span
            className="chat-attached-document-filename"
            title={document.filename}
            style={{
              fontWeight: 600,
              fontSize: '0.82rem',
              color: 'var(--text-main, #f4f4f5)',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {document.filename}
          </span>

          <span
            className="chat-attached-document-meta"
            style={{
              fontSize: '0.72rem',
              color: 'var(--text-muted, #a1a1aa)',
              flexShrink: 0,
            }}
          >
            • {lineLabel}
          </span>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexShrink: 0 }}>
          <button
            type="button"
            className="chat-attached-document-copy-btn"
            onClick={handleCopy}
            title={isCopied ? 'Copied!' : 'Copy document content'}
            aria-label={isCopied ? 'Copied' : 'Copy document'}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              width: '24px',
              height: '24px',
              padding: 0,
              border: 0,
              borderRadius: 'var(--radius-xs, 4px)',
              background: 'transparent',
              color: isCopied ? 'var(--status-success, #22c55e)' : 'var(--text-muted, #a1a1aa)',
              cursor: 'pointer',
              transition: 'color 0.15s ease',
            }}
          >
            {isCopied ? (
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            ) : (
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
            )}
          </button>

          <span
            className="chat-attached-document-toggle-action"
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: '4px',
              fontSize: '0.74rem',
              fontWeight: 500,
              color: 'var(--interactive-fg-hover, #60a5fa)',
            }}
          >
            <span>{isOpen ? 'Hide content' : 'View content'}</span>
            <svg
              width="11"
              height="11"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              style={{
                transform: isOpen ? 'rotate(180deg)' : 'none',
                transition: 'transform 0.18s ease',
              }}
              aria-hidden="true"
            >
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </span>
        </div>
      </div>

      {isOpen && (
        <div
          className="chat-attached-document-body"
          style={{
            borderTop: '1px solid color-mix(in srgb, var(--border, rgba(255, 255, 255, 0.1)) 60%, transparent)',
            background: 'rgba(0, 0, 0, 0.22)',
            padding: '8px 12px',
            maxHeight: '260px',
            overflowY: 'auto',
          }}
        >
          <pre
            style={{
              margin: 0,
              fontFamily: 'var(--font-mono, monospace)',
              fontSize: '0.78rem',
              lineHeight: 1.45,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
              color: 'var(--text-main, #e4e4e7)',
            }}
          >
            {document.content}
          </pre>
        </div>
      )}
    </div>
  )
})

export default AttachedDocumentCard
