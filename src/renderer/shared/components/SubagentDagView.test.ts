import { describe, it, expect } from 'vitest'
import { parseSubagentGraph } from './SubagentDagView'
import type { AgentProgress } from '../types'

describe('parseSubagentGraph', () => {
  it('parses modern swarm lifecycle events (SubagentStarted and SubagentFinished)', () => {
    const progress: AgentProgress[] = [
      {
        type: 'SubagentStarted',
        data: {
          subagent: 'stripe-webhook-tracer',
          profile: 'explorer',
          task: 'Trace webhook verification flow in routes/webhook.rs',
        },
      },
      {
        type: 'ToolStart',
        data: {
          action: 'read_file',
          input: { path: 'src/webhook.rs' },
          subagent: 'stripe-webhook-tracer',
        },
      },
      {
        type: 'ToolEnd',
        data: {
          action: 'read_file',
          input: { path: 'src/webhook.rs' },
          result: 'pub fn verify() {}',
          subagent: 'stripe-webhook-tracer',
        },
      },
      {
        type: 'SubagentFinished',
        data: {
          subagent: 'stripe-webhook-tracer',
          status: 'completed',
          elapsed_ms: 1200,
          steps: 2,
          summary_preview: 'Found verify function in webhook.rs',
        },
      },
    ]

    const graph = parseSubagentGraph(progress)
    expect(graph.subagents).toHaveLength(1)
    const node = graph.subagents[0]
    expect(node.name).toBe('stripe-webhook-tracer')
    expect(node.status).toBe('completed')
    expect(node.tools).toHaveLength(1)
    expect(node.tools[0].action).toBe('read_file')
  })

  it('maintains backward compatibility with legacy dispatch_subagent events', () => {
    const legacyProgress: AgentProgress[] = [
      {
        type: 'ToolStart',
        data: {
          action: 'dispatch_subagent',
          input: { name: 'explorer', instruction: 'Search for config files' },
        },
      },
      {
        type: 'ToolStart',
        data: {
          action: 'list_files',
          input: { path: '.' },
          subagent: 'explorer',
        },
      },
      {
        type: 'ToolEnd',
        data: {
          action: 'list_files',
          input: { path: '.' },
          result: 'Cargo.toml\nREADME.md',
          subagent: 'explorer',
        },
      },
      {
        type: 'ToolEnd',
        data: {
          action: 'dispatch_subagent',
          input: { name: 'explorer', instruction: 'Search for config files' },
          result: '[Subagent explorer result]\nFound Cargo.toml',
        },
      },
    ]

    const graph = parseSubagentGraph(legacyProgress)
    expect(graph.subagents).toHaveLength(1)
    const node = graph.subagents[0]
    expect(node.name).toBe('explorer')
    expect(node.status).toBe('completed')
    expect(node.tools).toHaveLength(1)
  })

  it('marks subagent as failed on error', () => {
    const progress: AgentProgress[] = [
      {
        type: 'SubagentStarted',
        data: {
          subagent: 'debugger',
          profile: 'debugger',
          task: 'Run failing test',
        },
      },
      {
        type: 'SubagentFinished',
        data: {
          subagent: 'debugger',
          status: 'failed',
          elapsed_ms: 500,
          steps: 1,
          summary_preview: 'Command exited with 101',
        },
      },
    ]

    const graph = parseSubagentGraph(progress)
    expect(graph.subagents[0].status).toBe('failed')
  })
})
