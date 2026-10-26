import React, { useState, useCallback } from 'react'
import type { RunTelemetrySummary, StandardTrajectoryRun } from '../../types'
import {
  CheckCircle2,
  XCircle,
  RotateCcw,
  Clock,
  Coins,
  Wrench,
  FileCode2,
  Activity,
  Download,
  Check,
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
  const [exported, setExported] = useState(false)

  const handleExportTrajectory = useCallback(() => {
    try {
      const trajectoryData: StandardTrajectoryRun = summary.trajectory || {
        schemaVersion: '1.0',
        runId: summary.runId,
        chatId: 'default',
        task: summary.task || 'Agent Run',
        model: 'unknown',
        provider: 'unknown',
        startedAt: new Date(Date.now() - summary.durationSecs * 1000).toISOString(),
        completedAt: new Date().toISOString(),
        outcome: summary.outcome,
        totalSteps: summary.toolCallsCount,
        totalTokens: summary.totalTokens,
        durationSecs: summary.durationSecs,
        filesCreated: summary.filesCreated || [],
        filesModified: (summary.filesChanged || []).filter(
          (f) => !(summary.filesCreated || []).includes(f)
        ),
        trajectory: (summary.toolTimeline || []).map((item, idx) => ({
          step: item.step || idx + 1,
          timestamp: new Date().toISOString(),
          thought: '',
          action: item.action,
          input: { target: item.target },
          observation: item.success ? 'Success' : 'Failed',
          status: item.success ? 'success' : 'failed',
          durationMs: 0,
          tokens: { inputTokens: 0, generatedTokens: 0 },
        })),
      }

      const json = JSON.stringify(trajectoryData, null, 2)
      if (typeof window !== 'undefined' && typeof document !== 'undefined') {
        const blob = new Blob([json], { type: 'application/json' })
        const url = URL.createObjectURL(blob)
        const a = document.createElement('a')
        a.href = url
        a.download = `${summary.runId}-trajectory.json`
        document.body.appendChild(a)
        a.click()
        document.body.removeChild(a)
        URL.revokeObjectURL(url)
      }
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        navigator.clipboard.writeText(json).catch(() => {})
      }
      setExported(true)
      setTimeout(() => setExported(false), 2000)
    } catch (e) {
      console.error('Failed to export trajectory:', e)
    }
  }, [summary])

  const isSuccess = summary.outcome === 'SUCCESS' || summary.outcome === 'VERIFIED'
  const isCompleted = summary.outcome === 'PARTIAL' || summary.outcome === 'COMPLETED'
  const isRolledBack = summary.outcome === 'ROLLED_BACK'
  const isUnverified = summary.outcome === 'UNVERIFIED'
  const isFailed = summary.outcome === 'FAILED'

  const formattedTokens =
    summary.totalTokens >= 1000
      ? `${(summary.totalTokens / 1000).toFixed(1)}k`
      : summary.totalTokens.toString()

  const formattedDuration = formatRunDuration(summary.durationSecs)

  const badgeClass = isSuccess || isCompleted
    ? 'run-summary-badge success'
    : isRolledBack || isUnverified
      ? 'run-summary-badge rollback'
      : 'run-summary-badge failure'

  const badgeText = isSuccess
    ? (summary.outcome === 'VERIFIED' ? 'Verified' : 'Success')
    : isCompleted
      ? 'Completed'
      : isRolledBack
        ? 'Rolled Back'
        : isUnverified
          ? 'Unverified'
          : 'Failure'

  return (
    <div className={`run-summary-card ${className}`}>
      {/* Header */}
      <div className="run-summary-header">
        <div className="run-summary-title">
          <Activity size={16} aria-hidden="true" />
          <span className="run-summary-heading"><strong>Run summary</strong><small title={`Run ${summary.runId}`}>#{summary.runId}</small></span>
        </div>
        <div className="run-summary-actions">
          <button
            type="button"
            className="run-summary-export-btn"
            onClick={handleExportTrajectory}
            title="Export SWE-bench / Agent Protocol compliant trajectory JSON"
            aria-label="Export trajectory"
          >
            {exported ? (
              <Check style={{ width: '12px', height: '12px' }} />
            ) : (
              <Download style={{ width: '12px', height: '12px' }} />
            )}
            <span>{exported ? 'Exported!' : 'Export Trajectory'}</span>
          </button>
          <span className={badgeClass}>
            {(isSuccess || isCompleted) && <CheckCircle2 style={{ width: '12px', height: '12px' }} />}
            {(isRolledBack || isUnverified) && <RotateCcw style={{ width: '12px', height: '12px' }} />}
            {(!isSuccess && !isCompleted && !isRolledBack && !isUnverified) && <XCircle style={{ width: '12px', height: '12px' }} />}
            <span>{badgeText}</span>
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
