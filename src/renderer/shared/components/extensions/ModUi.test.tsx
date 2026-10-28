import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, it, expect } from 'vitest'
import { ModUi, ModSessionControls, ModTestResults, modsPollIntervalMs } from './ModsPanel'
import type { ModNode } from '../../mods'
import { modViewport } from '../../utils/modViewport'

describe('Mod UI', () => {
  it('reports available band pixels and an estimated text width on both graphical surfaces', () => {
    for (const surface of ['web','desktop'] as const) expect(modViewport(664,40,surface,true)).toEqual({surface,widthPx:640,heightPx:40,columns:80,rows:2,sidePanelOpen:true})
    expect(modViewport(0,40,'web',false)).toBeNull()
  })
  it('shows session reload state and failing test diagnostics', () => {
    const controls=renderToStaticMarkup(<ModSessionControls enabled busy={false} onToggle={() => {}} />)
    expect(controls).toContain('aria-pressed="true"')
    expect(controls).toContain('Hot reload')
    const report=renderToStaticMarkup(<ModTestResults report={{ passed: 1, failed: 1, tests: [
      { file: 'weather.test.ts', name: 'updates', passed: true },
      { file: 'weather.test.ts', name: 'storm', passed: false, error: 'Expected Storm' },
    ] }} />)
    expect(report).toContain('1 passed')
    expect(report).toContain('storm')
    expect(report).toContain('Expected Storm')
  })
  it('inherits stack colors into controls and allows a child to restore default text', () => {
    const html = renderToStaticMarkup(<ModUi node={{ type: 'stack', color: 'cyan', children: [
      { type: 'button', label: 'Inspect', action: 'inspect' },
      { type: 'badge', text: 'Measured' },
      { type: 'text', text: 'Normal', color: 'default' },
    ] }} onAction={() => {}} />)
    expect(html).toContain('<button class="mod-color-cyan"')
    expect(html).toContain('mod-badge mod-color-cyan')
    expect(html).toContain('mod-text mod-color-default')
  })
  it('renders a weather row with a colored label and uncolored metrics', () => {
    const html = renderToStaticMarkup(<ModUi node={{ type: 'stack', direction: 'row', children: [
      { type: 'text', text: '☂ Showers', color: 'blue' },
      { type: 'text', text: '67.2% · 134.4k / 200k · ▁▂▃▄▅▆▇█ · ▲ +98.3k last turn' },
    ] } as ModNode} onAction={() => {}} />)
    expect(html).toContain('mod-stack-row')
    expect(html).toContain('mod-color-blue')
    expect(html).toContain('☂ Showers')
    expect(html).toContain('134.4k / 200k')
  })
  it('renders text literally and exposes a labeled control', () => {
    const html = renderToStaticMarkup(<ModUi node={{ type: 'stack', children: [
      { type: 'text', text: '<script>literal</script>' },
      { type: 'button', label: 'Add task', action: 'add' },
    ] }} onAction={() => {}} />)
    expect(html).toContain('&lt;script&gt;literal&lt;/script&gt;')
    expect(html).toContain('Add task')
    expect(html).not.toContain('<script>')
  })
  it('renders accessible progress and tabular data', () => {
    const html = renderToStaticMarkup(<ModUi node={{ type: 'stack', children: [
      { type: 'progress', label: 'Budget', value: 20, max: 100 },
      { type: 'table', columns: ['Task'], rows: [['Write tests']] },
    ] }} onAction={() => {}} />)
    expect(html).toContain('aria-label="Budget"')
    expect(html).toContain('value="20"')
    expect(html).toContain('<th>Task</th>')
    expect(html).toContain('<td>Write tests</td>')
  })
  it('backs off Mods polling when panel is closed and no mods are enabled', () => {
    expect(modsPollIntervalMs(false, [])).toBe(12000)
    expect(modsPollIntervalMs(false, [{ id: 'm1', enabled: false } as any])).toBe(12000)
    expect(modsPollIntervalMs(true, [])).toBe(3000)
    expect(modsPollIntervalMs(false, [{ id: 'm1', enabled: true } as any])).toBe(3000)
  })
})
