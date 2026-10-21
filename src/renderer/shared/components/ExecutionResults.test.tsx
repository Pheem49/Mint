import { describe, expect, it } from 'vitest'
// @ts-ignore react-dom/server lacks types without @types/react-dom
import { renderToStaticMarkup } from 'react-dom/server'
import ExecutionResults from './ExecutionResults'
import type { AgentProgress } from '../types'

describe('ExecutionResults component', () => {
  it('returns null and does not render duplicate bottom execution blocks', () => {
    const progress: AgentProgress[] = [
      {
        type: 'ExecutionUpdate',
        data: {
          callId: 'call-1',
          executions: [
            {
              callId: 'call-1',
              command: 'convert 09_edited_milky50.jpg -resize 900x out.jpg',
              state: 'succeeded',
              exitCode: 0,
              revision: 1,
              check: false,
              conditionVerified: null,
            },
          ],
        },
      },
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
    expect(html).toBe('')
  })
})
