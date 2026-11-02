import { describe, expect, it } from 'vitest'
import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { PlanCard } from './PlanCard'
import { extractToolCardPayloads } from './ToolResultCards'
import type { AgentProgress } from '../../types'

describe('PlanCard and ToolResultCards plan extraction', () => {
  it('extracts planData from exit_plan_mode ToolEnd event', () => {
    const progress: AgentProgress[] = [
      {
        type: 'ToolEnd',
        data: {
          action: 'exit_plan_mode',
          input: { plan: '## Implementation Plan\n1. Step one\n2. Step two' },
          result: 'Plan approved by user',
        },
      },
    ]

    const payloads = extractToolCardPayloads(progress, 'Proceeding with the implementation.')
    expect(payloads.planData).toBe('## Implementation Plan\n1. Step one\n2. Step two')
  })

  it('renders PlanCard with Proposed Plan title, Plan badge, and markdown content', () => {
    const sample = '## Objectives\n- [ ] Task 1: Initialize\n- [x] Task 2: Complete'
    const html = renderToStaticMarkup(<PlanCard plan={sample} />)

    expect(html).toContain('Proposed Plan')
    expect(html).toContain('approval-badge-chip plan')
    expect(html).toContain('Plan')
    expect(html).toContain('Objectives')
    expect(html).toContain('Task 1: Initialize')
    expect(html).toContain('Task 2: Complete')
  })

  it('returns null if plan is empty', () => {
    const html = renderToStaticMarkup(<PlanCard plan="   " />)
    expect(html).toBe('')
  })
})
