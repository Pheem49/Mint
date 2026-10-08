import { useState, useRef, useEffect } from 'react'

export interface SpeechToTextOptions {
  language?: string
  message: string
  sending: boolean
  isSpeaking: boolean
  onSendVoiceMessage: (text: string) => Promise<any>
  onSetMessage: (text: string) => void
}

export function useSpeechToText({
  language,
  message,
  sending,
  isSpeaking,
  onSendVoiceMessage,
  onSetMessage
}: SpeechToTextOptions) {
  const [isRecording, setIsRecording] = useState(false)
  const [voiceMode, setVoiceMode] = useState(false)
  const [voiceTranscript, setVoiceTranscript] = useState('')
  const [voiceAwaitingResponse, setVoiceAwaitingResponse] = useState(false)

  const recognitionRef = useRef<any>(null)
  const resolveStartRef = useRef<((started: boolean) => void) | null>(null)
  const silenceTimerRef = useRef<number | null>(null)
  const restartTimerRef = useRef<number | null>(null)

  const voiceModeRef = useRef(false)
  const sendingRef = useRef(false)
  const voiceAwaitingResponseRef = useRef(false)
  const isSpeakingRef = useRef(false)
  const messageRef = useRef(message)

  // Sync state to refs for event handlers
  useEffect(() => {
    voiceModeRef.current = voiceMode
    if (!voiceMode) {
      clearRestartTimer()
      setVoiceTranscript('')
    }
  }, [voiceMode])

  useEffect(() => {
    sendingRef.current = sending
  }, [sending])

  useEffect(() => {
    voiceAwaitingResponseRef.current = voiceAwaitingResponse
  }, [voiceAwaitingResponse])

  useEffect(() => {
    isSpeakingRef.current = isSpeaking
  }, [isSpeaking])

  useEffect(() => {
    messageRef.current = message
  }, [message])

  const clearRestartTimer = () => {
    if (restartTimerRef.current !== null) {
      window.clearTimeout(restartTimerRef.current)
      restartTimerRef.current = null
    }
  }

  const clearSilenceTimer = () => {
    if (silenceTimerRef.current !== null) {
      window.clearTimeout(silenceTimerRef.current)
      silenceTimerRef.current = null
    }
  }

  const stopRecognition = () => {
    clearRestartTimer()
    clearSilenceTimer()
    recognitionRef.current?.stop()
    setIsRecording(false)
    setVoiceMode(false)
  }

  const cancelRecognition = () => {
    clearRestartTimer()
    clearSilenceTimer()
    const recognition = recognitionRef.current
    recognitionRef.current = null
    resolveStartRef.current?.(false)
    resolveStartRef.current = null
    if (recognition) {
      recognition.onstart = recognition.onresult = recognition.onend = recognition.onerror = null
      try { recognition.abort() } catch {}
    }
    voiceModeRef.current = false
    setVoiceMode(false)
    setIsRecording(false)
    setVoiceTranscript('')
  }

  const scheduleVoiceListen = (delayMs = 350) => {
    clearRestartTimer()
    if (!voiceModeRef.current || sendingRef.current || voiceAwaitingResponseRef.current || isSpeakingRef.current) return
    restartTimerRef.current = window.setTimeout(() => {
      restartTimerRef.current = null
      startRecognition(true)
    }, delayMs)
  }

  const startRecognition = (autoSend = false): Promise<boolean> => {
    if (recognitionRef.current || sendingRef.current || voiceAwaitingResponseRef.current || isSpeakingRef.current) return Promise.resolve(false)

    const SpeechRecognitionApi = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition
    if (!SpeechRecognitionApi) {
      setVoiceTranscript('Speech recognition is unavailable in this browser. Use Chrome or Edge.')
      voiceModeRef.current = false
      setVoiceMode(true)
      return Promise.resolve(false)
    }

    let accumulatedTranscript = ''
    const started = new Promise<boolean>(resolve => { resolveStartRef.current = resolve })
    clearSilenceTimer()

    try {
      const recognition = new SpeechRecognitionApi()
      recognition.continuous = true
      recognition.interimResults = true
      recognition.lang = language === 'en' ? 'en-US' : 'th-TH'

      const resetSilenceTimeout = () => {
        clearSilenceTimer()
        if (autoSend) {
          silenceTimerRef.current = window.setTimeout(() => {
            recognition.stop()
          }, 2000)
        }
      }

      recognition.onstart = () => {
        if (recognitionRef.current !== recognition) return
        setIsRecording(true)
        setVoiceMode(true)
        resolveStartRef.current?.(true)
        resolveStartRef.current = null
      }
      recognition.onresult = (event: any) => {
        if (recognitionRef.current !== recognition) return
        let interimText = ''
        let finalText = ''
        for (let index = event.resultIndex; index < event.results.length; index += 1) {
          const transcript = event.results[index]?.[0]?.transcript ?? ''
          if (event.results[index]?.isFinal) {
            finalText += transcript
          } else {
            interimText += transcript
          }
        }
        const displayText = (finalText || interimText).trim()
        if (displayText) setVoiceTranscript(displayText)
        if (finalText.trim()) {
          accumulatedTranscript = finalText.trim()
        }
        if (displayText) {
          resetSilenceTimeout()
        }
      }
      recognition.onerror = (event: any) => {
        if (recognitionRef.current !== recognition) return
        resolveStartRef.current?.(false)
        resolveStartRef.current = null
        setVoiceTranscript(event.error === 'not-allowed' ? 'Microphone permission denied' : `Speech recognition error: ${event.error}`)
        voiceModeRef.current = false
        setVoiceMode(false)
        setIsRecording(false)
        clearSilenceTimer()
      }
      recognition.onend = () => {
        if (recognitionRef.current !== recognition) return
        resolveStartRef.current?.(false)
        resolveStartRef.current = null
        recognitionRef.current = null
        if (!autoSend) { voiceModeRef.current = false; setVoiceMode(false) }
        setIsRecording(false)
        clearSilenceTimer()
        const finalText = accumulatedTranscript.trim()
        if (finalText) {
          if (autoSend) {
            voiceAwaitingResponseRef.current = true
            setVoiceAwaitingResponse(true)
            onSendVoiceMessage(finalText)
              .catch((error: any) => console.error('Voice message failed', error))
              .finally(() => {
                voiceAwaitingResponseRef.current = false
                setVoiceAwaitingResponse(false)
                scheduleVoiceListen()
              })
          } else {
            onSetMessage(messageRef.current.trim() ? `${messageRef.current.trimEnd()} ${finalText}` : finalText)
          }
        } else {
          if (autoSend && voiceModeRef.current) scheduleVoiceListen()
        }
      }
      recognitionRef.current = recognition
      recognition.start()
      return started
    } catch (error) {
      resolveStartRef.current?.(false)
      resolveStartRef.current = null
      setVoiceTranscript(error instanceof Error ? error.message : String(error))
      recognitionRef.current = null
      setIsRecording(false)
      clearSilenceTimer()
      return Promise.resolve(false)
    }
  }

  // Auto-listen trigger
  useEffect(() => {
    if (!voiceMode || sending || voiceAwaitingResponse || isSpeaking || isRecording || recognitionRef.current) return
    scheduleVoiceListen()
  }, [voiceMode, sending, voiceAwaitingResponse, isSpeaking, isRecording])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      clearRestartTimer()
      clearSilenceTimer()
      resolveStartRef.current?.(false)
      resolveStartRef.current = null
      if (recognitionRef.current) {
        try {
          const recognition = recognitionRef.current
          recognition.onstart = recognition.onresult = recognition.onend = recognition.onerror = null
          recognition.abort()
        } catch (e) {
          // ignore
        }
      }
    }
  }, [])

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
    cancelRecognition,
    scheduleVoiceListen,
    clearRestartTimer
  }
}
