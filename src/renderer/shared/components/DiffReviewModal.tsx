import React, { useState, useEffect, useMemo } from 'react'
import '../css/management-views.css'

export interface DiffReviewModalProps {
  isOpen: boolean
  onClose: () => void
  filePath: string
  diffText: string
  onApprove: () => void
  onReject: () => void
  isDangerous?: boolean
}

interface SplitDiffRow {
  type: 'hunk' | 'line'
  hunkHeader?: string
  left?: {
    lineNum: number
    content: string
    isDel: boolean
  }
  right?: {
    lineNum: number
    content: string
    isAdd: boolean
  }
}

interface UnifiedDiffLine {
  type: 'hunk' | 'meta' | 'add' | 'del' | 'normal'
  oldNum?: number
  newNum?: number
  content: string
}

function parseHunkHeader(line: string): { oldStart: number; newStart: number } | null {
  const trimmed = line.trim()
  if (!trimmed.startsWith('@@')) return null
  const parts = trimmed.split('@@')
  if (parts.length < 3) return null
  const headerBody = parts[1].trim()
  let oldStart = 1
  let newStart = 1

  for (const token of headerBody.split(/\s+/)) {
    if (token.startsWith('-')) {
      const numStr = token.slice(1).split(',')[0]
      const parsed = parseInt(numStr, 10)
      if (!isNaN(parsed)) oldStart = parsed
    } else if (token.startsWith('+')) {
      const numStr = token.slice(1).split(',')[0]
      const parsed = parseInt(numStr, 10)
      if (!isNaN(parsed)) newStart = parsed
    }
  }
  return { oldStart, newStart }
}

function parseDiff(diffText: string) {
  const rawLines = diffText.split('\n')
  const unifiedLines: UnifiedDiffLine[] = []
  const splitRows: SplitDiffRow[] = []

  let additions = 0
  let deletions = 0

  let curOld = 1
  let curNew = 1

  let pendingDeletes: { lineNum: number; content: string }[] = []
  let pendingAdds: { lineNum: number; content: string }[] = []

  const flushPending = () => {
    const maxLen = Math.max(pendingDeletes.length, pendingAdds.length)
    for (let i = 0; i < maxLen; i++) {
      const leftItem = pendingDeletes[i]
      const rightItem = pendingAdds[i]
      splitRows.push({
        type: 'line',
        left: leftItem ? { lineNum: leftItem.lineNum, content: leftItem.content, isDel: true } : undefined,
        right: rightItem ? { lineNum: rightItem.lineNum, content: rightItem.content, isAdd: true } : undefined,
      })
    }
    pendingDeletes = []
    pendingAdds = []
  }

  for (const raw of rawLines) {
    if (raw.startsWith('@@')) {
      flushPending()
      const hunk = parseHunkHeader(raw)
      if (hunk) {
        curOld = hunk.oldStart
        curNew = hunk.newStart
      }
      unifiedLines.push({ type: 'hunk', content: raw })
      splitRows.push({ type: 'hunk', hunkHeader: raw })
      continue
    }

    if (raw.startsWith('---') || raw.startsWith('+++')) {
      flushPending()
      unifiedLines.push({ type: 'meta', content: raw })
      continue
    }

    if (raw.startsWith('+')) {
      additions++
      const content = raw.slice(1)
      unifiedLines.push({
        type: 'add',
        newNum: curNew,
        content,
      })
      pendingAdds.push({ lineNum: curNew, content })
      curNew++
    } else if (raw.startsWith('-')) {
      deletions++
      const content = raw.slice(1)
      unifiedLines.push({
        type: 'del',
        oldNum: curOld,
        content,
      })
      pendingDeletes.push({ lineNum: curOld, content })
      curOld++
    } else {
      flushPending()
      const content = raw.startsWith(' ') ? raw.slice(1) : raw
      unifiedLines.push({
        type: 'normal',
        oldNum: curOld,
        newNum: curNew,
        content,
      })
      splitRows.push({
        type: 'line',
        left: { lineNum: curOld, content, isDel: false },
        right: { lineNum: curNew, content, isAdd: false },
      })
      curOld++
      curNew++
    }
  }

  flushPending()

  return { unifiedLines, splitRows, additions, deletions }
}

