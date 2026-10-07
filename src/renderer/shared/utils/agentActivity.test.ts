import { describe, expect, it } from 'vitest'
import { describeTool, activitiesFrom } from './agentActivity'

describe('skill reading activity', () => {
  const input = { path: '/repo/.agents/skills/review/SKILL.md', skill_name: 'review' }
  it('shows a reading label while the tool runs', () => {
    expect(describeTool('read_file', input).label).toBe('Reading skill: review')
  })
  it('shows loaded only after a complete successful skill read', () => {
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input } },
      { type: 'ToolEnd', data: { action: 'read_file', input, result: '[Loaded skill: review]\nInstructions' } },
    ])
    expect(view.items[0].label).toBe('Loaded skill: review')
  })
  it('does not claim a failed skill read was loaded', () => {
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input } },
      { type: 'ToolEnd', data: { action: 'read_file', input, result: 'Error: denied' } },
    ])
    expect(view.items[0].state).toBe('error')
    expect(view.items[0].label).not.toContain('Loaded skill:')
  })
  it('matches parallel completions to the skill that finished', () => {
    const second = { path: '/repo/skills/test/SKILL.md', skill_name: 'test' }
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input } },
      { type: 'ToolStart', data: { action: 'read_file', input: second } },
      { type: 'ToolEnd', data: { action: 'read_file', input, result: '[Loaded skill: review]\nInstructions' } },
    ])
    expect(view.items[0].label).toBe('Loaded skill: review')
    expect(view.items[1].label).toBe('Reading skill: test')
    expect(view.timeline?.filter(item => item.kind === 'activity')).toHaveLength(2)
  })
  it('does not mark a line-range preview as a full skill load', () => {
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input } },
      { type: 'ToolEnd', data: { action: 'read_file', input, result: '1 | Header only' } },
    ])
    expect(view.items[0].label).toBe('Read skill preview: review')
  })
})
