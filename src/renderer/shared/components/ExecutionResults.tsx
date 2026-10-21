import { memo } from 'react'
import type { AgentProgress } from '../types'

/**
 * ExecutionResults previously rendered verification evidence at the bottom of the chat bubble.
 * Tool executions are now natively integrated in chronological order in AgentActivityTable.
 */
export default memo(function ExecutionResults(_props: { progress: AgentProgress[] }) {
  return null
})
