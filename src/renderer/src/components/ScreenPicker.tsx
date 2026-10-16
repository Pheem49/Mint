import SelectField from '../../shared/components/SelectField'
import { useEffect, useRef, useState, type PointerEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { createFrameChangeTracker, sampleFrame } from './liveTranslateFrames'
import { LANGUAGE_KEY, LANGUAGES } from './liveTranslateLanguage'
import './ScreenPicker.css'

type Region = { x: number; y: number; width: number; height: number }
type Point = { x: number; y: number }
type CaptureGeometry = { x: number; y: number; scaleFactor: number; windowCapture: boolean }
type Phase = 'selecting' | 'live' | 'paused'
type Status = 'waiting' | 'watching' | 'translating' | 'unchanged' | 'error'
type ControlAction = { type: 'pause' | 'resume' | 'change-area' | 'language'; languageChoice?: string; customLanguage?: string }
const CAPTURE_INTERVAL_MS = 750

function point(event: PointerEvent<HTMLElement>): Point {
  return { x: Math.max(0, Math.min(window.innerWidth, event.clientX)), y: Math.max(0, Math.min(window.innerHeight, event.clientY)) }
}

function regionBetween(start: Point, end: Point): Region {
  return { x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width: Math.abs(end.x - start.x), height: Math.abs(end.y - start.y) }
}

function captureRect(region: Region, geometry: CaptureGeometry): Region {
  return {
    x: Math.round(geometry.x + region.x * geometry.scaleFactor),
    y: Math.round(geometry.y + region.y * geometry.scaleFactor),
    width: Math.max(1, Math.round(region.width * geometry.scaleFactor)),
    height: Math.max(1, Math.round(region.height * geometry.scaleFactor)),
  }
}

export default function ScreenPicker() {
  const [geometry, setGeometry] = useState<CaptureGeometry | null>(null)
  const [phase, setPhase] = useState<Phase>('selecting')
  const [selection, setSelection] = useState<Region | null>(null)
  const [status, setStatus] = useState<Status>('waiting')
  const [interactive, setInteractive] = useState(true)
  const [translation, setTranslation] = useState('')
  const [translationBackdrop, setTranslationBackdrop] = useState('')
  const [error, setError] = useState('')
  const [savedLanguage] = useState(() => window.localStorage.getItem(LANGUAGE_KEY) || 'Thai')
  const [languageChoice, setLanguageChoice] = useState(() => LANGUAGES.includes(savedLanguage) ? savedLanguage : 'custom')
  const [customLanguage, setCustomLanguage] = useState(() => LANGUAGES.includes(savedLanguage) ? '' : savedLanguage)
  const dragStart = useRef<Point | null>(null)
  const previousPhase = useRef<Phase | null>(null)
  const session = useRef(0)
  const targetLanguage = languageChoice === 'custom' ? customLanguage.trim() : languageChoice
  const selectedPixels = selection && geometry ? captureRect(selection, geometry) : null

  useEffect(() => {
    let active = true
    const refreshGeometry = () => {
      void invoke<CaptureGeometry>('live_translate_geometry').then((value) => {
        if (active) setGeometry(value)
      }).catch((reason) => {
        if (active) {
          setError(reason instanceof Error ? reason.message : String(reason))
          setStatus('error')
        }
      })
    }
    refreshGeometry()
    window.addEventListener('resize', refreshGeometry)
    return () => {
      active = false
      window.removeEventListener('resize', refreshGeometry)
    }
  }, [])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void window.screenPickerApi?.closePicker()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | undefined
    let active = true
    void import('@tauri-apps/api/event').then(({ listen }) =>
      listen<boolean>('live-translate-interaction-mode', (event) => {
        if (active) setInteractive(event.payload)
      }).then((stop) => { if (active) unlisten = stop; else stop() }),
    ).catch((reason) => {
      if (active) setError(reason instanceof Error ? reason.message : String(reason))
    })
    return () => {
      active = false
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | undefined
    let active = true
    void import('@tauri-apps/api/event').then(({ listen }) =>
      listen<ControlAction>('live-translate-control', (event) => {
        if (!active) return
        const action = event.payload
        if (action.type === 'pause') setPhase('paused')
        if (action.type === 'resume') setPhase('live')
        if (action.type === 'change-area') {
          setPhase('selecting')
          setTranslation('')
          setTranslationBackdrop('')
          setError('')
        }
        if (action.type === 'language') {
          if (action.languageChoice) setLanguageChoice(action.languageChoice)
          if (action.customLanguage !== undefined) setCustomLanguage(action.customLanguage)
          setTranslation('')
        }
      }).then((stop) => { if (active) unlisten = stop; else stop() }),
    ).catch((reason) => {
      if (active) setError(reason instanceof Error ? reason.message : String(reason))
    })
    return () => { active = false; unlisten?.() }
  }, [])

  useEffect(() => {
    const prior = previousPhase.current
    previousPhase.current = phase
    if (phase === 'paused' || (phase === 'live' && prior === 'paused')) return
    const enabled = phase === 'live'
    let active = true
    const applyPhase = async () => {
      if (phase === 'selecting' && prior && prior !== 'selecting') {
        try {
          await invoke('close_desktop_window', { label: 'live-translate-controls' })
        } catch (reason) {
          if (active) setError(reason instanceof Error ? reason.message : String(reason))
        }
      }
      try {
        await invoke('set_live_translate_passthrough', { enabled })
        if (active) setInteractive(!enabled)
      } catch (reason) {
        if (!active) return
        setInteractive(true)
        setError(reason instanceof Error ? reason.message : String(reason))
        setStatus('error')
        if (enabled) setPhase('selecting')
      }
    }
    void applyPhase()
    return () => { active = false }
  }, [phase])

  useEffect(() => {
    if (targetLanguage) window.localStorage.setItem(LANGUAGE_KEY, targetLanguage)
  }, [targetLanguage])

  useEffect(() => {
    if (phase !== 'live' || interactive || !selectedPixels || !targetLanguage) return
    const currentSession = ++session.current
    let timer: ReturnType<typeof setTimeout> | undefined
    let translating = false
    let queued: string | null = null
    let hasChanged = createFrameChangeTracker()
    let retryAt = 0
    let retryChanged = false
    let translationFailed = false
    let stopped = false
    const rect = selectedPixels

    const translate = async (image: string) => {
      if (translating) { queued = image; return }
      translating = true
      setStatus('translating')
      let failed = false
      try {
        const result = await invoke<string>('translate_captured_frame', { image, targetLanguage })
        translationFailed = false
        if (!stopped && session.current === currentSession && !queued) {
          setTranslation(result.trim())
          setTranslationBackdrop(image)
          setError('')
        }
      } catch (reason) {
        failed = true
        translationFailed = true
        retryChanged = queued !== null
        queued = null
        retryAt = Date.now() + 5000
        if (!stopped && session.current === currentSession) {
          setError(reason instanceof Error ? reason.message : String(reason))
          setStatus('error')
        }
      } finally {
        translating = false
        if (!stopped && session.current === currentSession) {
          if (queued) {
            const latest = queued
            queued = null
            void translate(latest)
          } else if (!failed) setStatus('watching')
        }
      }
    }

    const poll = async () => {
      if (stopped) return
      let captureFailed = false
      let captureRetryDelay = 5000
      try {
        const image = await invoke<string>('capture_translation_frame', { rect })
        if (stopped || session.current !== currentSession) return
        const current = await sampleFrame(image)
        if (stopped || session.current !== currentSession) return
        if (Date.now() >= retryAt) {
          const changed = hasChanged(current)
          if (changed || retryChanged) {
            retryChanged = false
            setTranslation('')
            void translate(image)
          }
          else if (!translating && !translationFailed) setStatus('unchanged')
        }
      } catch (reason) {
        captureFailed = true
        if (!stopped && session.current === currentSession) {
          const message = reason instanceof Error ? reason.message : String(reason)
          if (message.includes('Click the game') || message.includes('outside the active game') || message.includes('No source window')) captureRetryDelay = CAPTURE_INTERVAL_MS
          setError(message)
          setStatus('error')
        }
      } finally {
        if (!stopped && session.current === currentSession) {
          timer = window.setTimeout(poll, captureFailed ? captureRetryDelay : CAPTURE_INTERVAL_MS)
        }
      }
    }

    void poll()
    return () => {
      stopped = true
      session.current += 1
      if (timer) window.clearTimeout(timer)
    }
  }, [phase, interactive, selectedPixels?.x, selectedPixels?.y, selectedPixels?.width, selectedPixels?.height, targetLanguage])

  const startSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (phase !== 'selecting' || !geometry) return
    dragStart.current = point(event)
    setSelection(null)
    setError('')
    setStatus('waiting')
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  const moveSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (dragStart.current) setSelection(regionBetween(dragStart.current, point(event)))
  }

  const finishSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (!dragStart.current) return
    const next = regionBetween(dragStart.current, point(event))
    dragStart.current = null
    const roomForResult = geometry?.windowCapture || Math.max(next.y, window.innerHeight - next.y - next.height) >= 88
    if (next.width < 120 || next.height < 72) {
      setError('Select an area at least 120 × 72 px.')
      setStatus('error')
      setSelection(null)
    } else if (!roomForResult) {
      setError('Leave a little space above or below the frame for the translation.')
      setStatus('error')
      setSelection(null)
    } else {
      setSelection(next)
    }
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }

  const begin = async () => {
    if (!selectedPixels || !targetLanguage) return
    try {
      await invoke('open_live_translate_controls')
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
      setStatus('error')
      return
    }
    setTranslation('')
    setTranslationBackdrop('')
    setError('')
    setStatus('waiting')
    setPhase('live')
  }

  const returnToSource = () => {
    void invoke('set_live_translate_passthrough', { enabled: true }).catch((reason) => {
      setError(reason instanceof Error ? reason.message : String(reason))
      setStatus('error')
    })
  }

  const resultStyle = (() => {
    if (!selection) return undefined
    if (geometry?.windowCapture) {
      return {
        left: selection.x,
        top: selection.y,
        width: selection.width,
        height: selection.height,
      }
    }
    const below = window.innerHeight - (selection.y + selection.height) - 16
    const above = selection.y - 16
    const placeBelow = below >= 80 || below >= above
    const maxHeight = Math.min(180, Math.max(0, placeBelow ? below : above))
    return {
      left: selection.x,
      top: placeBelow ? selection.y + selection.height + 8 : selection.y - maxHeight - 8,
      width: Math.min(window.innerWidth - selection.x - 16, Math.max(220, selection.width)),
      maxHeight,
    }
  })()

  return (
    <div className="live-translate">
      {phase === 'selecting' && (
        <div className="live-translate-picker" onPointerDown={startSelection} onPointerMove={moveSelection} onPointerUp={finishSelection} onPointerCancel={finishSelection}>
          {!geometry && <div className="live-translate-loading">{error || 'Preparing live screen…'}</div>}
          {selection && <div className="live-translate-selection" style={{ left: selection.x, top: selection.y, width: selection.width, height: selection.height }} />}
          {!selection && <div className="live-translate-crosshair" aria-hidden="true" />}
        </div>
      )}

      {phase !== 'selecting' && selection && (
        <>
          <div className="live-translate-live-frame" style={{ left: selection.x, top: selection.y, width: selection.width, height: selection.height }} aria-hidden="true" />
          {translation && <div className="live-translate-result" style={resultStyle} aria-live="polite" aria-label={`Translation to ${targetLanguage}`}>
            {geometry?.windowCapture && translationBackdrop && <img className="live-translate-result-backdrop" src={translationBackdrop} alt="" aria-hidden="true" />}
            <div className="live-translate-result-text">{translation}</div>
          </div>}
        </>
      )}

      {(phase === 'selecting' || interactive) && <div className="live-translate-toolbar">
        <div className="live-translate-brand"><span aria-hidden="true">◈</span><div><strong>Live translate</strong><small>{phase === 'selecting' ? 'Draw a frame around the text' : interactive ? 'Controls active · click-through off' : 'Click-through on · Alt+Shift+L for controls'}</small></div></div>
        <label className="live-translate-language">To
          <SelectField fullWidth={false} value={languageChoice} onValueChange={(nextValue) => setLanguageChoice(nextValue)} aria-label="Target language">
            {LANGUAGES.map((language) => <option key={language} value={language}>{language}</option>)}
            <option value="custom">Other language…</option>
          </SelectField>
        </label>
        {languageChoice === 'custom' && <input className="live-translate-custom-language" value={customLanguage} onChange={(event) => setCustomLanguage(event.target.value)} maxLength={64} placeholder="Language name" aria-label="Custom target language" />}
        {phase === 'selecting' ? (
          <button type="button" className="live-translate-primary" onClick={() => void begin()} disabled={!selectedPixels || !targetLanguage}>Translate this area</button>
        ) : (
          <>
            {interactive && <button type="button" className="live-translate-primary" onClick={returnToSource}>Return to game or book</button>}
            <button type="button" onClick={() => setPhase(phase === 'live' ? 'paused' : 'live')}>{phase === 'live' ? 'Pause' : 'Resume'}</button>
            <button type="button" onClick={() => { setPhase('selecting'); setTranslation(''); setError('') }}>Change area</button>
          </>
        )}
        <button type="button" className="live-translate-close" onClick={() => void window.screenPickerApi?.closePicker()} aria-label="Close live translate">×</button>
      </div>}
      {phase === 'selecting' && <div className="live-translate-help">{selection ? `${Math.round(selection.width)} × ${Math.round(selection.height)} px selected` : 'Drag a frame around text on your screen (at least 120 × 72 px)'} · Esc to close</div>}
      {status === 'error' && error && <div className="live-translate-error" role="alert">{error}</div>}
    </div>
  )
}
