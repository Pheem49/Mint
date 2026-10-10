import { memo, useCallback } from 'react'
import type { AgentProgress } from '../types'
import { activitiesFrom } from '../utils/agentActivity'
import { AgentActivityDrawer } from './AgentActivityDrawer'

export const ProgressActivity = memo(function ProgressActivity({ progress, id, isOpen, onToggle, historical = false, pendingApproval = false }: {
  progress: AgentProgress[]
  id: string
  isOpen: boolean
  onToggle: (id: string) => void
  historical?: boolean
  pendingApproval?: boolean
}) {
  const toggle = useCallback(() => onToggle(id), [id, onToggle])
  return <AgentActivityDrawer activityView={activitiesFrom(progress)} rawProgress={progress}
    isOpen={isOpen} onToggle={toggle} isHistorical={historical} pendingApproval={pendingApproval} />
})
