import React from 'react'
import type { RunTelemetrySummary } from '../../types'
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

export function formatRunDuration(secs: number): { value: string; subtext?: string } {
  if (secs < 60) {
    return { value: `${secs.toFixed(1)}s` }
  }
  const mins = Math.floor(secs / 60)
  const remSecs = secs % 60
  const remSecsStr = remSecs < 10 ? `0${remSecs.toFixed(1)}` : remSecs.toFixed(1)

  if (mins >= 60) {
    const hours = Math.floor(mins / 60)
    const remMins = mins % 60
    return {
      value: `${hours}h ${remMins}m`,
      subtext: `${secs.toFixed(1)}s`,
    }
  }

  return {
    value: `${mins}m ${remSecsStr}s`,
    subtext: `${secs.toFixed(1)}s`,
  }
}

export function RunSummaryDashboard({ summary, className = '' }: Props) {
  const isSuccess = summary.outcome === 'SUCCESS'
  const isRolledBack = summary.outcome === 'ROLLED_BACK'
  const isFailed = summary.outcome === 'FAILED'

  const formattedTokens =
    summary.totalTokens >= 1000
      ? `${(summary.totalTokens / 1000).toFixed(1)}k`
      : summary.totalTokens.toString()

  const formattedDuration = formatRunDuration(summary.durationSecs)

  const badgeClass = isSuccess
    ? 'run-summary-badge success'
    : isRolledBack
      ? 'run-summary-badge rollback'
      : 'run-summary-badge failure'

  return (
    <div className={`run-summary-card ${className}`}>
      {/* Header */}
      <div className="run-summary-header">
        <div className="run-summary-title">
          <Activity size={16} aria-hidden="true" />
          <span className="run-summary-heading"><strong>Run summary</strong><small title={`Run ${summary.runId}`}>#{summary.runId}</small></span>
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
          <div className="run-kpi-value" title={`${summary.durationSecs.toFixed(1)}s`}>
            {formattedDuration.value}
            {formattedDuration.subtext && (
              <span className="run-kpi-subtext"> · {formattedDuration.subtext}</span>
            )}
          </div>
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
            {summary.toolCallsCount}
            {summary.retriesCount > 0 && <span className="run-kpi-subtext"> · {summary.retriesCount} retried</span>}
          </div>
        </div>

        <div className="run-kpi-box">
          <div className="run-kpi-label">
            <FileCode2 style={{ width: '12px', height: '12px' }} />
            <span>Files</span>
          </div>
          <div className="run-kpi-value">
            {summary.filesCreated && summary.filesCreated.length > 0 && summary.filesChanged.length - summary.filesCreated.length > 0 ? (
              <>
                {summary.filesCreated.length} <span className="run-kpi-subtext">created</span>, {summary.filesChanged.length - summary.filesCreated.length} <span className="run-kpi-subtext">modified</span>
              </>
            ) : summary.filesCreated && summary.filesCreated.length > 0 ? (
              <>
                {summary.filesCreated.length}{' '}
                <span className="run-kpi-subtext">created</span>
              </>
            ) : summary.filesChanged.length > 0 ? (
              <>
                {summary.filesChanged.length}{' '}
                <span className="run-kpi-subtext">modified</span>
              </>
            ) : <>0 <span className="run-kpi-subtext">files</span></>}
          </div>
        </div>
      </div>
    </div>
  )
}
