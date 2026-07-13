/**
 * shared/components/AgentActivityDrawer.tsx
 * Togglable drawer containing the agent activities list/table.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import React from 'react'
import type { AgentActivityView } from '../utils/agentActivity'
import { AgentActivityTable } from './AgentActivityTable'


interface Props {
  activityView: AgentActivityView
  isOpen: boolean
  onToggle: () => void
  isHistorical?: boolean
  pendingApproval?: boolean
  key?: any
}


export function AgentActivityDrawer({
  activityView,
  isOpen,
  onToggle,
  isHistorical = false,
  pendingApproval = false,
}: Props) {
  if (activityView.items.length === 0) return null

  if (isHistorical) {
    return (
      <div className="agent-activity-history">
        <button
          type="button"
          className="agent-activity-toggle"
          aria-expanded={isOpen}
          onClick={onToggle}
        >
          <span>{activityView.summary}</span>
          <span aria-hidden="true">{isOpen ? '^' : '>'}</span>
        </button>
        {isOpen && (
          <div className="agent-activity-card agent-activity-card-history">
            <AgentActivityTable activityView={activityView} />
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
          <span>{activityView.summary}</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span className="agent-activity-status" data-state={pendingApproval ? 'approval' : 'active'}>
              {pendingApproval ? 'Waiting for approval' : 'Working'}
            </span>
            <span aria-hidden="true">{isOpen ? '^' : '>'}</span>
          </div>
        </div>
        {isOpen && (
          <div style={{ marginTop: '8px' }}>
            <AgentActivityTable activityView={activityView} />
          </div>
        )}
      </div>
    </div>
  )
}
