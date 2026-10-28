import { expect, it } from 'vitest'
import { AutoSpeechTracker, mouthFromSamples } from './speech-policy'
it('never reads a reconnect snapshot but allows a running turn to finish afterward', () => {
  const tracker = new AutoSpeechTracker()
  expect(tracker.observe([{ turnId: 1, status: 'completed' }, { turnId: 2, status: 'thinking' }])).toBeNull()
  expect(tracker.observe([{ turnId: 1, status: 'completed' }, { turnId: 2, status: 'completed' }])).toBe(2)
  expect(tracker.observe([{ turnId: 2, status: 'completed' }])).toBeNull()
  tracker.reset()
  expect(tracker.observe([{ turnId: 2, status: 'completed' }])).toBeNull()
})
it('tracks completed replies even while automatic reading is disabled', () => {
  const tracker = new AutoSpeechTracker()
  tracker.observe([])
  expect(tracker.observe([{ turnId: 1, status: 'completed' }])).toBe(1)
  expect(tracker.observe([{ turnId: 1, status: 'completed' }, { turnId: 2, status: 'failed' }])).toBeNull()
})
it('closes the mouth on silence and clamps loud samples', () => {
  expect(mouthFromSamples(new Float32Array(128), 0)).toBe(0)
  expect(mouthFromSamples(new Float32Array(128).fill(1), 0)).toBeGreaterThan(0)
  expect(mouthFromSamples(new Float32Array(128).fill(1), 1)).toBeLessThanOrEqual(1)
  expect(mouthFromSamples(new Float32Array(128), 1)).toBeLessThan(1)
})

it('stopping during preparation never starts late audio and releases its playback owner', async () => {
  const { vi } = await import('vitest')
  const { SpeechPlayer } = await import('../../../src/renderer/shared/speech/player')
  vi.stubGlobal('crypto', { randomUUID: () => '11111111-1111-4111-8111-111111111111' })
  vi.stubGlobal('cancelAnimationFrame', () => {})
  vi.stubGlobal('AudioContext', class { resume() { return Promise.resolve() } close() { return Promise.resolve() } })
  let finish!: (reply: any) => void
  let started!: () => void
  const preparing = new Promise<void>(resolve => { started = resolve })
  let audioRequests = 0
  let preparedOwner = ''
  const released = new Set<string>()
  let state = ''
  const player = new SpeechPlayer(async request => {
    if (request.operation === 'prepare') { preparedOwner = request.owner; started(); return new Promise(resolve => { finish = resolve }) }
    if (request.operation === 'audio') { audioRequests++; throw new Error('Cancelled audio must not be requested') }
    if (request.operation === 'release') { released.add(request.owner); return { kind: 'released' } }
    return { kind: 'lease', active: true }
  }, next => { state = next }, message => { throw new Error(message) })
  try {
    const playing = player.play({ chatId: 'one', turnId: 1, audioOnly: true })
    await preparing
    await player.stop()
    finish({ kind: 'prepared', text: 'hello', provider: 'google', volume: 1, speed: 1, pitch: 1, chunks: 1 })
    await playing
    expect(audioRequests).toBe(0)
    expect(state).toBe('idle')
    expect(released.has(preparedOwner)).toBe(true)
    await player.dispose()
  } finally { vi.unstubAllGlobals() }
})

it('a cancelled run finishing renewal cannot close the mouth of a newer playback', async () => {
  const { vi } = await import('vitest')
  const { SpeechPlayer } = await import('../../../src/renderer/shared/speech/player')
  let sequence = 0
  vi.stubGlobal('crypto', { randomUUID: () => `11111111-1111-4111-8111-${String(++sequence).padStart(12, '0')}` })
  const frames = new Set<number>(); let frameId = 0
  vi.stubGlobal('requestAnimationFrame', () => { frames.add(++frameId); return frameId })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id))
  let started!: () => void; const secondStarted = new Promise<void>(resolve => { started = resolve })
  vi.stubGlobal('AudioContext', class {
    resume() { return Promise.resolve() } close() { return Promise.resolve() }
    decodeAudioData() { return Promise.resolve({ duration: 1 }) }
    createGain() { return { gain: { value: 1 }, connect() {}, disconnect() {} } }
    createAnalyser() { return { fftSize: 256, connect() {}, disconnect() {}, getFloatTimeDomainData(samples: Float32Array) { samples.fill(0.25) } } }
    createBufferSource() { return { buffer: null, playbackRate: { value: 1 }, onended: null as null | (() => void), connect() {}, disconnect() {}, start() { started() }, stop() { this.onended?.() } } }
  })
  let renewStarted!: () => void; const renewing = new Promise<void>(resolve => { renewStarted = resolve })
  let finishOldRenew!: (value: any) => void
  let firstOwner = ''
  let mouthOpen = false
  const player = new SpeechPlayer(async request => {
    if (request.operation === 'prepare') {
      firstOwner ||= request.owner
      return { kind: 'prepared', text: 'hello', provider: 'google', volume: 1, speed: 1, pitch: 1, chunks: 1 }
    }
    if (request.operation === 'audio') return { kind: 'audio', data: 'AA==' }
    if (request.operation === 'renew' && request.owner === firstOwner) { renewStarted(); return new Promise(resolve => { finishOldRenew = resolve }) }
    if (request.operation === 'release') return { kind: 'released' }
    return { kind: 'lease', active: true }
  }, () => {}, message => { throw new Error(message) }, samples => { mouthOpen = samples !== null })
  try {
    const old = player.play({ chatId: 'one', turnId: 1 })
    await renewing
    const current = player.play({ chatId: 'two', turnId: 2 })
    await secondStarted
    finishOldRenew({ kind: 'lease', active: false })
    await old
    expect(mouthOpen).toBe(true)
    expect(frames.size).toBe(1)
    await player.stop(); await current; await player.dispose()
  } finally { vi.unstubAllGlobals() }
})
