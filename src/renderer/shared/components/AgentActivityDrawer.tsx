/**
 * shared/components/AgentActivityDrawer.tsx
 * Togglable drawer containing the agent activities list/table and Subagent DAG View.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import React, { useState } from 'react'
import type { AgentProgress, ActivePlan, RunTelemetrySummary } from '../types'
import type { AgentActivityView } from '../utils/agentActivity'
import { AgentActivityTable } from './AgentActivityTable'
import { SubagentDagView } from './SubagentDagView'
import { PlanChecklistWidget } from './PlanChecklistWidget'
import { RunSummaryDashboard } from './RunSummaryDashboard'

interface Props {
  activityView: AgentActivityView
  isOpen: boolean
  onToggle: () => void
  isHistorical?: boolean
  pendingApproval?: boolean
  rawProgress?: AgentProgress[]
  key?: any
}

export function AgentActivityDrawer({
  activityView,
  isOpen,
  onToggle,
  isHistorical = false,
  pendingApproval = false,
  rawProgress,
}: Props) {
  const [viewMode, setViewMode] = useState<'list' | 'dag'>('list')

  const latestPlan = (rawProgress || [])
    .slice()
    .reverse()
    .find((e): e is { type: 'PlanUpdated'; data: { plan: ActivePlan } } => e.type === 'PlanUpdated')
    ?.data.plan

  const latestRunSummary = (rawProgress || [])
    .slice()
    .reverse()
    .find((e): e is { type: 'RunCompleted'; data: { summary: RunTelemetrySummary } } => e.type === 'RunCompleted')
    ?.data.summary

  if (activityView.items.length === 0 && !latestPlan && !latestRunSummary) return null

  const hasSubagents = (rawProgress || []).some(
    (e) =>
      (e.type === 'ToolStart' || e.type === 'ToolEnd') &&
      (e.data.subagent || e.data.action === 'dispatch_subagent'),
  )

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
            borderBottom: '1px solid rgba(255, 255, 255, 0.08)',
          }}
        >
          <div
            style={{
              display: 'inline-flex',
              padding: '2px',
              background: 'rgba(255, 255, 255, 0.05)',
              borderRadius: '6px',
              border: '1px solid rgba(255, 255, 255, 0.08)',
            }}
          >
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                setViewMode('list')
              }}
              style={{
                background: viewMode === 'list' ? 'var(--accent, #10b981)' : 'transparent',
                color: viewMode === 'list' ? '#ffffff' : 'var(--text-muted, #9ca3af)',
                border: 'none',
                borderRadius: '4px',
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
                background: viewMode === 'dag' ? 'var(--accent, #10b981)' : 'transparent',
                color: viewMode === 'dag' ? '#ffffff' : 'var(--text-muted, #9ca3af)',
                border: 'none',
                borderRadius: '4px',
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
                  background: '#38bdf8',
                  display: 'inline-block',
                }}
              />
            </button>
          </div>
        </div>
      )}

      {viewMode === 'dag' && hasSubagents ? (
        <SubagentDagView progress={rawProgress || []} isLive={!isHistorical} />
      ) : activityView.items.length > 0 ? (
        <AgentActivityTable activityView={activityView} />
      ) : null}
    </>
  )

  const displaySummary =
    latestPlan?.objective ||
    activityView.summary ||
    (latestRunSummary
      ? `Run Telemetry (${latestRunSummary.outcome})`
      : 'Agent Activity')

  if (isHistorical) {
    return (
      <div className="agent-activity-history">
        <button
          type="button"
          className="agent-activity-toggle"
          aria-expanded={isOpen}
          onClick={onToggle}
        >
          <span>{displaySummary}</span>
          <span aria-hidden="true">{isOpen ? '^' : '>'}</span>
        </button>
        {isOpen && (
          <div className="agent-activity-card agent-activity-card-history">
            {renderContent()}
          </div>
        )}
      </div>
    )
  }

  // Active / Live view
  return (
    <div className="message ai-message agent-activity-message">
      <div className="agent-activity-card">
        <div className="agent-activity-header" style={{ cursor: 'pointer' }} onClick={onToggle}>
          <span>{displaySummary}</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span
              className="agent-activity-status"
              data-state={pendingApproval ? 'approval' : latestRunSummary ? 'done' : 'active'}
            >
              {pendingApproval ? 'Waiting for approval' : latestRunSummary ? 'Completed' : 'Working'}
            </span>
            <span aria-hidden="true">{isOpen ? '^' : '>'}</span>
          </div>
        </div>
        {isOpen && (
          <div style={{ marginTop: '8px' }}>
            {renderContent()}
          </div>
        )}
      </div>
    </div>
  )
}
