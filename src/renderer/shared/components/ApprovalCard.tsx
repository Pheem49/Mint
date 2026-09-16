/**
 * shared/components/ApprovalCard.tsx
 * Claude Desktop styled interactive Choice Cards for tool & action approvals.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import React, { useState, useEffect } from 'react'
import { renderApprovalDetails, renderDiff } from '../utils/approval'
import { DiffReviewModal } from './DiffReviewModal'

interface Props {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  pendingApproval: any
  onApproval: (approved: boolean, autoApproveSession?: boolean, answer?: string) => void
  key?: any
}

interface AskUserOption {
  label: string
  description?: string
}

export function ApprovalCard({ pendingApproval, onApproval }: Props) {
  const [askAnswer, setAskAnswer] = useState('')
  const [selectedOptions, setSelectedOptions] = useState<Set<string>>(new Set())
  const [showDiffModal, setShowDiffModal] = useState(false)

  const details = renderApprovalDetails(pendingApproval.approval)
  const writeFile = pendingApproval.approval?.WriteFile
  const applyPatch = pendingApproval.approval?.ApplyPatch
  const runShell = pendingApproval.approval?.RunShell
  const targetFilePath = writeFile?.path || applyPatch?.path || 'Code Change'
  const diffText = writeFile?.diff || applyPatch?.diff
  const isAskUser = !!pendingApproval.approval?.AskUser
  const mcpTool = pendingApproval.approval?.McpTool
  const MCP_ALLOW_ALL_SENTINEL = '__mcp_allow_all__'
  const askOptions: AskUserOption[] = Array.isArray(pendingApproval.approval?.AskUser?.options)
    ? pendingApproval.approval.AskUser.options
    : []
  const isMultiSelect = !!pendingApproval.approval?.AskUser?.multiSelect
  const askHeader: string | undefined = pendingApproval.approval?.AskUser?.header || undefined

  const buildMultiSelectAnswer = () => {
    const joined = Array.from(selectedOptions).join(', ')
    const freeText = askAnswer.trim()
    if (!freeText) return joined
    return joined ? `${joined} — ${freeText}` : freeText
  }

  const toggleOption = (label: string) => {
    setSelectedOptions((prev) => {
      const next = new Set(prev)
      if (next.has(label)) next.delete(label)
      else next.add(label)
      return next
    })
  }

  // Get badge label based on action type
  const getToolMeta = () => {
    if (runShell) return { badge: 'Run Command', badgeClass: details.isDangerous ? 'dangerous' : 'command' }
    if (writeFile || applyPatch) return { badge: 'File Edit', badgeClass: 'edit' }
    if (mcpTool) return { badge: `MCP: ${mcpTool.server}`, badgeClass: 'mcp' }
    if (isAskUser) return { badge: askHeader || 'Question', badgeClass: 'question' }
    return { badge: 'Action Approval', badgeClass: details.isDangerous ? 'dangerous' : 'default' }
  }

  const toolMeta = getToolMeta()

  // Standard non-AskUser choices
  const actionChoices = mcpTool
    ? [
        {
          key: '1',
          title: 'Allow once',
          variant: 'approve',
          action: () => onApproval(true, false),
        },
        {
          key: '2',
          title: `Always allow for "${mcpTool.server}"`,
          variant: 'session',
          action: () => onApproval(true, false, MCP_ALLOW_ALL_SENTINEL),
        },
        {
          key: '3',
          title: 'Reject',
          variant: 'cancel',
          action: () => onApproval(false),
        },
      ]
    : [
        {
          key: '1',
          title: 'Allow once',
          variant: 'approve',
          action: () => onApproval(true, false),
        },
        {
          key: '2',
          title: 'Always allow for this session',
          variant: 'session',
          action: () => onApproval(true, true),
        },
        {
          key: '3',
          title: 'Reject',
          variant: 'cancel',
          action: () => onApproval(false),
        },
      ]

  // Keyboard shortcut listener for numbers (1, 2, 3...) when not inside an input
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const activeEl = document.activeElement as HTMLElement | null
      if (
        activeEl &&
        (activeEl.tagName === 'INPUT' || activeEl.tagName === 'TEXTAREA' || activeEl.isContentEditable)
      ) {
        return
      }

      if (!isAskUser) {
        const num = parseInt(e.key, 10)
        if (!isNaN(num) && num >= 1 && num <= actionChoices.length) {
          e.preventDefault()
          actionChoices[num - 1].action()
        }
      } else if (!isMultiSelect && askOptions.length > 0) {
        const num = parseInt(e.key, 10)
        if (!isNaN(num) && num >= 1 && num <= askOptions.length) {
          e.preventDefault()
          onApproval(true, false, askOptions[num - 1].label)
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isAskUser, isMultiSelect, askOptions, actionChoices])

  return (
    <div className="message ai-message" style={{ width: '100%' }}>
      <div className="bubble-wrapper" style={{ width: '100%' }}>
        <div
          className="action-card approval-card"
          data-tier={details.isDangerous ? 'dangerous' : undefined}
          style={{ width: '100%', alignItems: 'stretch', textAlign: 'left' }}
        >
          {/* Card Header */}
          <div className="approval-card-header" style={{ width: '100%', textAlign: 'left' }}>
            <div className="approval-title-group" style={{ textAlign: 'left' }}>
              <div className="approval-title-text" style={{ textAlign: 'left' }}>{details.title}</div>
            </div>
            <span className={`approval-badge-chip ${toolMeta.badgeClass}`}>
              {toolMeta.badge}
            </span>
          </div>

          {/* Action / Question Content */}
          {diffText ? (
            <div className="approval-card-diff-container" style={{ width: '100%' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
                <span style={{ fontSize: '0.82rem', fontWeight: 600, color: 'var(--text-soft, #a1a1aa)' }}>
                  Diff: <code style={{ color: 'var(--text-main, #f4f4f5)' }}>{targetFilePath}</code>
                </span>
                <button
                  type="button"
                  className="management-action-btn"
                  onClick={() => setShowDiffModal(true)}
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: 6,
                    padding: '4px 10px',
                    fontSize: '0.75rem',
                    borderRadius: '8px',
                    background: 'rgba(56, 189, 248, 0.12)',
                    border: '1px solid rgba(56, 189, 248, 0.3)',
                    color: '#38bdf8',
                    cursor: 'pointer',
                  }}
                >
                  Review Side-by-Side
                </button>
              </div>
              {renderDiff(diffText)}
            </div>
          ) : (
            <div className={`approval-content-box ${isAskUser ? 'question-text' : ''}`}>
              {details.body}
            </div>
          )}

          {details.reason && !diffText && (
            <div className="approval-reason-text">{details.reason}</div>
          )}

          {/* Claude Desktop Choice Cards Stack */}
          {isAskUser ? (
            <div className="claude-choice-stack">
              {askOptions.length > 0 && (
                <div className="claude-choice-options">
                  {askOptions.map((option, i) => {
                    const isSelected = isMultiSelect && selectedOptions.has(option.label)
                    return (
                      <button
                        key={i}
                        type="button"
                        className={`claude-choice-card ${isSelected ? 'selected' : ''}`}
                        onClick={() =>
                          isMultiSelect
                            ? toggleOption(option.label)
                            : onApproval(true, false, option.label)
                        }
                      >
                        <div className="claude-choice-left">
                          <span className="claude-choice-key">{i + 1}</span>
                          <div className="claude-choice-text">
                            <span className="claude-choice-title">{option.label}</span>
                            {option.description && (
                              <span className="claude-choice-desc">{option.description}</span>
                            )}
                          </div>
                        </div>
                        <div className="claude-choice-right">
                          {isMultiSelect ? (
                            <span style={{ fontSize: '1.1rem', color: isSelected ? '#10b981' : 'var(--text-muted)' }}>
                              {isSelected ? '☑' : '☐'}
                            </span>
                          ) : (
                            <span style={{ fontSize: '1rem', opacity: 0.6 }}>›</span>
                          )}
                        </div>
                      </button>
                    )
                  })}
                </div>
              )}

              {/* Custom Answer Input Box */}
              <div className="claude-choice-custom-box">
                <div style={{ display: 'flex', gap: 8, alignItems: 'center', width: '100%' }}>
                  <input
                    type="text"
                    className="claude-choice-input"
                    placeholder={
                      askOptions.length > 0
                        ? 'Or type custom response (press Enter to submit)...'
                        : 'Type your response (press Enter to submit)...'
                    }
                    value={askAnswer}
                    onChange={(e) => setAskAnswer(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && !e.shiftKey) {
                        e.preventDefault()
                        if (isMultiSelect) {
                          onApproval(true, false, buildMultiSelectAnswer())
                        } else if (askAnswer.trim()) {
                          onApproval(true, false, askAnswer.trim())
                        }
                      }
                    }}
                  />
                  {(isMultiSelect || askAnswer.trim()) && (
                    <button
                      type="button"
                      className="approval-btn approval-btn-approve"
                      style={{ whiteSpace: 'nowrap', padding: '9px 16px', borderRadius: '8px' }}
                      disabled={isMultiSelect ? selectedOptions.size === 0 && !askAnswer.trim() : !askAnswer.trim()}
                      onClick={() =>
                        onApproval(true, false, isMultiSelect ? buildMultiSelectAnswer() : askAnswer.trim())
                      }
                    >
                      Submit
                    </button>
                  )}
                </div>
              </div>

              {/* Decline Choice Card */}
              <button
                type="button"
                className="claude-choice-card variant-cancel"
                onClick={() => onApproval(false)}
                style={{ marginTop: 2 }}
              >
                <div className="claude-choice-left">
                  <span className="claude-choice-key">✕</span>
                  <div className="claude-choice-text">
                    <span className="claude-choice-title">Decline / Skip</span>
                  </div>
                </div>
                <div className="claude-choice-right">›</div>
              </button>
            </div>
          ) : (
            <div className="claude-choice-stack">
              {actionChoices.map((choice) => (
                <button
                  key={choice.key}
                  type="button"
                  className={`claude-choice-card variant-${choice.variant}`}
                  onClick={choice.action}
                >
                  <div className="claude-choice-left">
                    <span className="claude-choice-key">{choice.key}</span>
                    <div className="claude-choice-text">
                      <span className="claude-choice-title">{choice.title}</span>
                    </div>
                  </div>
                  <div className="claude-choice-right">›</div>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {showDiffModal && diffText && (
        <DiffReviewModal
          isOpen={showDiffModal}
          onClose={() => setShowDiffModal(false)}
          filePath={targetFilePath}
          diffText={diffText}
          isDangerous={details.isDangerous}
          onApprove={() => {
            setShowDiffModal(false)
            onApproval(true)
          }}
          onReject={() => {
            setShowDiffModal(false)
            onApproval(false)
          }}
        />
      )}
    </div>
  )
}

