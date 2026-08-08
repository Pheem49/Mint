import { useCallback, useEffect, useRef, useState } from 'react'

export type GeminiLiveEvent =
  | { type: 'audioChunk'; data: string }
  | { type: 'transcript'; role: string; text: string }
  | { type: 'turnComplete' }
  | { type: 'toolStatus'; action: string; input: unknown; result?: string | null; error?: string | null }
  | { type: 'error'; message: string }
  | { type: 'closed' }

const MIC_SAMPLE_RATE = 16000
const PLAYBACK_SAMPLE_RATE = 24000
// ScriptProcessorNode buffer size: small enough for low latency, large enough to avoid
// excessive send-chunk traffic to the Rust side.
const CAPTURE_BUFFER_SIZE = 2048

export interface GeminiLiveVoiceOptions {
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
export function useGeminiLiveVoice({ workspacePath, chatId, startSession, sendAudioChunk, stopSession }: GeminiLiveVoiceOptions) {
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

  const voiceModeRef = useRef(false)
  const voiceAwaitingResponseRef = useRef(false)
  const isPausedRef = useRef(false)
  const sessionIdRef = useRef<string | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const micStreamRef = useRef<MediaStream | null>(null)
  const captureNodesRef = useRef<{ source: MediaStreamAudioSourceNode; processor: ScriptProcessorNode } | null>(null)
  const nextPlaybackTimeRef = useRef(0)
  const speakingTimeoutRef = useRef<number | null>(null)
  // Gemini streams transcripts as small fragments, not whole lines — this tracks
  // whose fragment is currently being built so same-role fragments accumulate
  // into one line and a role change (or `turnComplete`) starts a fresh one.
  const transcriptRoleRef = useRef<string | null>(null)

  useEffect(() => {
    voiceModeRef.current = voiceMode
  }, [voiceMode])
  useEffect(() => {
    voiceAwaitingResponseRef.current = voiceAwaitingResponse
  }, [voiceAwaitingResponse])

  const ensureAudioContext = useCallback((): AudioContext => {
    if (!audioContextRef.current || audioContextRef.current.state === 'closed') {
      audioContextRef.current = new AudioContext()
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

  const handleEvent = useCallback((event: GeminiLiveEvent) => {
    switch (event.type) {
      case 'audioChunk':
        setVoiceAwaitingResponse(false)
        playAudioChunk(event.data)
        break
      case 'transcript': {
        const setTranscript = event.role === 'user' ? setUserTranscript : setAssistantTranscript
        setTranscript((prev) => (transcriptRoleRef.current === event.role ? prev + event.text : event.text))
        transcriptRoleRef.current = event.role
        break
      }
      case 'turnComplete':
        // Leaves the finished line on screen — only resets which role the next
        // fragment continues, so the display doesn't flash blank between turns.
        transcriptRoleRef.current = null
        break
      case 'toolStatus':
        transcriptRoleRef.current = null
        setAssistantTranscript(event.error ? `${event.action} failed: ${event.error}` : `Running ${event.action}...`)
        break
      case 'error':
        console.error('Gemini Live session error', event.message)
        transcriptRoleRef.current = null
        setAssistantTranscript(event.message)
        break
      case 'closed':
        setIsRecording(false)
        setVoiceAwaitingResponse(false)
        setIsSpeaking(false)
        sessionIdRef.current = null
        transcriptRoleRef.current = null
        break
    }
  }, [playAudioChunk])

  const stopCapture = useCallback(() => {
    captureNodesRef.current?.processor.disconnect()
    captureNodesRef.current?.source.disconnect()
    captureNodesRef.current = null
    micStreamRef.current?.getTracks().forEach((track) => track.stop())
    micStreamRef.current = null
  }, [])

  const stop = useCallback(() => {
    stopCapture()
    const sessionId = sessionIdRef.current
    sessionIdRef.current = null
    setIsRecording(false)
    setVoiceAwaitingResponse(false)
    setIsPaused(false)
    isPausedRef.current = false
    setIsSpeaking(false)
    if (speakingTimeoutRef.current !== null) {
      window.clearTimeout(speakingTimeoutRef.current)
      speakingTimeoutRef.current = null
    }
    nextPlaybackTimeRef.current = 0
    if (sessionId) stopSession(sessionId).catch((error) => console.error('Failed to stop Gemini Live session', error))
  }, [stopCapture, stopSession])

  const togglePause = useCallback(() => {
    setIsPaused((current) => {
      const next = !current
      isPausedRef.current = next
      return next
    })
  }, [])

  const start = useCallback(async () => {
    try {
      transcriptRoleRef.current = null
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
        audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true }
      })
      console.log('[gemini-live] microphone granted, starting session...')
      micStreamRef.current = stream

      const sessionId = await startSession(handleEvent, workspacePath, chatId)
      console.log('[gemini-live] session started', sessionId)
      sessionIdRef.current = sessionId

      const ctx = ensureAudioContext()
      const source = ctx.createMediaStreamSource(stream)
      const processor = ctx.createScriptProcessor(CAPTURE_BUFFER_SIZE, 1, 1)
      const ratio = ctx.sampleRate / MIC_SAMPLE_RATE

      processor.onaudioprocess = (audioEvent) => {
        if (!sessionIdRef.current || isPausedRef.current) return
        const input = audioEvent.inputBuffer.getChannelData(0)
        const outLength = Math.floor(input.length / ratio)
        const pcm = new Int16Array(outLength)
        for (let i = 0; i < outLength; i += 1) {
          const sample = input[Math.floor(i * ratio)]
          pcm[i] = Math.max(-32768, Math.min(32767, Math.round(sample * 32768)))
        }
        let binary = ''
        const bytes = new Uint8Array(pcm.buffer)
        for (let i = 0; i < bytes.length; i += 1) binary += String.fromCharCode(bytes[i])
        sendAudioChunk(sessionIdRef.current, btoa(binary)).catch((error) =>
          console.error('Failed to send Gemini Live audio chunk', error)
        )
      }

      source.connect(processor)
      processor.connect(ctx.destination)
      captureNodesRef.current = { source, processor }
      setIsRecording(true)
      setVoiceAwaitingResponse(true)
    } catch (error) {
      console.error('Failed to start Gemini Live voice session', error)
      // Keep voiceMode on so the error stays visible in the voice-mode bar instead of
      // vanishing the instant it appears; the user can dismiss it with the mic button.
      setAssistantTranscript(`Error: ${error instanceof Error ? error.message : String(error)}`)
      stopCapture()
      setIsRecording(false)
      setVoiceAwaitingResponse(false)
    }
  }, [handleEvent, workspacePath, chatId, ensureAudioContext, stopCapture, startSession, sendAudioChunk])

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
    restart,
    startRecognition: noop,
    stopRecognition: noop,
    scheduleVoiceListen: noop,
    clearRestartTimer: noop
  }
}
