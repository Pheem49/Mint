import { useCallback, useEffect, useRef, useState } from 'react'
import { createNativeRecordingSession, type NativeRecordingBackend } from './nativeRecordingSession'

export interface NativeVoiceInputOptions extends NativeRecordingBackend {
  chatId?: string
  message: string
  onSetMessage: (text: string) => void
}

/** Explicit push-to-talk, with cancel/stop requests serialized behind device startup. */
export function useNativeVoiceInput(options: NativeVoiceInputOptions) {
  const [isRecording, setIsRecording] = useState(false)
  const [voiceMode, setVoiceMode] = useState(false)
  const [voiceTranscript, setVoiceTranscript] = useState('')
  const [voiceAwaitingResponse, setVoiceAwaitingResponse] = useState(false)
  const voiceAwaitingResponseRef = useRef(false)
  const voiceModeRef = useRef(false)
  const latest = useRef(options)
  const previousChatId = useRef(options.chatId)
  latest.current = options
  const mounted = useRef(true)
  const epoch = useRef(0)
  const stopping = useRef<Promise<void> | null>(null)
  const session = useRef<ReturnType<typeof createNativeRecordingSession> | null>(null)
  session.current ??= createNativeRecordingSession({
    startRecording: () => latest.current.startRecording(),
    stopRecordingAndTranscribe: () => latest.current.stopRecordingAndTranscribe(),
    cancelRecording: () => latest.current.cancelRecording(),
  })
  useEffect(() => { voiceAwaitingResponseRef.current = voiceAwaitingResponse }, [voiceAwaitingResponse])
  useEffect(() => { voiceModeRef.current = voiceMode }, [voiceMode])

  const startRecognition = useCallback(async () => {
    if (stopping.current) return false
    const request = ++epoch.current
    const chatId = latest.current.chatId
    setVoiceTranscript('')
    try {
      const started = await session.current!.start()
      if (started && mounted.current && request === epoch.current && chatId === latest.current.chatId) {
        setIsRecording(true)
        setVoiceMode(true)
      }
      return started && mounted.current && request === epoch.current && chatId === latest.current.chatId
    } catch (error) {
      if (mounted.current && request === epoch.current) {
        setVoiceTranscript(error instanceof Error ? error.message : String(error))
        setVoiceMode(true)
      }
      return false
    }
  }, [])

  const stopRecognition = useCallback(() => {
    if (stopping.current) return stopping.current
    const request = epoch.current
    const chatId = latest.current.chatId
    setIsRecording(false)
    setVoiceAwaitingResponse(true)
    setVoiceTranscript('Transcribing...')
    const finish = async () => {
      try {
        const text = await session.current!.stop()
        if (!mounted.current || request !== epoch.current || chatId !== latest.current.chatId || text === null) return
        const trimmed = text.trim()
        setVoiceTranscript(trimmed || 'No speech detected')
        if (trimmed) {
          const { message, onSetMessage } = latest.current
          onSetMessage(message.trim() ? `${message.trimEnd()} ${trimmed}` : trimmed)
        }
      } catch (error) {
        if (mounted.current && request === epoch.current) {
          setVoiceTranscript(error instanceof Error ? error.message : String(error))
        }
      } finally {
        stopping.current = null
        if (mounted.current) {
          setVoiceAwaitingResponse(false)
          setVoiceMode(false)
        }
      }
    }
    const task = finish()
    stopping.current = task
    return task
  }, [])

  const cancelRecognition = useCallback(async () => {
    ++epoch.current
    setIsRecording(false)
    setVoiceMode(false)
    setVoiceTranscript('')
    try { await session.current!.cancel() } catch (error) {
      if (mounted.current) {
        setVoiceTranscript(error instanceof Error ? error.message : String(error))
        setVoiceMode(true)
      }
    }
  }, [])

  useEffect(() => {
    if (previousChatId.current === options.chatId) return
    previousChatId.current = options.chatId
    void cancelRecognition()
  }, [options.chatId, cancelRecognition])

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
      ++epoch.current
      void session.current!.cancel().catch(error => console.error('Failed to close microphone', error))
    }
  }, [])
  // Native recording is one-shot; the legacy voice-conversation scheduling API stays inert.
  const scheduleVoiceListen = useCallback((_delayMs?: number) => {}, [])
  const clearRestartTimer = useCallback(() => {}, [])
  return { isRecording, voiceMode, setVoiceMode, voiceTranscript, setVoiceTranscript,
    voiceAwaitingResponse, voiceAwaitingResponseRef, voiceModeRef, startRecognition,
    stopRecognition, cancelRecognition, scheduleVoiceListen, clearRestartTimer }
}
