import { useEffect, useRef, useState, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { Mic, MicOff, PhoneOff, Volume2 } from 'lucide-react'
import { VoicePoweredOrb } from './ui/voice-powered-orb'
import { Button } from './ui/button'
import './GeminiLiveOverlay.css'

export interface GeminiLiveOverlayProps {
  status: 'connecting' | 'listening' | 'speaking' | 'thinking' | 'paused' | 'error'
  userTranscript: string
  assistantTranscript: string
  isPaused: boolean
  onTogglePause: () => void
  onEndCall: () => void
  voice: string
  voices: readonly string[]
  onChangeVoice: (voice: string) => void
  analyserRef?: RefObject<AnalyserNode | null>
}
const STATUS_LABEL: Record<GeminiLiveOverlayProps['status'], string> = {
  connecting: 'Connecting', listening: 'Listening', speaking: 'Speaking',
  thinking: 'Thinking', paused: 'Microphone paused', error: 'Connection unavailable',
}
const STATUS_HINT: Record<GeminiLiveOverlayProps['status'], string> = {
  connecting: 'Opening your microphone and Live session…',
  listening: 'Speak naturally. Mint is listening.',
  speaking: 'Mint is speaking. You can interrupt at any time.',
  thinking: 'Waiting for Mint’s response…',
  paused: 'Resume the microphone when you’re ready.',
  error: 'End this call and try again when you’re ready.',
}

export default function GeminiLiveOverlay({
  status, userTranscript, assistantTranscript, isPaused, onTogglePause, onEndCall,
  voice, voices, onChangeVoice, analyserRef,
}: GeminiLiveOverlayProps) {
  const [voicePickerOpen, setVoicePickerOpen] = useState(false)
  const dialogRef = useRef<HTMLDivElement>(null)
  const endCall = useRef(onEndCall)
  endCall.current = onEndCall
  // The decorative orb must not own a microphone in this call surface.
  const emptyAnalyser = useRef<AnalyserNode | null>(null)
  useEffect(() => {
    const previousFocus = document.activeElement as HTMLElement | null
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    const dialog = dialogRef.current!
    dialog.querySelector<HTMLButtonElement>('[data-call-control]:not(:disabled), [aria-label="End Live conversation"]')?.focus()
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); endCall.current(); return }
      if (event.key !== 'Tab') return
      const controls = Array.from(dialog.querySelectorAll<HTMLElement>('button:not(:disabled), [tabindex="0"]'))
      const first = controls[0], last = controls[controls.length - 1]
      if (event.shiftKey && (document.activeElement === first || !dialog.contains(document.activeElement))) {
        event.preventDefault(); last?.focus()
      } else if (!event.shiftKey && (document.activeElement === last || !dialog.contains(document.activeElement))) {
        event.preventDefault(); first?.focus()
      }
    }
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('keydown', onKey)
      document.body.style.overflow = previousOverflow
      previousFocus?.focus({ preventScroll: true })
    }
  }, [])

  const dialog = (
    <div ref={dialogRef} className="gemini-live-overlay live-orb-dialog" role="dialog" aria-modal="true" aria-label="Gemini Live conversation" data-status={status}>
      <div className="gemini-live-card">
        <header className="live-orb-header">
          <span className="gemini-live-badge">Gemini Live</span>
          <h2>Live conversation</h2>
        </header>
        <div className="live-orb-stage">
          <VoicePoweredOrb hue={0} enableVoiceControl={status === 'listening' || status === 'speaking' || status === 'thinking'} analyserRef={analyserRef ?? emptyAnalyser} />
        </div>
        <div className="live-orb-state">
          <div className="gemini-live-status" role="status">{STATUS_LABEL[status]}</div>
          <p>{STATUS_HINT[status]}</p>
        </div>
        <div className="gemini-live-transcript" aria-label="Conversation transcript" tabIndex={0}>
          {userTranscript || assistantTranscript ? <>
            {userTranscript && <p className="gemini-live-transcript-line"><span className="gemini-live-transcript-speaker">You</span>{userTranscript}</p>}
            {assistantTranscript && <p className="gemini-live-transcript-line"><span className="gemini-live-transcript-speaker">Mint</span>{assistantTranscript}</p>}
          </> : <p className="live-orb-empty">{status === 'connecting' ? 'Your conversation will appear here.' : status === 'error' ? 'No conversation yet.' : 'Say something to get started…'}</p>}
        </div>
        <div className="gemini-live-voice-picker">
          <Button type="button" variant="ghost" className="gemini-live-voice-btn" aria-label="Change voice" aria-expanded={voicePickerOpen} aria-haspopup="listbox" onClick={() => setVoicePickerOpen(open => !open)}>
            <Volume2 size={16} aria-hidden="true" /><span>{voice}</span>
          </Button>
          {voicePickerOpen && <div className="gemini-live-voice-menu" role="listbox" aria-label="Live voice">
            {voices.map(voiceName => <button key={voiceName} type="button" role="option" aria-selected={voiceName === voice} data-voice={voiceName} className={`gemini-live-voice-option ${voiceName === voice ? 'active' : ''}`} onClick={() => {
              setVoicePickerOpen(false)
              if (voiceName !== voice) onChangeVoice(voiceName)
            }}>{voiceName}</button>)}
          </div>}
        </div>
        <div className="gemini-live-controls">
          <Button type="button" data-call-control variant="secondary" size="lg" className="gemini-live-btn" onClick={onTogglePause} disabled={status === 'connecting' || status === 'error'} aria-label={isPaused ? 'Resume microphone' : 'Pause microphone'} aria-pressed={isPaused}>
            {isPaused ? <Mic size={18} aria-hidden="true" /> : <MicOff size={18} aria-hidden="true" />}<span>{isPaused ? 'Resume' : 'Pause mic'}</span>
          </Button>
          <Button type="button" variant="destructive" size="lg" className="gemini-live-btn gemini-live-end" onClick={onEndCall} aria-label="End Live conversation">
            <PhoneOff size={18} aria-hidden="true" /><span>End call</span>
          </Button>
        </div>
      </div>
    </div>
  )
  // Escape chat layout containment (transforms, filters and clipped panels).
  return typeof document === 'undefined' ? dialog : createPortal(dialog, document.body)
}
