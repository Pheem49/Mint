import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { activeCompactionFrom } from './contextCompaction'
import { ContextCompactionStatus } from '../components/ContextCompactionStatus'
import { activitiesFrom } from './agentActivity'
import type { AgentProgress } from '../types'

const started = { type: 'ContextCompaction', data: { status: 'started', message: 'Summarizing earlier steps to continue working' } } as const
const completed = { type: 'ContextCompaction', data: { status: 'completed', message: 'Context compacted' } } as const
const failed = { type: 'ContextCompaction', data: { status: 'failed', message: 'Context compaction failed: provider unavailable. Continuing with the original context.' } } as const

describe('context compaction progress', () => {
  it('keeps compaction active until its terminal event, then allows normal working status', () => {
    expect(activeCompactionFrom([started, { type: 'Thinking', data: { elapsed_secs: 10 } }])?.message).toContain('Summarizing earlier steps')
    expect(activeCompactionFrom([started, completed])).toBeNull()
    expect(activeCompactionFrom([started, failed])).toBeNull()
    expect(activeCompactionFrom([started, completed, started])).not.toBeNull()
  })
  it('renders the elapsed timer, explanatory text and animated gradient label', () => {
    const html = renderToStaticMarkup(<ContextCompactionStatus elapsedSeconds={12} />)
    expect(html).toContain('Compacting context')
    expect(html).toContain('12s')
    expect(html).toContain('Summarizing earlier steps to continue working')
    expect(html).toContain('thinking-status-label')
    expect(html).toContain('role="status"')
  })
  it('keeps completion and actionable failure visible in activity history, without a completed start row', () => {
    const timeline = activitiesFrom([started, completed, started, failed]).timeline
    expect(timeline?.map(item => 'thought' in item ? item.thought : '')).toEqual([completed.data.message, failed.data.message])
  })
})

it('keeps the collective timer active until all parallel subagents finish', () => {
  const a = { ...started, data: { ...started.data, subagent: 'A' } }
  const b = { ...started, data: { ...started.data, subagent: 'B' } }
  const doneA = { ...completed, data: { ...completed.data, subagent: 'A' } }
  const doneB = { ...completed, data: { ...completed.data, subagent: 'B' } }
  expect(activeCompactionFrom([a, b, doneA])?.index).toBe(0)
  expect(activeCompactionFrom([a, b, doneA, doneB])).toBeNull()
})

it('keeps repeated-name subagents active until every lifecycle finishes', () => {
  const child = { ...started, data: { ...started.data, subagent: 'reviewer' } }
  const done = { ...completed, data: { ...completed.data, subagent: 'reviewer' } }
  expect(activeCompactionFrom([child, child, done])?.index).toBe(0)
  expect(activeCompactionFrom([child, child, done, done])).toBeNull()
})
