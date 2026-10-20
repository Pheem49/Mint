import { describe, expect, it } from 'vitest'
// @ts-ignore react-dom/server lacks types without @types/react-dom
import { renderToStaticMarkup } from 'react-dom/server'
import ExecutionResults from './ExecutionResults'
import type { AgentProgress } from '../types'

describe('ExecutionResults component', () => {
  it('returns null when there are no executions and no outcome label', () => {
    const html = renderToStaticMarkup(<ExecutionResults progress={[]} />)
    expect(html).toBe('')
  })

  it('renders verified outcome header when outcome is VERIFIED without executions', () => {
    const progress: AgentProgress[] = [
      {
        type: 'RunCompleted',
        data: {
          summary: {
            runId: 'r1',
            task: 'test',
            outcome: 'VERIFIED',
            totalTokens: 10,
            toolCallsCount: 1,
            filesChanged: [],
            durationSecs: 1.5,
            toolTimeline: [],
            retriesCount: 0,
          },
        },
      },
    ]
    const html = renderToStaticMarkup(<ExecutionResults progress={progress} />)
    expect(html).toContain('data-completion-status="verified"')
    expect(html).toContain('Verified')
    expect(html).toContain('Verified Evidence')
  })

  it('renders unverified outcome header with correct pill status', () => {
    const progress: AgentProgress[] = [
      {
        type: 'RunCompleted',
        data: {
          summary: {
            runId: 'r2',
            task: 'test',
            outcome: 'UNVERIFIED',
            totalTokens: 10,
            toolCallsCount: 1,
            filesChanged: [],
            durationSecs: 1.5,
            toolTimeline: [],
            retriesCount: 0,
          },
        },
      },
    ]
    const html = renderToStaticMarkup(<ExecutionResults progress={progress} />)
    expect(html).toContain('data-completion-status="unverified"')
    expect(html).toContain('Unverified')
  })

  it('renders execution items with commands, checks, states, and exit codes', () => {
    const progress: AgentProgress[] = [
      {
        type: 'ExecutionUpdate',
        data: {
          callId: 'call-1',
          executions: [
            {
              callId: 'call-1',
              command: 'mogrify -resize 50% test.png',
              state: 'succeeded',
              exitCode: 0,
              revision: 1,
              check: false,
              conditionVerified: null,
            },
            {
              callId: 'call-2',
              command: 'test -f test.png',
              state: 'succeeded',
              exitCode: 0,
              revision: 1,
              check: true,
              conditionVerified: true,
            },
            {
              callId: 'call-3',
              command: 'cargo check',
              state: 'failed',
              exitCode: 1,
              revision: 1,
              check: true,
              conditionVerified: false,
            },
          ],
        },
      },
      {
        type: 'RunCompleted',
        data: {
          summary: {
            runId: 'r3',
            task: 'test',
            outcome: 'VERIFIED',
            totalTokens: 50,
            toolCallsCount: 3,
            filesChanged: [],
            durationSecs: 2.1,
            toolTimeline: [],
            retriesCount: 0,
          },
        },
      },
    ]

    const html = renderToStaticMarkup(<ExecutionResults progress={progress} />)
    expect(html).toContain('data-completion-status="verified"')
    expect(html).toContain('Verified Evidence')
    // Command & Check tags
    expect(html).toContain('Command')
    expect(html).toContain('Check')
    expect(html).toContain('mogrify -resize 50% test.png')
    expect(html).toContain('test -f test.png')
    expect(html).toContain('cargo check')
    // States
    expect(html).toContain('execution-state-succeeded')
    expect(html).toContain('execution-state-failed')
    // Exit codes
    expect(html).toContain('exit 0')
    expect(html).toContain('exit 1')
    expect(html).toContain('execution-exit-error')
    // Unverified warning
    expect(html).toContain('result unverified')
    // Summary
    expect(html).toContain('1 failed · 2 succeeded')
  })
})
