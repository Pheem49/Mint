import { describe, expect, it } from 'vitest'

import { createNativeRecordingSession as create } from './nativeRecordingSession'

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>(done => { resolve = done })
  return { promise, resolve }
}

describe('native microphone recording lifecycle', () => {
  it('discards a canceled recording even when device startup finishes later', async () => {
    const ready = deferred(); const calls: string[] = []
    const session = create({
      startRecording: async () => { calls.push('start'); await ready.promise },
      stopRecordingAndTranscribe: async () => { calls.push('transcribe'); return 'should not send' },
      cancelRecording: async () => { calls.push('discard') }
    })
    const start = session.start()
    const cancel = session.cancel()
    ready.resolve()
    expect(await start).toBe(false)
    await cancel
    expect(calls).toEqual(['start', 'discard'])
  })
  it('waits for startup before stopping, transcribes once, and permits another recording', async () => {
    const ready = deferred(); const calls: string[] = []
    const session = create({
      startRecording: async () => { calls.push('start'); await ready.promise },
      stopRecordingAndTranscribe: async () => { calls.push('transcribe'); return 'Hello Mint' },
      cancelRecording: async () => { calls.push('discard') }
    })
    const start = session.start(); const stop = session.stop(); const duplicate = session.stop()
    ready.resolve(); await start
    expect(await stop).toBe('Hello Mint'); expect(await duplicate).toBe('Hello Mint')
    expect(calls).toEqual(['start', 'transcribe'])
    expect(await session.start()).toBe(true); await session.cancel()
    expect(calls).toEqual(['start', 'transcribe', 'start', 'discard'])
  })
  it('does not transcribe or discard when opening the microphone is denied', async () => {
    const calls: string[] = []
    const session = create({
      startRecording: async () => { throw new Error('Microphone denied') },
      stopRecordingAndTranscribe: async () => { calls.push('transcribe'); return 'wrong' },
      cancelRecording: async () => { calls.push('discard') }
    })
    await expect(session.start()).rejects.toThrow('Microphone denied')
    await session.cancel(); expect(calls).toEqual([])
  })
  it('suppresses a transcript when canceled during transcription', async () => {
    const ready = deferred()
    const session = create({
      startRecording: async () => {},
      stopRecordingAndTranscribe: async () => { await ready.promise; return 'late transcript' },
      cancelRecording: async () => { throw new Error('already stopped') }
    })
    await session.start(); const stop = session.stop(); await Promise.resolve()
    const cancel = session.cancel(); ready.resolve()
    expect(await stop).toBeNull(); await cancel
  })
})
