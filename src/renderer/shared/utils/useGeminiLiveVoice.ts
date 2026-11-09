import { useCallback, useEffect, useRef, useState } from 'react'

export type GeminiLiveEvent =
  | { type: 'audioChunk'; data: string }
  | { type: 'transcript'; role: string; text: string }
  | { type: 'turnComplete' }
  | { type: 'interrupted' }
  | { type: 'toolStatus'; action: string; input: unknown; result?: string | null; error?: string | null }
  | { type: 'approvalRequested'; token: string; approval: Record<string, unknown> }
  | { type: 'error'; message: string }
  | { type: 'closed' }

export interface GeminiLiveSideEffectCallbacks {
  onTurnSaved?: () => void
  onToolActivity?: () => void
  onApprovalRequested?: (approval: { token: string; approval: Record<string, unknown> }) => void
}

export function routeGeminiLiveSideEffects(
  event: GeminiLiveEvent,
  callbacks: GeminiLiveSideEffectCallbacks,
) {
  switch (event.type) {
    case 'turnComplete':
    case 'interrupted':
    case 'closed':
      callbacks.onTurnSaved?.()
      break
    case 'toolStatus':
      callbacks.onToolActivity?.()
      break
    case 'approvalRequested':
      callbacks.onApprovalRequested?.({ token: event.token, approval: event.approval })
      break
  }
}

export interface StoppableAudioSource {
  stop: () => void
  disconnect: () => void
}

export interface LiveTranscriptFrameState {
  userTranscript: string
  assistantTranscript: string
  userFresh: boolean
  assistantFresh: boolean
}

export function applyLiveTranscriptFragment(
  state: LiveTranscriptFrameState,
  role: string,
  text: string,
) {
  if (role === 'user') {
    state.userTranscript = state.userFresh ? text : state.userTranscript + text
    state.userFresh = false
  } else {
    state.assistantTranscript = state.assistantFresh ? text : state.assistantTranscript + text
    state.assistantFresh = false
  }
}

/**
 * Stops and disconnects all scheduled/playing audio buffer sources immediately,
 * resetting the playback timeline cursor and speaking indicator. Used both on
 * user barge-in (`interrupted`) and when ending a Live session.
 */
export function stopQueuedAudioPlayback(
  sources: Set<StoppableAudioSource>,
  nextPlaybackTimeRef: { current: number },
  speakingTimeoutRef: { current: number | null },
  setIsSpeaking: (speaking: boolean) => void,
) {
  for (const source of sources) {
    try {
      source.stop()
      source.disconnect()
    } catch {
      /* Already ended. */
    }
  }
  sources.clear()
  nextPlaybackTimeRef.current = 0
  setIsSpeaking(false)
  if (speakingTimeoutRef.current !== null && typeof window !== 'undefined') {
    window.clearTimeout(speakingTimeoutRef.current)
    speakingTimeoutRef.current = null
  }
}

const MIC_SAMPLE_RATE = 16000
const PLAYBACK_SAMPLE_RATE = 24000
// Capture chunk size (~128ms at 16kHz, ~42ms at 48kHz): small enough for low latency,
// large enough to avoid excessive send-chunk traffic to the Rust side.
const CAPTURE_BUFFER_SIZE = 2048

function floatToPcm16Sample(sample: number): number {
  const clamped = Math.max(-1, Math.min(1, sample))
  return clamped < 0
    ? Math.max(-32768, Math.round(clamped * 32768))
    : Math.min(32767, Math.round(clamped * 32768))
}

/**
 * Downsamples Float32 mono audio to 16kHz PCM16 using a continuous time-weighted
 * boxcar anti-aliasing filter (low-pass average over each output sample window)
 * instead of nearest-neighbor point sampling, preventing high-frequency aliasing.
 */
