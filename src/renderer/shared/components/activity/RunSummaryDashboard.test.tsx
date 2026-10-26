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

  it('renders Completed badge with success class for PARTIAL outcome', () => {
    const summary: RunTelemetrySummary = {
      runId: 'r3',
      task: 'casual chat',
      outcome: 'PARTIAL',
      durationSecs: 2.1,
      totalTokens: 1200,
      toolCallsCount: 0,
      retriesCount: 0,
      filesChanged: [],
      filesCreated: [],
      toolTimeline: [],
    }
    const html = renderToStaticMarkup(<RunSummaryDashboard summary={summary} />)
    expect(html).toContain('run-summary-badge success')
    expect(html).toContain('Completed')
    expect(html).not.toContain('Failure')
  })

  it('renders Verified badge with success class for VERIFIED outcome', () => {
    const summary: RunTelemetrySummary = {
      runId: 'r4',
      task: 'verified task',
      outcome: 'VERIFIED',
      durationSecs: 15.0,
      totalTokens: 5000,
      toolCallsCount: 2,
      retriesCount: 0,
      filesChanged: [],
      filesCreated: [],
      toolTimeline: [],
    }
    const html = renderToStaticMarkup(<RunSummaryDashboard summary={summary} />)
    expect(html).toContain('run-summary-badge success')
    expect(html).toContain('Verified')
  })

  it('renders Failure badge with failure class for FAILED outcome', () => {
    const summary: RunTelemetrySummary = {
      runId: 'r5',
      task: 'failed task',
      outcome: 'FAILED',
      durationSecs: 5.0,
      totalTokens: 2000,
      toolCallsCount: 1,
      retriesCount: 0,
      filesChanged: [],
      filesCreated: [],
      toolTimeline: [],
    }
    const html = renderToStaticMarkup(<RunSummaryDashboard summary={summary} />)
    expect(html).toContain('run-summary-badge failure')
    expect(html).toContain('Failure')
  })

  it('renders Export Trajectory action button in header', () => {
    const summary: RunTelemetrySummary = {
      runId: 'run-traj-42',
      task: 'export trajectory test',
      outcome: 'SUCCESS',
      durationSecs: 3.2,
      totalTokens: 1500,
      toolCallsCount: 2,
      retriesCount: 0,
      filesChanged: ['src/app.ts'],
      filesCreated: [],
      toolTimeline: [],
    }
    const html = renderToStaticMarkup(<RunSummaryDashboard summary={summary} />)
    expect(html).toContain('run-summary-export-btn')
    expect(html).toContain('Export Trajectory')
    expect(html).toContain('title="Export SWE-bench / Agent Protocol compliant trajectory JSON"')
  })
})
