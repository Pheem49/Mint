import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import ToolSurfacePage from './ToolSurfacePage'

const noop = () => {}
const props = {
  surfaces: [], activeSurfaceId: null, terminalSize: 440,
  onSelect: noop, onClose: noop, onCloseAll: noop, onDuplicate: noop,
  onRename: noop, onCloseOthers: noop, onResizeTerminal: noop, onResizePanel: noop,
  onOpenTerminal: noop, onOpenBrowser: noop, onOpenReview: noop, onOpenFiles: noop,
  onOpenSources: noop, onOpenMods: noop, onOpenBackgroundTasks: noop, onClosePanel: noop,
  onModsPanelTarget: noop, onBackgroundPanelTarget: noop,
}

describe('Tools launcher', () => {
  it('offers session Sources with their count on Desktop and Web', () => {
    for (const desktopTools of [true, false]) {
      const html = renderToStaticMarkup(<ToolSurfacePage {...props} desktopTools={desktopTools} sourceCount={3} />)
      expect(html).toMatch(/role="menuitem"[^>]*title="Session sources"/)
      expect(html).toContain('3 session sources')
    }
  })

  it('renders Sources inside its selected tool tab', () => {
    const html = renderToStaticMarkup(<ToolSurfacePage {...props}
      surfaces={[{ id: 'sources', kind: 'sources', title: 'Sources' }]} activeSurfaceId="sources"
      sourcesContent={<section aria-label="Session sources">reference.png</section>} />)
    expect(html).toContain('reference.png')
    expect(html).toContain('aria-label="Session sources"')
    expect(html).not.toContain('Duplicate tab')
  })
  it('makes workspace mods and running background tasks discoverable on its first screen', () => {
    const html = renderToStaticMarkup(<ToolSurfacePage {...props} backgroundTaskCount={2} />)
    expect(html).toMatch(/role="menuitem"[^>]*title="Workspace mods"/)
    expect(html).toMatch(/role="menuitem"[^>]*title="Background tasks"/)
    expect(html).toContain('2 active background tasks')
    expect(html).toContain('Integrated terminal')
  })

  it('does not offer a native shell or desktop workspace on the web', () => {
    const html = renderToStaticMarkup(<ToolSurfacePage {...props} desktopTools={false} />)
    expect(html).toContain('Workspace mods')
    expect(html).toContain('Background tasks')
    expect(html).not.toContain('Integrated terminal')
    expect(html).not.toContain('Workspace files')
  })

  it('renders tabs with hover tooltip and quick switcher badge for multi-tab management', () => {
    const multiProps = {
      ...props,
      surfaces: [
        { id: 'img-1', kind: 'preview' as const, title: 'screenshot_large_diagram.png', artifact: { path: '/tmp/screenshot_large_diagram.png', title: 'screenshot_large_diagram.png' } },
        { id: 'img-2', kind: 'preview' as const, title: 'mockup_v2.png', artifact: { path: '/tmp/mockup_v2.png', title: 'mockup_v2.png' } },
        { id: 'term-1', kind: 'terminal' as const, title: 'Terminal 1' },
      ],
      activeSurfaceId: 'img-1',
    }
    const html = renderToStaticMarkup(<ToolSurfacePage {...multiProps} />)
    // Check tooltip attribute
    expect(html).toContain('title="screenshot_large_diagram.png"')
    expect(html).toContain('aria-label="screenshot_large_diagram.png"')
    // Check quick switcher button and badge
    expect(html).toContain('tool-surface-tabs-switcher-btn')
    expect(html).toContain('All open tabs (3)')
    expect(html).toContain('tool-surface-tabs-badge">3</span>')
  })
})
