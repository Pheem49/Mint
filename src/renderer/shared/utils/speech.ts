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
    recognitionRef.current = null
    setIsRecording(false)
  }

  const scheduleVoiceListen = (delayMs = 350) => {
    clearRestartTimer()
    if (!voiceModeRef.current || sendingRef.current || voiceAwaitingResponseRef.current || isSpeakingRef.current) return
    restartTimerRef.current = window.setTimeout(() => {
      restartTimerRef.current = null
      startRecognition(true)
    }, delayMs)
  }

  const startRecognition = (autoSend: boolean) => {
    if (recognitionRef.current || sendingRef.current || voiceAwaitingResponseRef.current || isSpeakingRef.current) return

    const SpeechRecognitionApi = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition
    if (!SpeechRecognitionApi) {
      alert('Speech-to-text (Speech Recognition) is not supported in this browser/WebView. Please use Google Chrome or Microsoft Edge to enable voice conversation.')
      voiceModeRef.current = false
      setVoiceMode(false)
      return
    }

    let accumulatedTranscript = ''
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
        setIsRecording(true)
      }
      recognition.onresult = (event: any) => {
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
        console.error('Speech recognition error', event)
        setIsRecording(false)
        clearSilenceTimer()
      }
      recognition.onend = () => {
        recognitionRef.current = null
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
    } catch (error) {
      console.error('Failed to start speech recognition', error)
      recognitionRef.current = null
      setIsRecording(false)
      clearSilenceTimer()
    }
  }

  // Auto-listen trigger
  useEffect(() => {
    if (!voiceMode || sending || voiceAwaitingResponse || isSpeaking || isRecording) return
    scheduleVoiceListen()
  }, [voiceMode, sending, voiceAwaitingResponse, isSpeaking, isRecording])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      clearRestartTimer()
      clearSilenceTimer()
      if (recognitionRef.current) {
        try {
          recognitionRef.current.stop()
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
    scheduleVoiceListen,
    clearRestartTimer
  }
}
