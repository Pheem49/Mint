import React, { useMemo, useState } from 'react'
import type { AgentProgress } from '../types'

interface Props {
  progress: AgentProgress[]
  isLive?: boolean
}

interface SubagentNode {
  name: string
  taskSnippet?: string
  status: 'running' | 'completed' | 'failed' | 'idle'
  currentTool?: string
  tools: Array<{
    action: string
    input?: any
    result?: string
    isError?: boolean
  }>
}

export function parseSubagentGraph(progress: AgentProgress[]): {
  orchestratorStatus: 'running' | 'completed' | 'idle'
  subagents: SubagentNode[]
} {
  const subagentMap = new Map<string, SubagentNode>()
  let isWorking = false

  for (const event of progress || []) {
    if (event.type === 'Thinking') {
      isWorking = true
    } else if (event.type === 'ToolStart') {
      isWorking = true
      const action = event.data.action
      const subagentName = event.data.subagent

      // Detect dispatch_subagent call from parent
      if (action === 'dispatch_subagent') {
        const targetName = (event.data.input as any)?.name || 'subagent'
        const instruction = (event.data.input as any)?.instruction || ''
        if (!subagentMap.has(targetName)) {
          subagentMap.set(targetName, {
            name: targetName,
            taskSnippet: instruction.length > 80 ? `${instruction.slice(0, 77)}...` : instruction,
            status: 'running',
            tools: [],
          })
        }
      }

      // Tool call inside a subagent
      if (subagentName) {
        let node = subagentMap.get(subagentName)
        if (!node) {
          node = {
            name: subagentName,
            status: 'running',
            tools: [],
          }
          subagentMap.set(subagentName, node)
        }
        node.status = 'running'
        node.currentTool = action
        node.tools.push({
          action,
          input: event.data.input,
        })
      }
    } else if (event.type === 'ToolEnd') {
      const action = event.data.action
      const subagentName = event.data.subagent
      const result = event.data.result || ''
      const isError = result.startsWith('Error:')

      if (action === 'dispatch_subagent') {
        const targetName = (event.data.input as any)?.name || 'subagent'
        const node = subagentMap.get(targetName)
        if (node) {
          node.status = isError ? 'failed' : 'completed'
          node.currentTool = undefined
        }
      }

      if (subagentName) {
        const node = subagentMap.get(subagentName)
        if (node) {
          const lastTool = node.tools[node.tools.length - 1]
          if (lastTool && lastTool.action === action) {
            lastTool.result = result
            lastTool.isError = isError
          }
          node.currentTool = undefined
        }
      }
    }
  }

  const subagents = Array.from(subagentMap.values())
  // If subagents were created, mark ones with no running tool as completed if turn finished
  for (const sub of subagents) {
    if (!sub.currentTool && sub.status === 'running') {
      sub.status = 'completed'
    }
  }

  return {
    orchestratorStatus: isWorking ? 'running' : 'completed',
    subagents,
  }
}

