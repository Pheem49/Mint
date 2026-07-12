// Re-export everything from the shared module — do NOT add logic here.
// Edit src/renderer/shared/agentProgress.ts instead.
export type { AgentProgress, InteractionMemory } from '../shared/agentProgress'
export {
  thoughtsFrom,
  trimAgentProgress,
  hasAgentToolActivity,
  mergeActivitySnapshots,
  parseAgentActivity,
  parseFileChangesFromProgress,
} from '../shared/agentProgress'
