import { memo } from 'react'
import type { AgentProgress, ExecutionEvidence } from '../types'

const labels = { verified: 'Verified', unverified: 'Unverified', blocked: 'Blocked', failed: 'Failed' } as const

export default memo(function ExecutionResults({ progress }: { progress: AgentProgress[] }) {
  const executions = new Map<string, ExecutionEvidence>()
  let outcome = ''
  for (const event of progress) {
    if (event.type === 'ExecutionUpdate') {
      for (const entry of event.data.executions) executions.set(`${entry.callId}:${entry.command}`, entry)
    } else if (event.type === 'RunCompleted') outcome = event.data.summary.outcome.toLowerCase()
  }
  const label = labels[outcome as keyof typeof labels]
  if (!executions.size && !label) return null
  return <section className="mcp-results" aria-label="Execution results">
    {label && <p role="status" data-completion-status={outcome}>{label}</p>}
    {[...executions].map(([id, entry]) => <details key={id} className="mcp-call-status">
      <summary>
        {entry.check ? 'Check' : 'Command'} · <span className={`execution-state execution-state-${entry.state.toLowerCase()}`}>{entry.state}</span>
        {entry.exitCode != null ? <span className={`execution-exit${entry.exitCode !== 0 ? ' execution-exit-error' : ''}`}> · exit {entry.exitCode}</span> : ''}
        {entry.conditionVerified === false ? ' · result unverified' : ''}
      </summary>
      <code>{entry.command}</code>
      {entry.jobId && <span>Job: {entry.jobId}</span>}
    </details>)}
  </section>
})
