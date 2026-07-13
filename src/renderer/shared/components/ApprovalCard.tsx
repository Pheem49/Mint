/**
 * shared/components/ApprovalCard.tsx
 * Renders the pending tool/action approval card with diffs and input fields.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import React, { useState } from 'react'
import { renderApprovalDetails, renderDiff } from '../utils/approval'

interface Props {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  pendingApproval: any
  onApproval: (approved: boolean, autoApproveSession?: boolean, answer?: string) => void
  key?: any
}


export function ApprovalCard({ pendingApproval, onApproval }: Props) {
  const [askAnswer, setAskAnswer] = useState('')

  const details = renderApprovalDetails(pendingApproval.approval)
  const writeFile = pendingApproval.approval?.WriteFile
  const applyPatch = pendingApproval.approval?.ApplyPatch
  const diffText = writeFile?.diff || applyPatch?.diff
  const isAskUser = !!pendingApproval.approval?.AskUser

  return (
    <div className="message ai-message" style={{ width: '100%' }}>
      <div className="bubble-wrapper" style={{ width: '100%' }}>
        <div className="action-card approval-card" data-tier={details.isDangerous ? 'dangerous' : undefined} style={{ width: '100%' }}>
          <div className="approval-card-content" style={{ width: '100%' }}>
            <div className="approval-card-title">{details.title}</div>
            <div className="approval-card-body" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>{details.body}</div>
            {diffText ? (
              <div className="approval-card-diff-container" style={{ marginTop: '8px', width: '100%' }}>
                <div style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-soft, #94a3b8)', marginBottom: '4px' }}>Diff:</div>
                {renderDiff(diffText)}
              </div>
            ) : (
              details.reason && <div className="approval-card-reason" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>{details.reason}</div>
            )}
            {isAskUser && (
              <div className="approval-card-input-container" style={{ marginTop: '12px', width: '100%' }}>
                <textarea
                  style={{
                    width: '100%',
                    minHeight: '70px',
                    padding: '10px',
                    background: 'rgba(0, 0, 0, 0.2)',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    borderRadius: '6px',
                    color: 'var(--text-main, #f8fafc)',
                    fontSize: '0.9rem',
                    fontFamily: 'inherit',
                    resize: 'vertical',
                    outline: 'none',
                  }}
                  placeholder="Type your answer here..."
                  value={askAnswer}
                  onChange={(e) => setAskAnswer(e.target.value)}
                />
              </div>
            )}
          </div>
          {isAskUser ? (
            <div className="approval-card-actions">
              <button
                type="button"
                className="approval-btn approval-btn-approve"
                disabled={!askAnswer.trim()}
                onClick={() => onApproval(true, false, askAnswer)}
              >
                Submit Answer
              </button>
              <button
                type="button"
                className="approval-btn approval-btn-cancel"
                onClick={() => onApproval(false)}
              >
                Decline
              </button>
            </div>
          ) : (
            <div className="approval-card-actions">
              <button type="button" className="approval-btn approval-btn-approve" onClick={() => onApproval(true)}>Approve</button>
              <button type="button" className="approval-btn" style={{ backgroundColor: 'rgba(16, 185, 129, 0.22)', borderColor: 'rgba(16, 185, 129, 0.4)', color: '#a7f3d0' }} onClick={() => onApproval(true, true)}>Approve this session</button>
              <button type="button" className="approval-btn approval-btn-cancel" onClick={() => onApproval(false)}>Cancel</button>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
