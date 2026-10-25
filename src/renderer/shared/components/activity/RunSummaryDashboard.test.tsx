import { describe, it, expect } from 'vitest'
// @ts-ignore react-dom/server lacks types without @types/react-dom
import { renderToStaticMarkup } from 'react-dom/server'
import React from 'react'
import { RunSummaryDashboard, formatRunDuration } from './RunSummaryDashboard'
import type { RunTelemetrySummary } from '../../types'

describe('RunSummaryDashboard and formatRunDuration', () => {
  it('formats duration correctly under 60 seconds', () => {
    expect(formatRunDuration(14.5)).toEqual({ value: '14.5s' })
    expect(formatRunDuration(0.8)).toEqual({ value: '0.8s' })
  })

  it('formats duration into minutes and seconds when >= 60 seconds', () => {
    const formatted = formatRunDuration(81.8)
    expect(formatted.value).toBe('1m 21.8s')
    expect(formatted.subtext).toBe('81.8s')

    const formatted60 = formatRunDuration(60.0)
    expect(formatted60.value).toBe('1m 00.0s')
    expect(formatted60.subtext).toBe('60.0s')
  })

  it('formats duration with hours when >= 3600 seconds', () => {
    const formatted = formatRunDuration(3665)
    expect(formatted.value).toBe('1h 1m')
    expect(formatted.subtext).toBe('3665.0s')
  })

  it('renders Duration with minute breakdown and raw seconds in subtext', () => {
    const mockSummary: RunTelemetrySummary = {
      runId: 'test-run-123',
      task: 'test-task',
      outcome: 'SUCCESS',
      durationSecs: 81.8,
      totalTokens: 12500,
      toolCallsCount: 5,
      retriesCount: 1,
      filesChanged: ['a.txt', 'b.txt'],
      filesCreated: ['b.txt'],
      toolTimeline: [],
    }

    const html = renderToStaticMarkup(<RunSummaryDashboard summary={mockSummary} />)

    expect(html).toContain('Run summary')
    expect(html).toContain('1m 21.8s')
    expect(html).toContain('· 81.8s')
    expect(html).toContain('12.5k')
    expect(html).toContain('Success')
    expect(html).not.toContain('Tool call timeline')
  })
})
