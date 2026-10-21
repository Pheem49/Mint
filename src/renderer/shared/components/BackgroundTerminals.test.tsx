import { describe, expect, it } from 'vitest'
import { filterScopedJobs } from './BackgroundTerminals'
import type { BackgroundJob } from '../types'

describe('BackgroundTerminals scoping and polling optimization', () => {
  const sampleJobs: BackgroundJob[] = [
    {
      id: 'job-1',
      command: 'npm run dev',
      workspacePath: '/home/user/project-a',
      status: 'running',
      createdAt: '2026-10-21T00:00:00Z',
      exitCode: null,
    },
    {
      id: 'job-2',
      command: 'cargo build',
      workspacePath: '/home/user/project-b',
      status: 'completed',
      createdAt: '2026-10-21T00:00:00Z',
      exitCode: 0,
    },
    {
      id: 'job-3',
      command: 'vite preview',
      workspacePath: '/home/user/project-a',
      status: 'running',
      createdAt: '2026-10-21T00:00:00Z',
      exitCode: null,
    },
  ]

  it('filters scoped jobs in memory without needing a second API request', () => {
    const scopedA = filterScopedJobs(sampleJobs, '/home/user/project-a')
    expect(scopedA.map(j => j.id)).toEqual(['job-1', 'job-3'])

    const scopedB = filterScopedJobs(sampleJobs, '/home/user/project-b')
    expect(scopedB.map(j => j.id)).toEqual(['job-2'])
  })

  it('returns empty array when workspacePath is null or undefined', () => {
    expect(filterScopedJobs(sampleJobs, null)).toEqual([])
    expect(filterScopedJobs(sampleJobs, undefined)).toEqual([])
    expect(filterScopedJobs(sampleJobs, '')).toEqual([])
  })
})