export function SubagentDagView({ progress, isLive = false }: Props) {
  const [selectedSubagent, setSelectedSubagent] = useState<string | null>(null)

  const graph = useMemo(() => parseSubagentGraph(progress), [progress])
  const activeSubagent = useMemo(() => {
    if (!selectedSubagent) return null
    return graph.subagents.find((s) => s.name === selectedSubagent) || null
  }, [graph.subagents, selectedSubagent])

  if (graph.subagents.length === 0) {
    return (
      <div style={{ padding: '24px', textAlign: 'center', color: 'var(--text-muted, #9ca3af)', fontSize: '0.85rem' }}>
        <p style={{ margin: 0, fontWeight: 500 }}>No subagents dispatched in this turn</p>
        <p style={{ margin: '6px 0 0 0', opacity: 0.7, fontSize: '0.78rem' }}>
          Subagents will appear here in an interactive DAG tree when the Main Agent invokes them via <code>dispatch_subagent</code>.
        </p>
      </div>
    )
  }

  return (
    <div className="subagent-dag-container" style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '16px' }}>
      {/* SVG Canvas / Flow Layout */}
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          gap: '28px',
          position: 'relative',
          padding: '12px',
        }}
      >
        {/* Root Orchestrator Node */}
        <div
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: '10px',
            padding: '10px 18px',
            borderRadius: '12px',
            background: 'linear-gradient(135deg, rgba(16, 185, 129, 0.15), rgba(59, 130, 246, 0.15))',
            border: '1px solid rgba(16, 185, 129, 0.35)',
            boxShadow: '0 4px 16px rgba(0, 0, 0, 0.3)',
            zIndex: 2,
          }}
        >
          <div
            style={{
              width: '32px',
              height: '32px',
              borderRadius: '8px',
              background: '#10b981',
              color: '#ffffff',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: '0.75rem',
              fontWeight: 700,
            }}
          >
            M
          </div>
          <div>
            <div style={{ fontWeight: 600, fontSize: '0.88rem', color: '#f3f4f6' }}>Main Orchestrator</div>
            <div style={{ fontSize: '0.72rem', color: isLive ? '#10b981' : '#9ca3af', display: 'flex', alignItems: 'center', gap: '4px' }}>
              <span
                style={{
                  width: '6px',
                  height: '6px',
                  borderRadius: '50%',
                  background: isLive ? '#10b981' : '#6b7280',
                  display: 'inline-block',
                }}
              />
              {isLive ? 'Active Session' : 'Completed'}
            </div>
          </div>
        </div>

        {/* Subagents Branching Grid */}
        <div
          style={{
            display: 'flex',
            flexWrap: 'wrap',
            justifyContent: 'center',
            gap: '16px',
            width: '100%',
            position: 'relative',
            zIndex: 2,
          }}
        >
          {graph.subagents.map((subagent) => {
            const isSelected = selectedSubagent === subagent.name
            const statusColor =
              subagent.status === 'running'
                ? '#3b82f6'
                : subagent.status === 'failed'
                  ? '#ef4444'
                  : '#10b981'

            return (
              <div
                key={subagent.name}
                onClick={() => setSelectedSubagent(isSelected ? null : subagent.name)}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  width: '260px',
                  padding: '12px',
                  borderRadius: '10px',
                  background: isSelected ? 'rgba(30, 41, 59, 0.95)' : 'rgba(15, 23, 42, 0.8)',
                  border: `1px solid ${isSelected ? statusColor : 'rgba(255, 255, 255, 0.1)'}`,
                  cursor: 'pointer',
                  boxShadow: isSelected ? `0 0 16px ${statusColor}33` : '0 4px 12px rgba(0, 0, 0, 0.2)',
                  transition: 'all 0.2s ease',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span
                      style={{
                        width: '8px',
                        height: '8px',
                        borderRadius: '50%',
                        background: statusColor,
                        display: 'inline-block',
                      }}
                    />
                    <span style={{ fontWeight: 600, fontSize: '0.84rem', color: '#f3f4f6' }}>{subagent.name}</span>
                  </div>
                  <span
                    style={{
                      fontSize: '0.68rem',
                      fontWeight: 600,
                      padding: '2px 6px',
                      borderRadius: '4px',
                      textTransform: 'uppercase',
                      background: `${statusColor}22`,
                      color: statusColor,
                      border: `1px solid ${statusColor}44`,
                    }}
                  >
                    {subagent.status === 'running' && subagent.currentTool
                      ? subagent.currentTool
                      : subagent.status}
                  </span>
                </div>

                {subagent.taskSnippet && (
                  <p style={{ margin: '0 0 8px 0', fontSize: '0.74rem', color: '#94a3b8', lineHeight: 1.4 }}>
                    {subagent.taskSnippet}
                  </p>
                )}

                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    fontSize: '0.72rem',
                    color: '#64748b',
                    paddingTop: '6px',
                    borderTop: '1px solid rgba(255, 255, 255, 0.06)',
                  }}
                >
                  <span>{subagent.tools.length} tool {subagent.tools.length === 1 ? 'call' : 'calls'}</span>
                  <span style={{ color: isSelected ? statusColor : '#94a3b8' }}>
                    {isSelected ? 'Hide Details ▲' : 'View Logs ▼'}
                  </span>
                </div>
              </div>
            )
          })}
        </div>
      </div>

      {/* Selected Subagent Tool Inspection Drawer */}
      {activeSubagent && (
        <div
          style={{
            background: 'rgba(15, 23, 42, 0.95)',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            borderRadius: '10px',
            padding: '14px',
            marginTop: '8px',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <span style={{ fontWeight: 600, fontSize: '0.86rem', color: '#10b981' }}>
                Subagent Activity: {activeSubagent.name}
              </span>
              <span style={{ fontSize: '0.75rem', color: '#94a3b8' }}>({activeSubagent.tools.length} steps)</span>
            </div>
            <button
              type="button"
              onClick={() => setSelectedSubagent(null)}
              style={{
                background: 'transparent',
                border: 'none',
                color: '#94a3b8',
                cursor: 'pointer',
                fontSize: '0.8rem',
              }}
            >
              Close
            </button>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', maxHeight: '300px', overflowY: 'auto' }}>
            {activeSubagent.tools.map((tool, idx) => (
              <div
                key={idx}
                style={{
                  background: 'rgba(0, 0, 0, 0.25)',
                  border: '1px solid rgba(255, 255, 255, 0.05)',
                  borderRadius: '6px',
                  padding: '8px 10px',
                  fontSize: '0.76rem',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '4px' }}>
                  <span style={{ fontWeight: 600, color: '#38bdf8' }}>{tool.action}</span>
                  {tool.isError && <span style={{ color: '#f87171', fontWeight: 600 }}>FAILED</span>}
                </div>
                {tool.input && (
                  <pre
                    style={{
                      margin: '4px 0',
                      padding: '4px 6px',
                      background: '#090d16',
                      borderRadius: '4px',
                      color: '#cbd5e1',
                      fontSize: '0.72rem',
                      overflowX: 'auto',
                    }}
                  >
                    {JSON.stringify(tool.input, null, 2)}
                  </pre>
                )}
                {tool.result && (
                  <div style={{ marginTop: '4px', color: '#94a3b8', fontSize: '0.72rem', maxHeight: '100px', overflowY: 'auto' }}>
                    <strong>Result:</strong> {tool.result.slice(0, 200)}
                    {tool.result.length > 200 ? '...' : ''}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
