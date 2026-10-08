export interface NativeRecordingBackend {
  startRecording: () => Promise<void>
  stopRecordingAndTranscribe: () => Promise<string>
  cancelRecording: () => Promise<void>
}

/** One recorder lifecycle, including stop/cancel requests made during device startup. */
export function createNativeRecordingSession(backend: NativeRecordingBackend) {
  type Run = { phase: 'starting' | 'recording' | 'ending'; cancelled: boolean; ready: Promise<void>; result?: Promise<string | null> }
  let run: Run | null = null

  const start = async () => {
    if (run) return false
    const current: Run = { phase: 'starting', cancelled: false, ready: Promise.resolve() }
    run = current
    current.ready = backend.startRecording()
    try {
      await current.ready
      if (current.phase !== 'starting') return false
      current.phase = 'recording'
      return true
    } catch (error) {
      if (run === current) run = null
      throw error
    }
  }
  const finish = (cancelled: boolean): Promise<string | null> => {
    const current = run
    if (!current) return Promise.resolve(null)
    current.cancelled ||= cancelled
    if (current.result) return current.result
    current.phase = 'ending'
    current.result = (async () => {
      try {
        try { await current.ready } catch { return null }
        if (current.cancelled) { await backend.cancelRecording(); return null }
        const text = await backend.stopRecordingAndTranscribe()
        return current.cancelled ? null : text
      } finally {
        if (run === current) run = null
      }
    })()
    return current.result
  }
  return { start, stop: () => finish(false), cancel: async () => { await finish(true) } }
}