export function downsampleToPcm16(
  input: Float32Array,
  inputSampleRate: number,
  targetSampleRate: number = MIC_SAMPLE_RATE,
): Int16Array {
  if (input.length === 0 || inputSampleRate <= 0 || targetSampleRate <= 0) {
    return new Int16Array(0)
  }
  if (inputSampleRate === targetSampleRate) {
    const out = new Int16Array(input.length)
    for (let i = 0; i < input.length; i += 1) {
      out[i] = floatToPcm16Sample(input[i])
    }
    return out
  }
  const ratio = inputSampleRate / targetSampleRate
  const outLength = Math.floor(input.length / ratio)
  const out = new Int16Array(outLength)
  for (let i = 0; i < outLength; i += 1) {
    const start = i * ratio
    const end = (i + 1) * ratio
    const firstIdx = Math.floor(start)
    const lastIdx = Math.floor(end)
    let avg = 0
    if (firstIdx === lastIdx) {
      avg = input[firstIdx] ?? 0
    } else {
      let sum = 0
      let weight = 0
      const wLeft = firstIdx + 1 - start
      sum += (input[firstIdx] ?? 0) * wLeft
      weight += wLeft
      const interiorEnd = Math.min(lastIdx, input.length)
      for (let k = firstIdx + 1; k < interiorEnd; k += 1) {
        sum += input[k]
        weight += 1
      }
      if (lastIdx < input.length && end > lastIdx) {
        const wRight = end - lastIdx
        sum += input[lastIdx] * wRight
        weight += wRight
      }
      avg = weight > 0 ? sum / weight : 0
    }
    out[i] = floatToPcm16Sample(avg)
  }
  return out
}

export function pcm16ToBase64(pcm: Int16Array): string {
  const bytes = new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength)
  let binary = ''
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i])
  }
  return btoa(binary)
}

const WORKLET_PROCESSOR_NAME = 'mint-gemini-live-capture'
const WORKLET_SOURCE = `
class MintGeminiLiveCaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this._buffer = new Float32Array(${CAPTURE_BUFFER_SIZE});
    this._offset = 0;
  }
  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (!channel || channel.length === 0) return true;
    let read = 0;
    while (read < channel.length) {
      const space = this._buffer.length - this._offset;
      const toCopy = Math.min(space, channel.length - read);
      this._buffer.set(channel.subarray(read, read + toCopy), this._offset);
      this._offset += toCopy;
      read += toCopy;
      if (this._offset >= this._buffer.length) {
        const chunk = this._buffer.slice(0);
        this._offset = 0;
        this.port.postMessage(chunk, [chunk.buffer]);
      }
    }
    return true;
  }
}
registerProcessor('${WORKLET_PROCESSOR_NAME}', MintGeminiLiveCaptureProcessor);
`

export interface GeminiLiveVoiceOptions extends GeminiLiveSideEffectCallbacks {
  workspacePath?: string | null
  chatId?: string | null
  /** Starts a session and returns its id; `onEvent` fires for the session's lifetime. */
  startSession: (onEvent: (event: GeminiLiveEvent) => void, workspacePath?: string | null, chatId?: string | null) => Promise<string>
  /** Pushes a chunk of base64-encoded PCM16 (16kHz, mono) mic audio into a running session. */
  sendAudioChunk: (sessionId: string, chunkBase64: string) => Promise<void>
  stopSession: (sessionId: string) => Promise<void>
}

/**
 * Realtime voice mode backed by Google's Gemini Live API (beta): captures mic audio,
 * streams it to a Rust-side Gemini Live session (via Tauri IPC on desktop, a WebSocket on
 * web — see the injected `startSession`/`sendAudioChunk`/`stopSession` options), and plays
 * back the model's spoken reply as it arrives. Voice-driven tool calls (video edit,
 * subtitles, etc.) are executed on the Rust side and reported here as transcript lines.
 *
 * Exposes the same field names as `useSpeechToText` for isRecording/voiceMode/
 * startRecognition/stopRecognition so it can be swapped in without reworking a chat
 * panel's mic button, plus its own `userTranscript`/`assistantTranscript` (kept
 * separate, not one shared line, so the user's last line stays visible under "You:"
 * once Mint starts replying under "Mint:"). `startRecognition`/`stopRecognition`/
 * `scheduleVoiceListen` are no-ops here since the Live API keeps one continuous
 * duplex stream instead of discrete listen-then-send cycles — starting/stopping is
 * driven entirely by `setVoiceMode`.
 */
