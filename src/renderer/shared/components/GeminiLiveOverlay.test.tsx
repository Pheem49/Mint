import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import GeminiLiveOverlay from './GeminiLiveOverlay'

const props = {
  userTranscript: '', assistantTranscript: '', isPaused: false,
  onTogglePause() {}, onEndCall() {}, voice: 'Puck', voices: ['Puck', 'Kore'], onChangeVoice() {},
}

describe('Live conversation', () => {
  it('shows connection progress before a microphone session exists', () => {
    const html = renderToStaticMarkup(<GeminiLiveOverlay {...props} status={'connecting' as any} />)
    expect(html).toContain('Connecting')
    expect(html).not.toContain('Say something to get started')
  })
  it('exposes named call controls and spoken conversation separately from the decorative orb', () => {
    const html = renderToStaticMarkup(<GeminiLiveOverlay {...props} status="listening" userTranscript="สวัสดี" assistantTranscript="Hello" />)
    expect(html).toContain('aria-label="Pause microphone"')
    expect(html).toContain('aria-label="End Live conversation"')
    expect(html).toContain('สวัสดี')
    expect(html).toContain('Hello')
  })
})
