import { describe, it, expect } from 'vitest'
import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { ReleaseNotesCard } from './ReleaseNotesCard'
import { renderFormattedMessage } from '../../utils/markdown'

describe('ReleaseNotesCard', () => {
  it('renders release notes with framed card, version badge, and sections', () => {
    const sample = `# Release Notes - Mint Agent v1.18.0\n\n## Smart Thinking\n- **Safe Non-Text Filter**: Excludes media\n- **Unrestricted**: All models`
    const html = renderToStaticMarkup(<ReleaseNotesCard markdown={sample} />)

    expect(html).toContain('release-notes-card')
    expect(html).toContain('v1.18.0')
    expect(html).toContain('Release Notes')
    expect(html).toContain('Smart Thinking')
    expect(html).toContain('Safe Non-Text Filter')
    expect(html).toContain('Excludes media')
  })

  it('renders through renderFormattedMessage seamlessly', () => {
    const sample = `# Release Notes - Mint Agent v1.18.0\n\n## UI Ergonomics\n- **Code Blocks**: Clean copying`
    const node = renderFormattedMessage(sample)
    const html = renderToStaticMarkup(<>{node}</>)

    expect(html).toContain('release-notes-card')
    expect(html).toContain('UI Ergonomics')
    expect(html).toContain('Clean copying')
  })
})
