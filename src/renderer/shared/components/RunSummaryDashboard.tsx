import React from 'react'
import type { RunTelemetrySummary } from '../types'
import {
  CheckCircle2,
  XCircle,
  RotateCcw,
  Clock,
  Coins,
  Wrench,
  FileCode2,
  Activity,
} from 'lucide-react'

interface Props {
  summary: RunTelemetrySummary
  className?: string
}

export function RunSummaryDashboard({ summary, className = '' }: Props) {
  const isSuccess = summary.outcome === 'SUCCESS'
  const isRolledBack = summary.outcome === 'ROLLED_BACK'
  const isFailed = summary.outcome === 'FAILED'

  const formattedTokens =
    summary.totalTokens >= 1000
      ? `${(summary.totalTokens / 1000).toFixed(1)}k`
      : summary.totalTokens.toString()

  const badgeClass = isSuccess
    ? 'run-summary-badge success'
    : isRolledBack
      ? 'run-summary-badge rollback'
      : 'run-summary-badge failure'

  const statusColor = isSuccess
    ? 'var(--status-speaking, #22c55e)'
    : isRolledBack
      ? '#f59e0b'
      : 'var(--status-error, #ef4444)'

  return (
    <div className={`run-summary-card ${className}`}>
      {/* Header */}
      <div className="run-summary-header">
        <div className="run-summary-title">
          <Activity style={{ width: '15px', height: '15px', color: statusColor }} />
          <span>Run Telemetry #{summary.runId}</span>
        </div>
        <div>
          <span className={badgeClass}>
            {isSuccess && <CheckCircle2 style={{ width: '12px', height: '12px' }} />}
            {isRolledBack && <RotateCcw style={{ width: '12px', height: '12px' }} />}
            {isFailed && <XCircle style={{ width: '12px', height: '12px' }} />}
            <span>{isSuccess ? 'Success' : isRolledBack ? 'Rolled Back' : 'Failure'}</span>
          </span>
        </div>
      </div>

      {/* KPI Grid */}
      <div className="run-kpi-grid">
        <div className="run-kpi-box">
          <div className="run-kpi-label">
            <Clock style={{ width: '12px', height: '12px' }} />
            <span>Duration</span>
          </div>
          <div className="run-kpi-value">{summary.durationSecs.toFixed(1)}s</div>
        </div>

        <div className="run-kpi-box">
          <div className="run-kpi-label">
            <Coins style={{ width: '12px', height: '12px' }} />
            <span>Tokens</span>
          </div>
          <div className="run-kpi-value">{formattedTokens}</div>
        </div>

        <div className="run-kpi-box">
          <div className="run-kpi-label">
            <Wrench style={{ width: '12px', height: '12px' }} />
            <span>Tools</span>
          </div>
          <div className="run-kpi-value">
            {summary.toolCallsCount}{' '}
            <span className="run-kpi-subtext">({summary.retriesCount} retry)</span>
          </div>
        </div>

        <div className="run-kpi-box">
          <div className="run-kpi-label">
            <FileCode2 style={{ width: '12px', height: '12px' }} />
            <span>Files</span>
          </div>
          <div className="run-kpi-value">
            {summary.filesChanged.length}{' '}
            <span className="run-kpi-subtext">modified</span>
          </div>
        </div>
      </div>

      {/* Tool Calls Timeline */}
      {summary.toolTimeline && summary.toolTimeline.length > 0 && (
        <div className="run-timeline-container">
          <div className="run-timeline-title">Tool Calls Timeline</div>
          <div className="run-timeline-list">
            {summary.toolTimeline.map((item, idx) => (
              <div key={idx} className="run-timeline-item">
                <div className="run-timeline-left">
                  <span className="run-timeline-step">
                    {String(item.step).padStart(2, '0')}s
                  </span>
                  {!item.success ? (
                    <XCircle style={{ width: '14px', height: '14px', color: 'var(--status-error, #ef4444)', flexShrink: 0 }} />
                  ) : item.retried ? (
                    <RotateCcw style={{ width: '14px', height: '14px', color: '#f59e0b', flexShrink: 0 }} />
                  ) : (
                    <CheckCircle2 style={{ width: '14px', height: '14px', color: 'var(--status-speaking, #22c55e)', flexShrink: 0 }} />
                  )}
                  <span className="run-timeline-action">{item.action}</span>
                  {item.target && <span className="run-timeline-target">[{item.target}]</span>}
                </div>
                <div>
                  {!item.success && (
                    <span className="run-timeline-note-failed">(failed)</span>
                  )}
                  {item.retried && item.success && (
                    <span className="run-timeline-note-retry">(self-corrected)</span>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