export function useGeminiLiveVoice({
  workspacePath,
  chatId,
  startSession,
  sendAudioChunk,
  stopSession,
  onTurnSaved,
  onToolActivity,
  onApprovalRequested,
}: GeminiLiveVoiceOptions) {
  const [isRecording, setIsRecording] = useState(false)
  const [voiceMode, setVoiceMode] = useState(false)
  // Kept separate (rather than one shared line) so the user's own speech stays
  // visible under "You:" once Mint starts replying under "Mint:", instead of
  // one role's fragments overwriting the other's.
  const [userTranscript, setUserTranscript] = useState('')
  const [assistantTranscript, setAssistantTranscript] = useState('')
  const [voiceAwaitingResponse, setVoiceAwaitingResponse] = useState(false)
  const [isPaused, setIsPaused] = useState(false)
  const [isSpeaking, setIsSpeaking] = useState(false)
  const [errorMessage, setErrorMessage] = useState('')

  const voiceModeRef = useRef(false)
  const voiceAwaitingResponseRef = useRef(false)
  const isPausedRef = useRef(false)
  const sessionIdRef = useRef<string | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)
  const generationRef = useRef(0)
  const playbackSourcesRef = useRef(new Set<AudioBufferSourceNode>())
  const stopRef = useRef<() => void>(() => {})
  const sideEffectsRef = useRef<GeminiLiveSideEffectCallbacks>({
    onTurnSaved,
    onToolActivity,
    onApprovalRequested,
  })
  sideEffectsRef.current = { onTurnSaved, onToolActivity, onApprovalRequested }
  const micStreamRef = useRef<MediaStream | null>(null)
  const captureNodesRef = useRef<{
    source: MediaStreamAudioSourceNode
    processor: AudioNode
    workletPort?: MessagePort
  } | null>(null)
  const nextPlaybackTimeRef = useRef(0)
  const speakingTimeoutRef = useRef<number | null>(null)
  // Gemini streams transcripts as small fragments, not whole lines — track
  // freshness per role independently so interleaved user/assistant fragments
  // within the same turn accumulate into their own lines without wiping each
  // other out, and start fresh after `turnComplete` / `interrupted`.
  const userTurnFreshRef = useRef(true)
  const assistantTurnFreshRef = useRef(true)

  useEffect(() => {
    voiceModeRef.current = voiceMode
  }, [voiceMode])
  useEffect(() => {
    voiceAwaitingResponseRef.current = voiceAwaitingResponse
  }, [voiceAwaitingResponse])

  const ensureAudioContext = useCallback((): AudioContext => {
    if (!audioContextRef.current || audioContextRef.current.state === 'closed') {
      audioContextRef.current = new AudioContext()
      analyserRef.current = audioContextRef.current.createAnalyser()
      analyserRef.current.fftSize = 512
      analyserRef.current.smoothingTimeConstant = 0.3
      analyserRef.current.minDecibels = -90
      analyserRef.current.maxDecibels = -10
    }
    return audioContextRef.current
  }, [])

  const playAudioChunk = useCallback((base64: string) => {
    const ctx = ensureAudioContext()
    const binary = atob(base64)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i)
    const samples = new Int16Array(bytes.buffer, bytes.byteOffset, Math.floor(bytes.length / 2))

    const buffer = ctx.createBuffer(1, samples.length, PLAYBACK_SAMPLE_RATE)
    const channel = buffer.getChannelData(0)
    for (let i = 0; i < samples.length; i += 1) channel[i] = samples[i] / 32768

    const source = ctx.createBufferSource()
    source.buffer = buffer
    source.connect(ctx.destination)
    if (analyserRef.current) source.connect(analyserRef.current)
    playbackSourcesRef.current.add(source)
    source.onended = () => { playbackSourcesRef.current.delete(source); source.disconnect() }

    const startAt = Math.max(ctx.currentTime, nextPlaybackTimeRef.current)
    source.start(startAt)
    nextPlaybackTimeRef.current = startAt + buffer.duration

    // Chunks arrive continuously while the model is talking; flip back out of "speaking"
    // a short moment after the last chunk instead of on every single chunk boundary.
    setIsSpeaking(true)
    if (speakingTimeoutRef.current !== null) window.clearTimeout(speakingTimeoutRef.current)
    const quietDelayMs = Math.max(0, (startAt + buffer.duration - ctx.currentTime) * 1000) + 500
    speakingTimeoutRef.current = window.setTimeout(() => setIsSpeaking(false), quietDelayMs)
  }, [ensureAudioContext])

  const stopPlayback = useCallback(() => {
    stopQueuedAudioPlayback(
      playbackSourcesRef.current,
      nextPlaybackTimeRef,
      speakingTimeoutRef,
      setIsSpeaking,
    )
  }, [])

  const handleEvent = useCallback((event: GeminiLiveEvent) => {
    switch (event.type) {
      case 'audioChunk':
        setVoiceAwaitingResponse(false)
        playAudioChunk(event.data)
        break
      case 'transcript': {
        if (event.role === 'user') {
          const fresh = userTurnFreshRef.current
          userTurnFreshRef.current = false
          setUserTranscript((prev) => (fresh ? event.text : prev + event.text))
        } else {
          const fresh = assistantTurnFreshRef.current
          assistantTurnFreshRef.current = false
          setAssistantTranscript((prev) => (fresh ? event.text : prev + event.text))
        }
        break
      }
      case 'turnComplete':
        // Leaves the finished line on screen — only resets per-role freshness
        // so the display doesn't flash blank between turns.
        userTurnFreshRef.current = true
        assistantTurnFreshRef.current = true
        break
      case 'interrupted':
        // User barged in while Mint was speaking — flush queued audio immediately.
        stopPlayback()
        userTurnFreshRef.current = true
        assistantTurnFreshRef.current = true
        break
      case 'toolStatus':
        assistantTurnFreshRef.current = true
        setAssistantTranscript(event.error ? `${event.action} failed: ${event.error}` : `Running ${event.action}...`)
        break
      case 'approvalRequested':
        break
      case 'error':
        console.error('Gemini Live session error', event.message)
        userTurnFreshRef.current = true
        assistantTurnFreshRef.current = true
        setAssistantTranscript(event.message)
        setErrorMessage(event.message)
        break
      case 'closed':
        stopRef.current()
        setIsRecording(false)
        setVoiceAwaitingResponse(false)
        setIsSpeaking(false)
        sessionIdRef.current = null
        userTurnFreshRef.current = true
        assistantTurnFreshRef.current = true
        setErrorMessage('Live connection closed')
        break
    }
    routeGeminiLiveSideEffects(event, sideEffectsRef.current)
  }, [playAudioChunk, stopPlayback])

  const stopCapture = useCallback(() => {
    if (captureNodesRef.current?.workletPort) {
      captureNodesRef.current.workletPort.onmessage = null
      captureNodesRef.current.workletPort.close?.()
    }
    captureNodesRef.current?.processor.disconnect()
    captureNodesRef.current?.source.disconnect()
    captureNodesRef.current = null
    micStreamRef.current?.getTracks().forEach((track) => track.stop())
    micStreamRef.current = null
  }, [])

  const stop = useCallback(() => {
    ++generationRef.current
    stopCapture()
    stopPlayback()
    analyserRef.current?.disconnect()
    analyserRef.current = null
    const context = audioContextRef.current
    audioContextRef.current = null
    if (context && context.state !== 'closed') void context.close().catch(error => console.error('Failed to close Live audio', error))
    const sessionId = sessionIdRef.current
    sessionIdRef.current = null
    setIsRecording(false)
    setVoiceAwaitingResponse(false)
    setIsPaused(false)
    isPausedRef.current = false
    if (sessionId) {
      stopSession(sessionId)
        .then(() => {
          sideEffectsRef.current.onTurnSaved?.()
        })
        .catch((error) => console.error('Failed to stop Gemini Live session', error))
    }
  }, [stopCapture, stopPlayback, stopSession])
  stopRef.current = stop

  const togglePause = useCallback(() => {
    setIsPaused((current) => {
      const next = !current
      isPausedRef.current = next
      return next
    })
  }, [])

  const start = useCallback(async () => {
    const generation = ++generationRef.current
    try {
      setErrorMessage('')
      userTurnFreshRef.current = true
      assistantTurnFreshRef.current = true
      setUserTranscript('')
      setAssistantTranscript('')
      if (!navigator.mediaDevices?.getUserMedia) {
        throw new Error('This window does not support microphone capture (navigator.mediaDevices.getUserMedia is unavailable).')
      }

      console.log('[gemini-live] requesting microphone permission...')
      // Without echoCancellation, the model's own spoken reply (played back through
      // the speakers in playAudioChunk) gets picked up by the mic and re-sent to
      // Gemini as if the user said it — causing duplicated transcripts and Gemini
      // interrupting/looping on its own voice.
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          sampleRate: MIC_SAMPLE_RATE,
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        },
      })
      if (generation !== generationRef.current) {
        stream.getTracks().forEach(track => track.stop())
        return
      }
      console.log('[gemini-live] microphone granted, starting session...')
      micStreamRef.current = stream

      const sessionId = await startSession(event => {
        if (generation === generationRef.current) handleEvent(event)
      }, workspacePath, chatId)
      if (generation !== generationRef.current) {
        await stopSession(sessionId)
        return
      }
      console.log('[gemini-live] session started', sessionId)
      sessionIdRef.current = sessionId

      const ctx = ensureAudioContext()
      const source = ctx.createMediaStreamSource(stream)
      if (ctx.state === 'suspended') await ctx.resume()
      if (generation !== generationRef.current) { source.disconnect(); return }

      const forwardFloatChunk = (samples: Float32Array) => {
        if (!sessionIdRef.current || isPausedRef.current) return
        const pcm = downsampleToPcm16(samples, ctx.sampleRate, MIC_SAMPLE_RATE)
        if (pcm.length === 0) return
        sendAudioChunk(sessionIdRef.current, pcm16ToBase64(pcm)).catch((error) =>
          console.error('Failed to send Gemini Live audio chunk', error),
        )
      }

      let attachedWorklet = false
      if (
        typeof AudioWorkletNode !== 'undefined' &&
        ctx.audioWorklet &&
        typeof ctx.audioWorklet.addModule === 'function' &&
        typeof Blob !== 'undefined' &&
        typeof URL !== 'undefined' &&
        typeof URL.createObjectURL === 'function'
      ) {
        try {
          const blob = new Blob([WORKLET_SOURCE], { type: 'application/javascript' })
          const moduleUrl = URL.createObjectURL(blob)
          try {
            await ctx.audioWorklet.addModule(moduleUrl)
          } finally {
            URL.revokeObjectURL(moduleUrl)
          }
          if (generation !== generationRef.current) {
            source.disconnect()
            return
          }
          const workletNode = new AudioWorkletNode(ctx, WORKLET_PROCESSOR_NAME, {
            numberOfInputs: 1,
            numberOfOutputs: 0,
            channelCount: 1,
          })
          workletNode.port.onmessage = (msgEvent: MessageEvent<Float32Array>) => {
            if (msgEvent.data instanceof Float32Array) {
              forwardFloatChunk(msgEvent.data)
            }
          }
          source.connect(workletNode)
          if (analyserRef.current) source.connect(analyserRef.current)
          captureNodesRef.current = { source, processor: workletNode, workletPort: workletNode.port }
          attachedWorklet = true
        } catch {
          // Fall back to ScriptProcessorNode below if AudioWorklet registration fails.
        }
      }

      if (!attachedWorklet) {
        const processor = ctx.createScriptProcessor(CAPTURE_BUFFER_SIZE, 1, 1)
        processor.onaudioprocess = (audioEvent) => {
          forwardFloatChunk(audioEvent.inputBuffer.getChannelData(0))
        }
        source.connect(processor)
        if (analyserRef.current) source.connect(analyserRef.current)
        processor.connect(ctx.destination)
        captureNodesRef.current = { source, processor }
      }
      setIsRecording(true)
      setVoiceAwaitingResponse(true)
    } catch (error) {
      if (generation !== generationRef.current) return
      console.error('Failed to start Gemini Live voice session', error)
      // Keep voiceMode on so the error stays visible in the voice-mode bar instead of
      // vanishing the instant it appears; the user can dismiss it with the mic button.
      setAssistantTranscript(`Error: ${error instanceof Error ? error.message : String(error)}`)
      setErrorMessage(error instanceof Error ? error.message : String(error))
      stop()
    }
  }, [handleEvent, workspacePath, chatId, ensureAudioContext, stop, startSession, stopSession, sendAudioChunk])

  useEffect(() => {
    if (voiceMode) {
      start()
    } else {
      stop()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [voiceMode])

  useEffect(() => stop, [stop])

  // Reconnects with a fresh session — used when a setting that only takes effect at the
  // Live API's `setup` handshake (e.g. voice) changes while a call is already in progress.
  const restart = useCallback(() => {
    if (!voiceModeRef.current) return
    stop()
    start()
  }, [stop, start])

  const noop = useCallback(() => {}, [])

  return {
    isRecording,
    voiceMode,
    setVoiceMode,
    userTranscript,
    assistantTranscript,
    voiceAwaitingResponse,
    voiceAwaitingResponseRef,
    voiceModeRef,
    isPaused,
    togglePause,
    isSpeaking,
    analyserRef,
    errorMessage,
    restart,
    startRecognition: noop,
    stopRecognition: noop,
    scheduleVoiceListen: noop,
    clearRestartTimer: noop
  }
}
