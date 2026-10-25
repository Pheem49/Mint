import React, { useMemo } from 'react'
import type { AgentProgress } from '../types'
import { parseSubagentGraph } from './SubagentDagView'

export interface SubagentStatusStripProps {
  progress: AgentProgress[]
  onOpenDetails?: () => void
}

export function SubagentStatusStrip({ progress, onOpenDetails }: SubagentStatusStripProps) {
  const { subagents } = useMemo(() => parseSubagentGraph(progress), [progress])

  if (!subagents || subagents.length === 0) {
    return null
  }

  const runningCount = subagents.filter(s => s.status === 'running').length
  const completedCount = subagents.filter(s => s.status === 'completed').length
  const failedCount = subagents.filter(s => s.status === 'failed').length
  const anyRunning = runningCount > 0

  const title = anyRunning
    ? `Running Subagents (${completedCount}/${subagents.length} completed)`
    : failedCount > 0
    ? `Subagents (${completedCount} completed, ${failedCount} failed)`
    : `Subagents (${completedCount} completed)`

  return (
    <div
      className="subagent-status-strip"
      data-testid="subagent-status-strip"
      onClick={onOpenDetails}
      role={onOpenDetails ? 'button' : undefined}
      tabIndex={onOpenDetails ? 0 : undefined}
      onKeyDown={e => {
        if (onOpenDetails && (e.key === 'Enter' || e.key === ' ')) {
          e.preventDefault()
          onOpenDetails()
        }
      }}
      title={onOpenDetails ? 'Click to inspect subagent activity' : undefined}
    >
      <div className="subagent-status-strip-header">
        <div className="subagent-status-strip-header-left">
          {anyRunning ? (
            <span className="subagent-status-spinner" aria-hidden="true" />
          ) : failedCount > 0 ? (
            <span className="subagent-status-icon-alert" aria-hidden="true">⚠</span>
          ) : (
            <span className="subagent-status-icon-check" aria-hidden="true">✓</span>
          )}
          <span className="subagent-status-strip-title">{title}</span>
        </div>
        {onOpenDetails && (
          <span className="subagent-status-strip-hint">Details ›</span>
        )}
      </div>

      <div className="subagent-status-strip-list">
        {subagents.map(agent => {
          const isRunning = agent.status === 'running'
          const isSuccess = agent.status === 'completed'
          const isFailed = agent.status === 'failed'

          let desc = (agent.taskSnippet && agent.taskSnippet.toLowerCase() !== 'null') ? agent.taskSnippet : 'Working on task...'
          if ((isSuccess || isFailed) && agent.summaryPreview && agent.summaryPreview.toLowerCase() !== 'null') {
            desc = agent.summaryPreview
          } else if (isRunning && agent.currentTool) {
            desc = `Running ${agent.currentTool.replace('_', ' ')}`
          }

          let suffix = ''
          if (isSuccess) {
            const secs = Math.max(1, Math.round((agent.elapsedMs ?? 0) / 1000))
            suffix = `done ${secs}s`
          } else if (isFailed) {
            const secs = Math.max(1, Math.round((agent.elapsedMs ?? 0) / 1000))
            suffix = `failed ${secs}s`
          } else {
            suffix = 'running'
          }

          return (
            <div
              key={agent.name}
              className={`subagent-status-strip-row ${agent.status}`}
              data-testid={`subagent-status-row-${agent.name}`}
            >
              <div className="subagent-status-strip-row-main">
                {isRunning ? (
                  <span className="subagent-status-row-spinner" aria-hidden="true" />
                ) : isSuccess ? (
                  <span className="subagent-status-row-check" aria-hidden="true">✓</span>
                ) : (
                  <span className="subagent-status-row-cross" aria-hidden="true">✗</span>
                )}
                <span className="subagent-status-strip-tag">[{agent.name}]</span>
                <span className="subagent-status-strip-desc">{desc}</span>
              </div>
              <span className="subagent-status-strip-timing">{suffix}</span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
