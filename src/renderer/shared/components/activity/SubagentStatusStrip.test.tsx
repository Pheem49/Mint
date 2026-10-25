import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, it, expect } from 'vitest'
import { SubagentStatusStrip } from './SubagentStatusStrip'
import type { AgentProgress } from '../../types'

describe('SubagentStatusStrip', () => {
  it('renders nothing when there are no subagents', () => {
    const html = renderToStaticMarkup(createElement(SubagentStatusStrip, { progress: [] }))
    expect(html).toBe('')
  })

  it('renders running and completed subagent cards', () => {
    const progress: AgentProgress[] = [
      {
        type: 'SubagentStarted',
        data: {
          subagent: 'recon',
          profile: 'explorer',
          task: 'Searching for payment webhook endpoints...',
        },
      },
      {
        type: 'SubagentStarted',
        data: {
          subagent: 'config',
          profile: 'explorer',
          task: 'Checking env files',
        },
      },
      {
        type: 'SubagentFinished',
        data: {
          subagent: 'config',
          status: 'completed',
          elapsed_ms: 3000,
          steps: 2,
          summary_preview: 'Checked stripe API keys in .env',
        },
      },
    ]

    const html = renderToStaticMarkup(createElement(SubagentStatusStrip, { progress }))
    expect(html).toContain('subagent-status-strip')
    expect(html).toContain('Running Subagents (1/2 completed)')
    expect(html).toContain('[recon]')
    expect(html).toContain('Searching for payment webhook endpoints...')
    expect(html).toContain('[config]')
    expect(html).toContain('Checked stripe API keys in .env')
    expect(html).toContain('done 3s')
  })

  it('renders failed subagents with failed status and count', () => {
    const progress: AgentProgress[] = [
      {
        type: 'SubagentStarted',
        data: {
          subagent: 'debugger',
          profile: 'explorer',
          task: 'Running tests',
        },
      },
      {
        type: 'SubagentFinished',
        data: {
          subagent: 'debugger',
          status: 'failed',
          elapsed_ms: 1500,
          steps: 1,
          summary_preview: 'Tests failed with panic',
        },
      },
    ]

    const html = renderToStaticMarkup(createElement(SubagentStatusStrip, { progress }))
    expect(html).toContain('subagent-status-strip')
    expect(html).toContain('Subagents (0 completed, 1 failed)')
    expect(html).toContain('[debugger]')
    expect(html).toContain('Tests failed with panic')
    expect(html).toContain('failed 2s')
  })
})
