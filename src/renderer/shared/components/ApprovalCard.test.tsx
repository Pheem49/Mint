import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { ApprovalCard } from './ApprovalCard'
import { requiresExplicitApproval } from '../utils/approval'

describe('skill approval', () => {
  it('excludes skill proposals from session auto approval while keeping ordinary tool permissions', () => {
    expect(requiresExplicitApproval({ SkillWrite: { path: 'SKILL.md' } })).toBe(true)
    expect(requiresExplicitApproval({ WriteFile: { path: 'app.ts' } })).toBe(false)
    expect(requiresExplicitApproval(undefined)).toBe(false)
  })
  it('shows the proposed skill and diff review without offering session permission', () => {
    const html = renderToStaticMarkup(<ApprovalCard
      pendingApproval={{ approval: { SkillWrite: { path: '.agents/skills/build/SKILL.md', diff: '+Use the build command', content: 'Use the build command' } } }}
      onApproval={() => {}}
    />)
    expect(html).toContain('.agents/skills/build/SKILL.md')
    expect(html).toContain('Review')
    expect(html).toContain('Allow once')
    expect(html).toContain('Reject')
    expect(html).not.toContain('Always allow for this session')
  })
})
