import React, { memo } from 'react'
import { FormattedMessage } from '../../utils/markdown'

export interface PlanCardProps {
  plan: string
}

export const PlanCard = memo(function PlanCard({ plan }: PlanCardProps) {
  if (!plan || !plan.trim()) return null

  return (
    <div
      className="action-card approval-card plan-inline-card"
      style={{ width: '100%', alignItems: 'stretch', textAlign: 'left', marginBottom: '12px' }}
    >
      <div className="approval-card-header" style={{ width: '100%', textAlign: 'left' }}>
        <div className="approval-title-group" style={{ textAlign: 'left' }}>
          <div className="approval-title-text" style={{ textAlign: 'left' }}>Proposed Plan</div>
        </div>
        <span className="approval-badge-chip plan">
          Plan
        </span>
      </div>

      <div className="approval-content-box plan-review-content" style={{ maxHeight: '420px', overflowY: 'auto' }}>
        <FormattedMessage text={plan.trim()} />
      </div>
    </div>
  )
})

export default PlanCard
