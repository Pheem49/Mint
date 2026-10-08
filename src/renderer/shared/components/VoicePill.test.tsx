import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import VoicePill from './VoicePill'

describe('VoicePill status', () => {
  it('exposes the actual recording state to assistive technology', () => {
    const html = renderToStaticMarkup(<VoicePill active ariaLabel="Voice input" />)
    expect(html).toContain('aria-pressed="true"')
    expect(html).toContain('data-state="listening"')
    expect(html).toContain('aria-label="Voice input"')
    expect(html).toContain('voice-pill__wave')
    expect(html).toContain('0:00')
  })
  it('renders a disabled idle microphone without a false recording state', () => {
    const html = renderToStaticMarkup(<VoicePill disabled showTime={false} waveform={false} />)
    expect(html).toContain('disabled=""')
    expect(html).toContain('aria-pressed="false"')
    expect(html).not.toContain('<canvas')
  })
})
