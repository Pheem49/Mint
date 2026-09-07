import React from 'react'
import type { ActivePlan } from '../types'
import { CheckCircle2, Circle, Loader2, XCircle, ListTodo } from 'lucide-react'

interface PlanChecklistWidgetProps {
  plan: ActivePlan
  className?: string
}

export function PlanChecklistWidget({ plan, className = '' }: PlanChecklistWidgetProps) {
  if (!plan || !plan.tasks || plan.tasks.length === 0) {
    return null
  }

  const completedCount = plan.tasks.filter((t) => t.status === 'completed').length
  const totalCount = plan.tasks.length
  const percent = Math.round((completedCount / totalCount) * 100)

  return (
    <div className={`plan-checklist-card ${className}`}>
      <div className="plan-checklist-header">
        <div className="plan-checklist-title">
          <ListTodo style={{ width: '15px', height: '15px', color: 'var(--accent, #10b981)' }} />
          <span>{plan.objective || 'Task Plan'}</span>
        </div>
        <span className="plan-checklist-count">
          {completedCount}/{totalCount} ({percent}%)
        </span>
      </div>

      {/* Progress Bar */}
      <div className="plan-progress-track">
        <div className="plan-progress-bar" style={{ width: `${percent}%` }} />
      </div>

      {/* Tasks List */}
      <div className="plan-tasks-list">
        {plan.tasks.map((task, idx) => {
          const isDone = task.status === 'completed'
          const isRunning = task.status === 'in_progress'
          const isFailed = task.status === 'failed'

          const stateClass = isRunning
            ? 'in-progress'
            : isDone
              ? 'completed'
              : isFailed
                ? 'failed'
                : ''

          return (
            <div key={task.id || idx} className={`plan-task-item ${stateClass}`}>
              <span style={{ display: 'inline-flex', alignItems: 'center', flexShrink: 0 }}>
                {isDone && <CheckCircle2 style={{ width: '14px', height: '14px', color: 'var(--status-speaking, #22c55e)' }} />}
                {isRunning && (
                  <Loader2
                    style={{
                      width: '14px',
                      height: '14px',
                      color: 'var(--status-listening, #7dd3fc)',
                      animation: 'spin 1s linear infinite',
                    }}
                  />
                )}
                {isFailed && <XCircle style={{ width: '14px', height: '14px', color: 'var(--status-error, #ef4444)' }} />}
                {!isDone && !isRunning && !isFailed && (
                  <Circle style={{ width: '14px', height: '14px', color: 'rgba(255, 255, 255, 0.3)' }} />
                )}
              </span>
              <span className="plan-task-text">{task.title}</span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
