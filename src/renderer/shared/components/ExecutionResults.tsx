import { memo, useState } from 'react'
import {
  ShieldCheck,
  ShieldAlert,
  AlertTriangle,
  XCircle,
  Terminal,
  CheckCheck,
  Check,
  X,
  Loader2,
  ChevronDown,
  ChevronRight,
  Copy,
  CheckCheck as CopiedCheck
} from 'lucide-react'
import type { AgentProgress, ExecutionEvidence } from '../types'
import '../css/execution-results.css'

const labels = { verified: 'Verified', unverified: 'Unverified', blocked: 'Blocked', failed: 'Failed' } as const

export default memo(function ExecutionResults({ progress }: { progress: AgentProgress[] }) {
  const executions = new Map<string, ExecutionEvidence>()
  let outcome = ''
  for (const event of progress) {
    if (event.type === 'ExecutionUpdate') {
      for (const entry of event.data.executions) executions.set(`${entry.callId}:${entry.command}`, entry)
    } else if (event.type === 'RunCompleted') outcome = event.data.summary.outcome.toLowerCase()
  }

  const label = labels[outcome as keyof typeof labels]
  const [isSectionOpen, setIsSectionOpen] = useState(true)
  const [expandedItems, setExpandedItems] = useState<Record<string, boolean>>({})
  const [copiedId, setCopiedId] = useState<string | null>(null)

  if (!executions.size && !label) return null

  const copyCommand = (id: string, text: string) => {
    navigator.clipboard?.writeText(text)
    setCopiedId(id)
    setTimeout(() => setCopiedId(null), 1500)
  }

  const toggleItem = (id: string) => {
    setExpandedItems(prev => ({ ...prev, [id]: !prev[id] }))
  }

  const renderHeaderIcon = () => {
    switch (outcome) {
      case 'verified':
        return <ShieldCheck size={16} className="execution-header-icon verified" aria-hidden="true" />
      case 'failed':
        return <XCircle size={16} className="execution-header-icon failed" aria-hidden="true" />
      case 'unverified':
        return <AlertTriangle size={16} className="execution-header-icon unverified" aria-hidden="true" />
      case 'blocked':
        return <ShieldAlert size={16} className="execution-header-icon blocked" aria-hidden="true" />
      default:
        return <Terminal size={16} className="execution-header-icon running" aria-hidden="true" />
    }
  }

  const total = executions.size
  const items = [...executions.values()]
  const succeededCount = items.filter(e => e.state === 'succeeded').length
  const failedCount = items.filter(e => e.state === 'failed').length
  const runningCount = items.filter(e => e.state === 'running').length
  const checksCount = items.filter(e => e.check).length

  let summaryText = ''
  if (runningCount > 0) {
    summaryText = `${runningCount} running · ${succeededCount} succeeded`
  } else if (failedCount > 0) {
    summaryText = `${failedCount} failed · ${succeededCount} succeeded`
  } else if (total > 0) {
    summaryText = `${total} ${total === 1 ? 'action' : 'actions'}${checksCount > 0 ? ` · ${checksCount} checks` : ''}`
  }

  return (
    <section className="mcp-results execution-results-card" aria-label="Execution results">
      <div
        className={`execution-results-header ${total > 0 ? 'is-clickable' : ''}`}
        onClick={() => total > 0 && setIsSectionOpen(v => !v)}
      >
        <div className="execution-header-left">
          {renderHeaderIcon()}
          <span className="execution-header-title">
            {outcome === 'verified' ? 'Verified Evidence' : 'Execution Verification'}
          </span>
          {label && (
            <span
              className={`execution-outcome-pill ${outcome}`}
              role="status"
              data-completion-status={outcome}
            >
              {label}
            </span>
          )}
        </div>
        {total > 0 && (
          <div className="execution-header-right">
            {summaryText && <span className="execution-header-summary">{summaryText}</span>}
            <button
              type="button"
              className="execution-toggle-btn"
              aria-expanded={isSectionOpen}
              aria-label={isSectionOpen ? 'Collapse execution evidence' : 'Expand execution evidence'}
              onClick={e => {
                e.stopPropagation()
                setIsSectionOpen(v => !v)
              }}
            >
              <ChevronDown
                size={14}
                className={`execution-toggle-chevron ${isSectionOpen ? 'is-open' : ''}`}
                aria-hidden="true"
              />
            </button>
          </div>
        )}
      </div>

      {total > 0 && isSectionOpen && (
        <div className="execution-items-container">
          {items.map(entry => {
            const id = `${entry.callId}:${entry.command}`
            const isExpanded = Boolean(expandedItems[id])
            return (
              <div key={id} className={`execution-item ${isExpanded ? 'is-expanded' : ''}`}>
                <div
                  className="execution-item-row"
                  onClick={() => toggleItem(id)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={e => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault()
                      toggleItem(id)
                    }
                  }}
                  aria-expanded={isExpanded}
                >
                  {entry.check ? (
                    <CheckCheck size={14} className="execution-item-kind-icon check" aria-hidden="true" />
                  ) : (
                    <Terminal size={14} className="execution-item-kind-icon command" aria-hidden="true" />
                  )}
                  <span className={`execution-kind-tag ${entry.check ? 'check' : 'command'}`}>
                    {entry.check ? 'Check' : 'Command'}
                  </span>
                  <code className="execution-command-preview" title={entry.command}>
                    {entry.command}
                  </code>
                  <div className="execution-item-meta">
                    <span className={`execution-state execution-state-${entry.state.toLowerCase()}`}>
                      {entry.state === 'succeeded' && (
                        <Check size={11} className="execution-state-icon" strokeWidth={2.5} aria-hidden="true" />
                      )}
                      {entry.state === 'running' && (
                        <Loader2 size={11} className="execution-state-icon spin" aria-hidden="true" />
                      )}
                      {entry.state === 'failed' && (
                        <X size={11} className="execution-state-icon" strokeWidth={2.5} aria-hidden="true" />
                      )}
                      <span>{entry.state}</span>
                    </span>
                    {entry.exitCode != null && (
                      <span className={`execution-exit${entry.exitCode !== 0 ? ' execution-exit-error' : ''}`}>
                        exit {entry.exitCode}
                      </span>
                    )}
                    {entry.conditionVerified === false && (
                      <span className="execution-unverified-tag">result unverified</span>
                    )}
                    <ChevronRight
                      size={13}
                      className={`execution-item-chevron ${isExpanded ? 'is-open' : ''}`}
                      aria-hidden="true"
                    />
                  </div>
                </div>

                {isExpanded && (
                  <div className="execution-item-detail">
                    <div className="execution-detail-header">
                      <span className="execution-detail-label">
                        {entry.check ? 'Verification Check' : 'Shell Command'}
                      </span>
                      <button
                        type="button"
                        className={`execution-copy-btn ${copiedId === id ? 'is-copied' : ''}`}
                        onClick={e => {
                          e.stopPropagation()
                          copyCommand(id, entry.command)
                        }}
                        title="Copy command to clipboard"
                      >
                        {copiedId === id ? (
                          <CopiedCheck size={12} aria-hidden="true" />
                        ) : (
                          <Copy size={12} aria-hidden="true" />
                        )}
                        <span>{copiedId === id ? 'Copied' : 'Copy'}</span>
                      </button>
                    </div>
                    <pre className="execution-full-command">
                      <code>{entry.command}</code>
                    </pre>
                    {(entry.jobId || entry.revision > 1 || entry.conditionVerified != null) && (
                      <div className="execution-item-meta-row">
                        {entry.jobId && <span className="execution-meta-pill">Job: {entry.jobId}</span>}
                        {entry.revision > 1 && <span className="execution-meta-pill">Rev: {entry.revision}</span>}
                        {entry.conditionVerified != null && (
                          <span
                            className={`execution-meta-pill ${entry.conditionVerified ? 'verified' : 'unverified'}`}
                          >
                            Condition: {entry.conditionVerified ? 'Verified' : 'Unverified'}
                          </span>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
})
