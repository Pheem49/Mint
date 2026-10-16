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
  onOpenMods: noop, onOpenBackgroundTasks: noop, onClosePanel: noop,
  onModsPanelTarget: noop, onBackgroundPanelTarget: noop,
}

describe('Tools launcher', () => {
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
})
