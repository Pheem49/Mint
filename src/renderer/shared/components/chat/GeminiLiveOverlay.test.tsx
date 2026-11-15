import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import GeminiLiveOverlay, { resolveLiveOverlayEscape } from './GeminiLiveOverlay'
import {
  applyLiveTranscriptFragment,
  downsampleToPcm16,
  routeGeminiLiveSideEffects,
  stopQueuedAudioPlayback,
} from '../../utils/useGeminiLiveVoice'

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
  it('stops and disconnects all queued audio sources when interrupted', () => {
    let stopped1 = false, disconnected1 = false
    let stopped2 = false, disconnected2 = false
    const sources = new Set<any>([
      { stop() { stopped1 = true }, disconnect() { disconnected1 = true } },
      { stop() { stopped2 = true }, disconnect() { disconnected2 = true } },
    ])
    const nextPlaybackTimeRef = { current: 12.5 }
    const speakingTimeoutRef = { current: null as number | null }
    let speaking = true

    stopQueuedAudioPlayback(sources, nextPlaybackTimeRef, speakingTimeoutRef, (val) => { speaking = val })

    expect(stopped1).toBe(true)
    expect(disconnected1).toBe(true)
    expect(stopped2).toBe(true)
    expect(disconnected2).toBe(true)
    expect(sources.size).toBe(0)
    expect(nextPlaybackTimeRef.current).toBe(0)
    expect(speaking).toBe(false)
  })
  it('renders an inline ApprovalCard inside the Live overlay when a voice tool requests approval', () => {
    const html = renderToStaticMarkup(
      <GeminiLiveOverlay
        {...props}
        status="thinking"
        pendingApproval={{
          token: 'tok-1',
          approval: {
            RunShell: {
              command: 'cargo test --workspace',
              mode: 'workspace-write',
              background: false,
            },
          },
        }}
        onApproval={() => {}}
      />
    )
    expect(html).toContain('cargo test --workspace')
    expect(html).toContain('Allow once')
    expect(html).toContain('Reject')
  })
  it('routes turn persistence, tool activity, and approval requests from Gemini Live events', () => {
    let turnsSaved = 0
    let toolActivities = 0
    const approvals: Array<{ token: string; approval: Record<string, unknown> }> = []
    const callbacks = {
      onTurnSaved: () => { turnsSaved += 1 },
      onToolActivity: () => { toolActivities += 1 },
      onApprovalRequested: (payload: { token: string; approval: Record<string, unknown> }) => {
        approvals.push(payload)
      },
    }

    routeGeminiLiveSideEffects({ type: 'turnComplete' }, callbacks)
    expect(turnsSaved).toBe(1)

    routeGeminiLiveSideEffects({ type: 'interrupted' }, callbacks)
    expect(turnsSaved).toBe(2)

    routeGeminiLiveSideEffects({ type: 'closed' }, callbacks)
    expect(turnsSaved).toBe(3)

    routeGeminiLiveSideEffects(
      { type: 'toolStatus', action: 'read_file', input: { path: 'README.md' }, result: 'ok' },
      callbacks,
    )
    expect(toolActivities).toBe(1)

    routeGeminiLiveSideEffects(
      {
        type: 'approvalRequested',
        token: 'tok-live-99',
        approval: { RunShell: { command: 'ls', mode: 'read-only', background: false } },
      },
      callbacks,
    )
    expect(approvals).toEqual([
      {
        token: 'tok-live-99',
        approval: { RunShell: { command: 'ls', mode: 'read-only', background: false } },
      },
    ])
  })
  it('downsamples mic audio to 16kHz PCM16 with boxcar anti-aliasing instead of nearest-neighbor aliasing', () => {
    // 1. Direct 16kHz -> 16kHz conversion with clamping
    const direct = downsampleToPcm16(new Float32Array([0, 0.5, -0.5, 1.5, -1.5]), 16000, 16000)
    expect(Array.from(direct)).toEqual([0, 16384, -16384, 32767, -32768])

    // 2. 48kHz -> 16kHz (ratio = 3): high-frequency alternating [1.0, -1.0, 0.0] averages to 0
    // (nearest-neighbor would alias to +32767!), while steady [0.5, 0.5, 0.5] averages to 16384.
    const antiAliased = downsampleToPcm16(
      new Float32Array([1.0, -1.0, 0.0, 0.5, 0.5, 0.5]),
      48000,
      16000,
    )
    expect(Array.from(antiAliased)).toEqual([0, 16384])

    // 3. Fractional 44.1kHz -> 16kHz (ratio = 2.75625) preserves DC level accurately
    const dc44k = new Float32Array(441).fill(0.25)
    const resampled = downsampleToPcm16(dc44k, 44100, 16000)
    expect(resampled.length).toBe(160)
    expect(resampled.every((sample) => Math.abs(sample - 8192) <= 1)).toBe(true)
  })
  it('accumulates interleaved user and assistant transcript fragments within a turn without wiping either line', () => {
    const state = {
      userTranscript: '',
      assistantTranscript: '',
      userFresh: true,
      assistantFresh: true,
    }
    // User starts speaking, assistant starts replying while final user fragment arrives
    applyLiveTranscriptFragment(state, 'user', 'ช่วยดู ')
    applyLiveTranscriptFragment(state, 'assistant', 'ได้ครับ ')
    applyLiveTranscriptFragment(state, 'user', 'ไฟล์ README หน่อย')
    applyLiveTranscriptFragment(state, 'assistant', 'กำลังอ่านไฟล์ให้ครับ')

    expect(state.userTranscript).toBe('ช่วยดู ไฟล์ README หน่อย')
    expect(state.assistantTranscript).toBe('ได้ครับ กำลังอ่านไฟล์ให้ครับ')

    // After turnComplete (resetting fresh flags), the next turn starts clean lines
    state.userFresh = true
    state.assistantFresh = true
    applyLiveTranscriptFragment(state, 'user', 'ขอบคุณครับ')
    expect(state.userTranscript).toBe('ขอบคุณครับ')
    expect(state.assistantTranscript).toBe('ได้ครับ กำลังอ่านไฟล์ให้ครับ')
  })
  it('guards Escape key in GeminiLiveOverlay when voice picker or pending approval is active', () => {
    expect(resolveLiveOverlayEscape({ voicePickerOpen: true, hasPendingApproval: false })).toBe('close_voice_picker')
    expect(resolveLiveOverlayEscape({ voicePickerOpen: false, hasPendingApproval: true })).toBe('ignore')
    expect(resolveLiveOverlayEscape({ voicePickerOpen: false, hasPendingApproval: false })).toBe('end_call')
  })
})