export const DiffReviewModal: React.FC<DiffReviewModalProps> = ({
  isOpen,
  onClose,
  filePath,
  diffText,
  onApprove,
  onReject,
  isDangerous,
}) => {
  const [viewMode, setViewMode] = useState<'split' | 'unified'>('split')
  const [copied, setCopied] = useState(false)

  const { unifiedLines, splitRows, additions, deletions } = useMemo(
    () => parseDiff(diffText || ''),
    [diffText]
  )

  useEffect(() => {
    if (!isOpen) return
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
      } else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
        onApprove()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose, onApprove])

  if (!isOpen) return null

  const handleCopyPath = () => {
    if (navigator?.clipboard?.writeText) {
      navigator.clipboard.writeText(filePath)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  return (
    <div className="management-modal-overlay" onClick={onClose} style={{ zIndex: 10001 }}>
      <div
        className="management-modal diff-review-modal"
        onClick={(e) => e.stopPropagation()}
        style={{
          maxWidth: '92vw',
          width: '1100px',
          maxHeight: '90vh',
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        {/* Header */}
        <div className="management-modal-header" style={{ padding: '16px 20px', gap: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0, flex: 1 }}>
            <div
              className="management-card-icon"
              style={{ width: 34, height: 34, fontSize: '0.72rem', fontWeight: 700, flexShrink: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(56, 189, 248, 0.12)', color: '#38bdf8', borderRadius: '6px' }}
            >
              DIFF
            </div>
            <div style={{ minWidth: 0, flex: 1 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <h3
                  className="management-card-title"
                  style={{ fontSize: '1rem', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}
                  title={filePath}
                >
                  {filePath}
                </h3>
                <button
                  type="button"
                  onClick={handleCopyPath}
                  className="management-action-btn"
                  style={{ padding: '2px 6px', fontSize: '0.72rem' }}
                  title="Copy file path"
                >
                  {copied ? 'Copied' : 'Copy'}
                </button>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 3 }}>
                <span
                  style={{
                    fontSize: '0.75rem',
                    fontWeight: 700,
                    color: '#34d399',
                    background: 'rgba(16, 185, 129, 0.15)',
                    padding: '1px 6px',
                    borderRadius: '4px',
                  }}
                >
                  +{additions}
                </span>
                <span
                  style={{
                    fontSize: '0.75rem',
                    fontWeight: 700,
                    color: '#f87171',
                    background: 'rgba(239, 68, 68, 0.15)',
                    padding: '1px 6px',
                    borderRadius: '4px',
                  }}
                >
                  -{deletions}
                </span>
                {isDangerous && (
                  <span
                    style={{
                      fontSize: '0.72rem',
                      fontWeight: 700,
                      color: '#fbbf24',
                      background: 'rgba(245, 158, 11, 0.15)',
                      padding: '1px 6px',
                      borderRadius: '4px',
                      textTransform: 'uppercase',
                    }}
                  >
                    Review carefully
                  </span>
                )}
              </div>
            </div>
          </div>

          {/* View switcher: Split vs Unified */}
          <div className="management-filter-pills" style={{ display: 'flex', gap: 2, padding: 3, flexShrink: 0 }}>
            <button
              type="button"
              className={`management-pill-btn ${viewMode === 'split' ? 'active' : ''}`}
              onClick={() => setViewMode('split')}
              style={{ padding: '5px 10px', fontSize: '0.78rem' }}
            >
              Side-by-Side
            </button>
            <button
              type="button"
              className={`management-pill-btn ${viewMode === 'unified' ? 'active' : ''}`}
              onClick={() => setViewMode('unified')}
              style={{ padding: '5px 10px', fontSize: '0.78rem' }}
            >
              Unified
            </button>
          </div>

          <button type="button" className="management-modal-close" onClick={onClose} title="Close">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        {/* Diff Content Body */}
        <div
          className="management-modal-body"
          style={{
            padding: 0,
            overflowY: 'auto',
            flex: 1,
            background: '#090d16',
            fontFamily: 'Consolas, Monaco, "JetBrains Mono", monospace',
            fontSize: '0.8rem',
            lineHeight: 1.5,
          }}
        >
          {viewMode === 'split' ? (
            <div className="diff-split-container" style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', minWidth: '700px' }}>
              {/* Left Header */}
              <div
                style={{
                  position: 'sticky',
                  top: 0,
                  zIndex: 2,
                  background: '#0e1526',
                  padding: '6px 12px',
                  borderBottom: '1px solid rgba(255,255,255,0.08)',
                  borderRight: '1px solid rgba(255,255,255,0.08)',
                  color: '#94a3b8',
                  fontWeight: 600,
                  fontSize: '0.75rem',
                }}
              >
                Original (Before)
              </div>
              {/* Right Header */}
              <div
                style={{
                  position: 'sticky',
                  top: 0,
                  zIndex: 2,
                  background: '#0e1526',
                  padding: '6px 12px',
                  borderBottom: '1px solid rgba(255,255,255,0.08)',
                  color: '#94a3b8',
                  fontWeight: 600,
                  fontSize: '0.75rem',
                }}
              >
                Proposed (After)
              </div>

              {splitRows.map((row, idx) => {
                if (row.type === 'hunk') {
                  return (
                    <div
                      key={idx}
                      style={{
                        gridColumn: '1 / -1',
                        background: 'rgba(56, 189, 248, 0.08)',
                        color: '#38bdf8',
                        padding: '4px 12px',
                        fontSize: '0.72rem',
                        fontWeight: 600,
                        borderTop: '1px solid rgba(56, 189, 248, 0.2)',
                        borderBottom: '1px solid rgba(56, 189, 248, 0.2)',
                      }}
                    >
                      {row.hunkHeader}
                    </div>
                  )
                }

                const left = row.left
                const right = row.right

                return (
                  <React.Fragment key={idx}>
                    {/* Left Pane Cell */}
                    <div
                      style={{
                        display: 'flex',
                        borderRight: '1px solid rgba(255,255,255,0.06)',
                        background: left?.isDel ? 'rgba(239, 68, 68, 0.16)' : 'transparent',
                        borderLeft: left?.isDel ? '3px solid #ef4444' : '3px solid transparent',
                        minHeight: '22px',
                      }}
                    >
                      <span
                        style={{
                          width: '44px',
                          textAlign: 'right',
                          paddingRight: '10px',
                          color: left?.isDel ? '#fca5a5' : '#475569',
                          userSelect: 'none',
                          flexShrink: 0,
                        }}
                      >
                        {left ? left.lineNum : ''}
                      </span>
                      <span
                        style={{
                          whiteSpace: 'pre-wrap',
                          wordBreak: 'break-all',
                          paddingRight: '8px',
                          color: left?.isDel ? '#fecaca' : '#cbd5e1',
                        }}
                      >
                        {left ? left.content : ''}
                      </span>
                    </div>

                    {/* Right Pane Cell */}
                    <div
                      style={{
                        display: 'flex',
                        background: right?.isAdd ? 'rgba(16, 185, 129, 0.16)' : 'transparent',
                        borderLeft: right?.isAdd ? '3px solid #10b981' : '3px solid transparent',
                        minHeight: '22px',
                      }}
                    >
                      <span
                        style={{
                          width: '44px',
                          textAlign: 'right',
                          paddingRight: '10px',
                          color: right?.isAdd ? '#86efac' : '#475569',
                          userSelect: 'none',
                          flexShrink: 0,
                        }}
                      >
                        {right ? right.lineNum : ''}
                      </span>
                      <span
                        style={{
                          whiteSpace: 'pre-wrap',
                          wordBreak: 'break-all',
                          paddingRight: '8px',
                          color: right?.isAdd ? '#bbf7d0' : '#cbd5e1',
                        }}
                      >
                        {right ? right.content : ''}
                      </span>
                    </div>
                  </React.Fragment>
                )
              })}
            </div>
          ) : (
            <div style={{ minWidth: '500px' }}>
              {unifiedLines.map((line, idx) => {
                if (line.type === 'hunk') {
                  return (
                    <div
                      key={idx}
                      style={{
                        background: 'rgba(56, 189, 248, 0.08)',
                        color: '#38bdf8',
                        padding: '4px 12px',
                        fontSize: '0.72rem',
                        fontWeight: 600,
                        borderTop: '1px solid rgba(56, 189, 248, 0.2)',
                        borderBottom: '1px solid rgba(56, 189, 248, 0.2)',
                      }}
                    >
                      {line.content}
                    </div>
                  )
                }
                if (line.type === 'meta') {
                  return (
                    <div key={idx} style={{ color: '#64748b', padding: '2px 12px', fontWeight: 600 }}>
                      {line.content}
                    </div>
                  )
                }

                let bg = 'transparent'
                let borderL = '3px solid transparent'
                let textCol = '#cbd5e1'
                let prefix = ' '

                if (line.type === 'add') {
                  bg = 'rgba(16, 185, 129, 0.14)'
                  borderL = '3px solid #10b981'
                  textCol = '#bbf7d0'
                  prefix = '+'
                } else if (line.type === 'del') {
                  bg = 'rgba(239, 68, 68, 0.14)'
                  borderL = '3px solid #ef4444'
                  textCol = '#fecaca'
                  prefix = '-'
                }

                return (
                  <div
                    key={idx}
                    style={{
                      display: 'flex',
                      background: bg,
                      borderLeft: borderL,
                      padding: '1px 0',
                    }}
                  >
                    <span style={{ width: '42px', textAlign: 'right', color: '#475569', paddingRight: '8px', userSelect: 'none', flexShrink: 0 }}>
                      {line.oldNum || ''}
                    </span>
                    <span style={{ width: '42px', textAlign: 'right', color: '#475569', paddingRight: '8px', userSelect: 'none', flexShrink: 0 }}>
                      {line.newNum || ''}
                    </span>
                    <span style={{ width: '16px', color: line.type === 'add' ? '#34d399' : line.type === 'del' ? '#f87171' : '#64748b', userSelect: 'none' }}>
                      {prefix}
                    </span>
                    <span style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-all', color: textCol }}>
                      {line.content}
                    </span>
                  </div>
                )
              })}
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          className="management-modal-footer"
          style={{
            padding: '14px 20px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <div style={{ fontSize: '0.8rem', color: 'var(--text-muted, #94a3b8)' }}>
            <span>Shortcut: </span>
            <kbd style={{ background: 'rgba(255,255,255,0.1)', padding: '2px 5px', borderRadius: 4, fontSize: '0.72rem' }}>Esc</kbd> Cancel
            <span style={{ margin: '0 8px' }}>•</span>
            <kbd style={{ background: 'rgba(255,255,255,0.1)', padding: '2px 5px', borderRadius: 4, fontSize: '0.72rem' }}>Cmd/Ctrl+Enter</kbd> Approve
          </div>

          <div style={{ display: 'flex', gap: 10 }}>
            <button
              type="button"
              className="management-action-btn danger"
              onClick={onReject}
              style={{ padding: '8px 16px', fontSize: '0.88rem' }}
            >
              Reject Change
            </button>
            <button
              type="button"
              className="management-primary-btn"
              onClick={onApprove}
              style={{ padding: '8px 20px', fontSize: '0.88rem' }}
            >
              Approve & Apply
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

export default DiffReviewModal
