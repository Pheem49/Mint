/**
 * shared/components/AgentActivityDrawer.tsx
 * Togglable deep module containing the agent activities list/table, PlanChecklist, RunSummary, and Subagent DAG View.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import React, { memo, useCallback, useMemo, useState } from 'react'
import { ChevronDown } from 'lucide-react'
import type { ArtifactFile } from '../../utils/artifacts'
import type { AgentProgress } from '../../types'
import {
  activitiesFrom,
  extractLatestPlan,
  extractLatestRunSummary,
  hasSubagentActivity,
  type AgentActivityView,
} from '../../utils/agentActivity'
import { AgentActivityTable } from './AgentActivityTable'
import { SubagentDagView } from './SubagentDagView'
import { PlanChecklistWidget } from './PlanChecklistWidget'
import { RunSummaryDashboard } from './RunSummaryDashboard'

export interface AgentActivityDrawerProps {
  progress?: AgentProgress[]
  activityView?: AgentActivityView
  rawProgress?: AgentProgress[]
  id?: string
  isOpen?: boolean
  defaultOpen?: boolean
  onToggle?: (id: string) => void
  historical?: boolean
  isHistorical?: boolean
  pendingApproval?: boolean
  chatId?: string
  workspacePath?: string
  onReferenceFile?: (filePath: string) => void
  onOpenArtifact?: (artifact: ArtifactFile) => void
  key?: any
}

export const AgentActivityDrawer = memo(function AgentActivityDrawer({
  progress,
  activityView: providedActivityView,
  rawProgress,
  id = '',
  isOpen: controlledIsOpen,
  defaultOpen = false,
  onToggle,
  historical,
  isHistorical: isHistoricalProp,
  pendingApproval = false,
  chatId,
  workspacePath,
  onReferenceFile,
  onOpenArtifact,
}: AgentActivityDrawerProps) {
  const effectiveProgress = progress ?? rawProgress ?? []
  const isHistorical = historical ?? isHistoricalProp ?? false
  const [internalOpen, setInternalOpen] = useState(defaultOpen)
  const isOpen = controlledIsOpen !== undefined ? controlledIsOpen : internalOpen

  const handleToggle = useCallback(() => {
    if (onToggle) {
      onToggle(id)
    } else {
      setInternalOpen((prev) => !prev)
    }
  }, [id, onToggle])

  const [viewMode, setViewMode] = useState<'list' | 'dag'>('list')

  const activityView = useMemo(
    () => providedActivityView ?? activitiesFrom(effectiveProgress),
    [providedActivityView, effectiveProgress],
  )

  const latestPlan = useMemo(() => extractLatestPlan(effectiveProgress), [effectiveProgress])
  const latestRunSummary = useMemo(() => extractLatestRunSummary(effectiveProgress), [effectiveProgress])
  const hasSubagents = useMemo(() => hasSubagentActivity(effectiveProgress), [effectiveProgress])

  if (
    activityView.items.length === 0 &&
    (!activityView.timeline || activityView.timeline.length === 0) &&
    !latestPlan &&
    !latestRunSummary
  ) {
    return null
  }

  const renderContent = () => (
    <>
      {latestRunSummary && (
        <div style={{ marginBottom: '12px' }}>
          <RunSummaryDashboard summary={latestRunSummary} />
        </div>
      )}

      {latestPlan && (
        <div style={{ marginBottom: '12px' }}>
          <PlanChecklistWidget plan={latestPlan} />
        </div>
      )}

      {hasSubagents && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'flex-start',
            gap: '8px',
            marginBottom: '10px',
            paddingBottom: '8px',
            borderBottom: '1px solid var(--border)',
          }}
        >
          <div
            style={{
              display: 'inline-flex',
              padding: '2px',
              background: 'var(--surface-strong)',
              borderRadius: 'var(--radius-xs, 6px)',
              border: '1px solid var(--border)',
            }}
          >
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                setViewMode('list')
              }}
              style={{
                background: viewMode === 'list' ? 'var(--accent)' : 'transparent',
                color: viewMode === 'list' ? 'var(--text-on-accent)' : 'var(--text-muted)',
                border: 'none',
                borderRadius: 'var(--radius-2xs, 4px)',
                padding: '3px 8px',
                fontSize: '0.74rem',
                fontWeight: 500,
                cursor: 'pointer',
              }}
            >
              Activity List
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                setViewMode('dag')
              }}
              style={{
                background: viewMode === 'dag' ? 'var(--accent)' : 'transparent',
                color: viewMode === 'dag' ? 'var(--text-on-accent)' : 'var(--text-muted)',
                border: 'none',
                borderRadius: 'var(--radius-2xs, 4px)',
                padding: '3px 8px',
                fontSize: '0.74rem',
                fontWeight: 500,
                cursor: 'pointer',
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
              }}
            >
              <span>Subagent DAG</span>
              <span
                style={{
                  width: '6px',
                  height: '6px',
                  borderRadius: '50%',
                  background: 'var(--status-listening, #38bdf8)',
                  display: 'inline-block',
                }}
              />
            </button>
          </div>
        </div>
      )}

      {viewMode === 'dag' && hasSubagents ? (
        <SubagentDagView progress={effectiveProgress} isLive={!isHistorical} />
      ) : activityView.items.length > 0 || (activityView.timeline && activityView.timeline.length > 0) ? (
        <AgentActivityTable
          activityView={activityView}
          progress={effectiveProgress}
          chatId={chatId}
          workspacePath={workspacePath}
          onReferenceFile={onReferenceFile}
          onOpenArtifact={onOpenArtifact}
        />
      ) : null}
    </>
  )

  const naturalActionSummary =
    latestPlan?.objective ||
    activityView.naturalSummary ||
    activityView.summary ||
    (latestRunSummary ? `Run telemetry (${latestRunSummary.outcome})` : 'Working through task')

  const durationText =
    activityView.durationText ||
    (latestRunSummary?.durationSecs ? `${Math.round(latestRunSummary.durationSecs)}s` : '')

  const workedHeadline = durationText
    ? `Worked for ${durationText}`
    : (isHistorical || latestRunSummary) ? 'Worked' : 'Working'

  if (isHistorical) {
    return (
      <div className="agent-activity-history modern-activity-history">
        <button
          type="button"
          className="agent-activity-toggle modern-activity-toggle"
          aria-expanded={isOpen}
          onClick={handleToggle}
        >
          <div className="modern-activity-header-left">
            <span className="modern-activity-duration">{workedHeadline}</span>
            <ChevronDown
              size={13}
              className={`agent-activity-toggle-chevron${isOpen ? ' is-open' : ''}`}
              aria-hidden="true"
            />
          </div>
          <span className="modern-activity-natural-summary">{naturalActionSummary}</span>
        </button>
        {isOpen && (
          <div className="agent-activity-card modern-activity-card agent-activity-card-history">
            {renderContent()}
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="message ai-message agent-activity-message modern-activity-message">
      <div className="agent-activity-card modern-activity-card">
        <div className="agent-activity-header modern-activity-header" style={{ cursor: 'pointer' }} onClick={handleToggle}>
          <div className="modern-activity-header-left">
            <span className="modern-activity-duration">{workedHeadline}</span>
            <ChevronDown
              size={13}
              className={`agent-activity-toggle-chevron${isOpen ? ' is-open' : ''}`}
              aria-hidden="true"
            />
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span
              className="agent-activity-status"
              data-state={pendingApproval ? 'approval' : latestRunSummary ? 'done' : 'active'}
            >
              {pendingApproval ? 'Waiting for approval' : latestRunSummary ? 'Completed' : 'Working'}
            </span>
          </div>
        </div>
        <div className="modern-activity-subheader" style={{ cursor: 'pointer' }} onClick={handleToggle}>
          <span>{naturalActionSummary}</span>
          <ChevronDown
            size={12}
            className={`modern-activity-subchevron${isOpen ? ' is-open' : ''}`}
            aria-hidden="true"
          />
        </div>
        {isOpen && <div className="modern-activity-content">{renderContent()}</div>}
      </div>
    </div>
  )
})

export const ProgressActivity = AgentActivityDrawer
