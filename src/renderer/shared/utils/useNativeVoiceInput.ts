import { useCallback, useEffect, useRef, useState } from 'react'

export interface NativeVoiceInputOptions {
  onSendVoiceMessage: (text: string) => Promise<any>
  startRecording: () => Promise<void>
  stopRecordingAndTranscribe: () => Promise<string>
}

/**
 * Push-to-talk voice input backed by a Rust-side mic recorder (cpal) and
 * whichever chat provider is configured for transcription — desktop only.
 * Replaces the old browser `window.SpeechRecognition`-based `useSpeechToText`,
 * which only worked in Chrome/Edge and not at all in the Tauri desktop
 * webview on Linux.
 *
 * Exposes the same field names `useSpeechToText` did (isRecording/voiceMode/
 * startRecognition/stopRecognition/scheduleVoiceListen/clearRestartTimer) so
 * it drops into a chat panel's mic button without reworking call sites.
 * Unlike the old hook, this is explicit push-to-talk with no continuous
 * auto-relisten loop (there's no mid-recording silence detection — audio only
 * reaches the transcription step after the user presses stop), so
 * `scheduleVoiceListen`/`clearRestartTimer` are no-ops here.
 */
export function useNativeVoiceInput({ onSendVoiceMessage, startRecording, stopRecordingAndTranscribe }: NativeVoiceInputOptions) {
  const [isRecording, setIsRecording] = useState(false)
  const [voiceMode, setVoiceMode] = useState(false)
  const [voiceTranscript, setVoiceTranscript] = useState('')
  const [voiceAwaitingResponse, setVoiceAwaitingResponse] = useState(false)

  const voiceAwaitingResponseRef = useRef(false)
  const voiceModeRef = useRef(false)

  useEffect(() => {
    voiceAwaitingResponseRef.current = voiceAwaitingResponse
  }, [voiceAwaitingResponse])
  useEffect(() => {
    voiceModeRef.current = voiceMode
  }, [voiceMode])

  const startRecognition = useCallback(async () => {
    if (isRecording) return
    setVoiceTranscript('')
    try {
      await startRecording()
      setIsRecording(true)
      setVoiceMode(true)
    } catch (error) {
      console.error('Failed to start native mic recording', error)
      setVoiceTranscript(error instanceof Error ? error.message : String(error))
    }
  }, [isRecording, startRecording])

  const stopRecognition = useCallback(async () => {
    if (!isRecording) return
    setIsRecording(false)
    setVoiceAwaitingResponse(true)
    setVoiceTranscript('Transcribing...')
    try {
      const text = await stopRecordingAndTranscribe()
      const trimmed = text.trim()
      if (trimmed) {
        setVoiceTranscript(trimmed)
        await onSendVoiceMessage(trimmed)
      } else {
        setVoiceTranscript('No speech detected')
      }
    } catch (error) {
      console.error('Native voice transcription failed', error)
      // Surfaces MicTranscribeError's message verbatim, e.g. "provider 'anthropic'
      // does not support audio input for voice transcription — switch to a
      // provider that does..."
      setVoiceTranscript(error instanceof Error ? error.message : String(error))
    } finally {
      setVoiceAwaitingResponse(false)
      setVoiceMode(false)
    }
  }, [isRecording, stopRecordingAndTranscribe, onSendVoiceMessage])

  // No continuous auto-relisten loop under native push-to-talk — kept as no-ops
  // so existing call sites (which invoke these after TTS playback finishes) stay
  // harmless without needing to be stripped out everywhere.
  const scheduleVoiceListen = useCallback((_delayMs?: number) => {}, [])
  const clearRestartTimer = useCallback(() => {}, [])

  return {
    isRecording,
    voiceMode,
    setVoiceMode,
    voiceTranscript,
    setVoiceTranscript,
    voiceAwaitingResponse,
    voiceAwaitingResponseRef,
    voiceModeRef,
    startRecognition,
    stopRecognition,
    scheduleVoiceListen,
    clearRestartTimer
  }
}
