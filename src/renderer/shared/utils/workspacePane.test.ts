import { describe, expect, it } from 'vitest'
import { workspacePaneLayout, workspacePaneKeyWidth, readWorkspacePaneWidth } from './workspacePane'

describe('Workspace pane sizing', () => {
  it('enforces the minimum and maximum on a wide window', () => {
    expect(workspacePaneLayout(100, 1600).width).toBe(280)
    expect(workspacePaneLayout(1000, 1600).width).toBe(720)
  })
  it('reserves space for the conversation and restores the preferred width after shrinking', () => {
    expect(workspacePaneLayout(650, 1000)).toMatchObject({ width: 522, max: 522, stacked: false })
    expect(workspacePaneLayout(650, 1500).width).toBe(650)
  })
  it('stacks the panes when their minimum widths cannot fit', () => {
    expect(workspacePaneLayout(400, 757).stacked).toBe(true)
    expect(workspacePaneLayout(400, 758).stacked).toBe(false)
  })
  it('validates persisted sizes and tolerates unavailable storage', () => {
    for (const value of [null, '', 'oops', 'Infinity']) expect(readWorkspacePaneWidth(() => value)).toBe(400)
    expect(readWorkspacePaneWidth(() => '560')).toBe(560)
    expect(readWorkspacePaneWidth(() => '9999')).toBe(720)
    expect(readWorkspacePaneWidth(() => { throw new Error('blocked') })).toBe(400)
  })
  it('supports keyboard resizing, bounded Home/End and default reset', () => {
    const layout = workspacePaneLayout(400, 1000)
    expect(workspacePaneKeyWidth('ArrowLeft', layout)).toBe(376)
    expect(workspacePaneKeyWidth('ArrowRight', layout)).toBe(424)
    expect(workspacePaneKeyWidth('Home', layout)).toBe(280)
    expect(workspacePaneKeyWidth('End', layout)).toBe(522)
    expect(workspacePaneKeyWidth('Enter', layout)).toBe(400)
    expect(workspacePaneKeyWidth('Tab', layout)).toBeNull()
    expect(workspacePaneKeyWidth('ArrowRight', workspacePaneLayout(720, 1600))).toBe(720)
  })
})
