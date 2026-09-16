import React, { useState, useEffect } from 'react'
import type { GitCheckpoint, FileChange } from '../types'

export interface RewindModalProps {
  isOpen: boolean
  onClose: () => void
  onConfirm: (targetStep: number) => Promise<void>
  checkpoints: GitCheckpoint[]
  targetCheckpoint: GitCheckpoint | null
  changes: FileChange[]
  workspacePath?: string
  isLoading?: boolean
}

export const RewindModal: React.FC<RewindModalProps> = ({
  isOpen,
  onClose,
  onConfirm,
  checkpoints,
  targetCheckpoint,
  changes,
  isLoading = false,
}) => {
  const [selectedStep, setSelectedStep] = useState<number>(() => {
    return targetCheckpoint?.step ?? (checkpoints.length > 0 ? checkpoints[checkpoints.length - 1].step : 0)
  })

  useEffect(() => {
    if (targetCheckpoint) {
      setSelectedStep(targetCheckpoint.step)
    } else if (checkpoints.length > 0) {
      setSelectedStep(checkpoints[checkpoints.length - 1].step)
    }
  }, [targetCheckpoint, checkpoints])

  useEffect(() => {
    if (!isOpen) return
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !isLoading) {
        onClose()
      } else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !isLoading) {
        onConfirm(selectedStep)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, isLoading, onClose, onConfirm, selectedStep])

  if (!isOpen) return null

  const activeCheckpoint = checkpoints.find((cp) => cp.step === selectedStep) ?? targetCheckpoint

  const handleConfirmClick = () => {
    if (isLoading) return
    onConfirm(selectedStep)
  }

  const formatTimestamp = (ts: number) => {
    if (!ts) return ''
    try {
      const date = new Date(ts * 1000)
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
    } catch {
      return ''
    }
  }

  return (
    <div
      className="management-modal-overlay"
      onClick={isLoading ? undefined : onClose}
      style={{ zIndex: 10002 }}
    >
      <div
        className="management-modal rewind-modal"
        onClick={(e) => e.stopPropagation()}
        style={{
          maxWidth: '560px',
          width: '100%',
          display: 'flex',
          flexDirection: 'column',
          boxShadow: '0 24px 48px rgba(0, 0, 0, 0.65)',
          border: '1px solid color-mix(in srgb, var(--status-error, #ef4444) 30%, var(--border))',
        }}
      >
        {/* Header */}
        <div
          className="management-modal-header"
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '16px 20px',
            borderBottom: '1px solid var(--border)',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <span
              style={{
                width: '32px',
                height: '32px',
                borderRadius: '8px',
                background: 'color-mix(in srgb, var(--status-error, #ef4444) 15%, transparent)',
                color: 'var(--status-error, #ef4444)',
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="1 4 1 10 7 10" />
                <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
              </svg>
            </span>
            <div>
              <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600, color: 'var(--text-main)' }}>
                Rewind Workspace
              </h3>
              <span style={{ fontSize: '12px', color: 'var(--text-muted)' }}>
                Git Checkpoint Time Machine
              </span>
            </div>
          </div>
          <button
            type="button"
            className="management-modal-close"
            onClick={isLoading ? undefined : onClose}
            disabled={isLoading}
            style={{
              background: 'transparent',
              border: 'none',
              color: 'var(--text-muted)',
              cursor: isLoading ? 'not-allowed' : 'pointer',
              padding: '6px',
              borderRadius: '6px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        {/* Body */}
        <div style={{ padding: '20px', display: 'flex', flexDirection: 'column', gap: '16px', overflowY: 'auto', maxHeight: '70vh' }}>
          {/* Warning Banner */}
          <div
            style={{
              padding: '12px 14px',
              borderRadius: '10px',
              background: 'color-mix(in srgb, var(--status-error, #ef4444) 10%, transparent)',
              border: '1px solid color-mix(in srgb, var(--status-error, #ef4444) 25%, transparent)',
              fontSize: '13px',
              lineHeight: 1.5,
              color: 'var(--text-main)',
              display: 'flex',
              gap: '10px',
              alignItems: 'flex-start',
            }}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--status-error, #ef4444)" strokeWidth="2" style={{ flexShrink: 0, marginTop: '2px' }}>
              <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z" />
              <line x1="12" y1="9" x2="12" y2="13" />
              <line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
            <div>
              <strong>Revert files to before this turn?</strong>
              <div style={{ marginTop: '2px', color: 'var(--text-muted)' }}>
                Your workspace will be restored to the state before these edits. A safety rescue snapshot will be preserved automatically so you can undo at any time.
              </div>
            </div>
          </div>

          {/* Checkpoint Target Information */}
          <div
            style={{
              background: 'var(--surface-bg)',
              border: '1px solid var(--border)',
              borderRadius: '10px',
              padding: '14px',
              display: 'flex',
              flexDirection: 'column',
              gap: '10px',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                Target Checkpoint
              </span>
              {checkpoints.length > 1 && (
                <select
                  value={selectedStep}
                  onChange={(e) => setSelectedStep(Number(e.target.value))}
                  disabled={isLoading}
                  style={{
                    background: 'var(--input-bg, #111)',
                    color: 'var(--text-main)',
                    border: '1px solid var(--border)',
                    borderRadius: '6px',
                    fontSize: '12px',
                    padding: '3px 8px',
                    cursor: 'pointer',
                  }}
                >
                  {checkpoints.map((cp) => (
                    <option key={cp.step} value={cp.step}>
                      Step {cp.step} ({cp.commitHash.slice(0, 7)})
                    </option>
                  ))}
                </select>
              )}
            </div>

            {activeCheckpoint ? (
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                <span
                  style={{
                    padding: '3px 8px',
                    borderRadius: '6px',
                    background: 'color-mix(in srgb, var(--accent) 15%, transparent)',
                    color: 'var(--accent)',
                    fontWeight: 600,
                    fontSize: '12px',
                  }}
                >
                  Step {activeCheckpoint.step}
                </span>
                <code
                  style={{
                    padding: '3px 7px',
                    borderRadius: '6px',
                    background: 'var(--input-bg, rgba(0,0,0,0.3))',
                    color: 'var(--text-main)',
                    fontSize: '12px',
                    border: '1px solid var(--border)',
                  }}
                >
                  {activeCheckpoint.commitHash.slice(0, 7)}
                </code>
                {activeCheckpoint.action && (
                  <span style={{ fontSize: '12px', color: 'var(--text-muted)' }}>
                    before {activeCheckpoint.action}
                  </span>
                )}
                {activeCheckpoint.timestamp > 0 && (
                  <span style={{ fontSize: '11px', color: 'var(--text-muted)', marginLeft: 'auto' }}>
                    {formatTimestamp(activeCheckpoint.timestamp)}
                  </span>
                )}
              </div>
            ) : (
              <div style={{ fontSize: '12px', color: 'var(--text-muted)' }}>
                Restoring to initial state before turn execution.
              </div>
            )}
          </div>

          {/* Affected Files List */}
          {changes.length > 0 && (
            <div>
              <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-muted)', marginBottom: '8px', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                Files in this turn ({changes.length})
              </div>
              <div
                style={{
                  maxHeight: '160px',
                  overflowY: 'auto',
                  border: '1px solid var(--border)',
                  borderRadius: '8px',
                  background: 'var(--surface-bg)',
                  display: 'flex',
                  flexDirection: 'column',
                }}
              >
                {changes.map((file, idx) => (
                  <div
                    key={file.path || idx}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      padding: '8px 12px',
                      fontSize: '12.5px',
                      borderBottom: idx < changes.length - 1 ? '1px solid var(--border)' : 'none',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" strokeWidth="2">
                        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                        <polyline points="14 2 14 8 20 8" />
                      </svg>
                      <span
                        title={file.path}
                        style={{
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                          color: 'var(--text-main)',
                          fontFamily: 'monospace',
                        }}
                      >
                        {file.path}
                      </span>
                    </div>

                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexShrink: 0 }}>
                      {file.created ? (
                        <span
                          style={{
                            fontSize: '10.5px',
                            fontWeight: 600,
                            padding: '1px 6px',
                            borderRadius: '4px',
                            background: 'color-mix(in srgb, var(--accent) 15%, transparent)',
                            color: 'var(--accent)',
                          }}
                        >
                          [NEW FILE]
                        </span>
                      ) : (
                        <>
                          {file.additions > 0 && (
                            <span style={{ fontSize: '11px', color: 'var(--accent)', fontWeight: 600 }}>
                              +{file.additions}
                            </span>
                          )}
                          {file.deletions > 0 && (
                            <span style={{ fontSize: '11px', color: 'var(--status-error, #ef4444)', fontWeight: 600 }}>
                              -{file.deletions}
                            </span>
                          )}
                        </>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'flex-end',
            gap: '10px',
            padding: '14px 20px',
            borderTop: '1px solid var(--border)',
            background: 'color-mix(in srgb, var(--surface-bg) 50%, var(--panel-bg))',
          }}
        >
          <button
            type="button"
            onClick={onClose}
            disabled={isLoading}
            style={{
              padding: '8px 16px',
              borderRadius: '8px',
              border: '1px solid var(--border)',
              background: 'transparent',
              color: 'var(--text-main)',
              fontSize: '13px',
              cursor: isLoading ? 'not-allowed' : 'pointer',
              fontWeight: 500,
              transition: 'background 0.15s ease',
            }}
          >
            Cancel
          </button>

          <button
            type="button"
            onClick={handleConfirmClick}
            disabled={isLoading}
            style={{
              padding: '8px 18px',
              borderRadius: '8px',
              border: 'none',
              background: 'var(--status-error, #ef4444)',
              color: '#ffffff',
              fontSize: '13px',
              cursor: isLoading ? 'not-allowed' : 'pointer',
              fontWeight: 600,
              display: 'inline-flex',
              alignItems: 'center',
              gap: '7px',
              boxShadow: '0 2px 8px color-mix(in srgb, var(--status-error, #ef4444) 40%, transparent)',
              opacity: isLoading ? 0.75 : 1,
              transition: 'all 0.15s ease',
            }}
          >
            {isLoading ? (
              <>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" className="spinning-loader">
                  <path d="M21 12a9 9 0 1 1-6.219-8.56" />
                </svg>
                <span>Rewinding...</span>
              </>
            ) : (
              <>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                  <polyline points="1 4 1 10 7 10" />
                  <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
                </svg>
                <span>Confirm Rewind</span>
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  )
}

export default RewindModal
