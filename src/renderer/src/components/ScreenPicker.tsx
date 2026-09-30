import { useEffect, useRef, useState, type PointerEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { hasMeaningfulFrameChange, sampleFrame } from './liveTranslateFrames'
import './ScreenPicker.css'

type Region = { x: number; y: number; width: number; height: number }
type Point = { x: number; y: number }
type Phase = 'selecting' | 'live' | 'paused'
type Status = 'waiting' | 'watching' | 'translating' | 'unchanged' | 'error'

const LANGUAGE_KEY = 'mint:live-translate-language'
const LANGUAGES = ['Thai', 'English', 'Japanese', 'Chinese', 'Korean', 'Spanish', 'French', 'German', 'Portuguese', 'Vietnamese', 'Indonesian', 'Arabic', 'Hindi']
const nextPaint = () => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))

function point(event: PointerEvent<HTMLElement>): Point {
  return { x: Math.max(0, Math.min(window.innerWidth, event.clientX)), y: Math.max(0, Math.min(window.innerHeight, event.clientY)) }
}

function regionBetween(start: Point, end: Point): Region {
  return { x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width: Math.abs(end.x - start.x), height: Math.abs(end.y - start.y) }
}

function captureRect(region: Region, image: { width: number; height: number }): Region {
  const scaleX = image.width / window.innerWidth
  const scaleY = image.height / window.innerHeight
  return {
    x: Math.round(region.x * scaleX),
    y: Math.round(region.y * scaleY),
    width: Math.max(1, Math.round(region.width * scaleX)),
    height: Math.max(1, Math.round(region.height * scaleY)),
  }
}

export default function ScreenPicker() {
  const [snapshot, setSnapshot] = useState('')
  const [sourceSize, setSourceSize] = useState<{ width: number; height: number } | null>(null)
  const [phase, setPhase] = useState<Phase>('selecting')
  const [selection, setSelection] = useState<Region | null>(null)
  const [status, setStatus] = useState<Status>('waiting')
  const [translation, setTranslation] = useState('')
  const [error, setError] = useState('')
  const [sampling, setSampling] = useState(false)
  const [savedLanguage] = useState(() => window.localStorage.getItem(LANGUAGE_KEY) || 'Thai')
  const [languageChoice, setLanguageChoice] = useState(() => LANGUAGES.includes(savedLanguage) ? savedLanguage : 'custom')
  const [customLanguage, setCustomLanguage] = useState(() => LANGUAGES.includes(savedLanguage) ? '' : savedLanguage)
  const dragStart = useRef<Point | null>(null)
  const session = useRef(0)
  const previewRequest = useRef<Promise<string | null> | null>(null)
  const targetLanguage = languageChoice === 'custom' ? customLanguage.trim() : languageChoice
  const selectedPixels = selection && sourceSize ? captureRect(selection, sourceSize) : null

  useEffect(() => {
    let active = true
    previewRequest.current ||= invoke<string | null>('take_screen_capture_preview')
    void previewRequest.current.then((image) => {
      if (!active) return
      if (image) setSnapshot(image)
      else setError('Screen preview is unavailable. Close and try again.')
    }).catch((reason) => {
      if (active) setError(reason instanceof Error ? reason.message : String(reason))
    })
    return () => { active = false }
  }, [])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void window.screenPickerApi?.closePicker()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  useEffect(() => {
    if (targetLanguage) window.localStorage.setItem(LANGUAGE_KEY, targetLanguage)
  }, [targetLanguage])

  useEffect(() => {
    if (phase !== 'live' || !selectedPixels || !targetLanguage) return
    const currentSession = ++session.current
    let timer: ReturnType<typeof setTimeout> | undefined
    let translating = false
    let queued: string | null = null
    let previous: Uint8ClampedArray | null = null
    let retryAt = 0
    let stopped = false
    const rect = selectedPixels

    const translate = async (image: string) => {
      if (translating) { queued = image; return }
      translating = true
      setStatus('translating')
      let failed = false
      try {
        const result = await invoke<string>('translate_captured_frame', { image, targetLanguage })
        if (!stopped && session.current === currentSession) {
          setTranslation(result.trim())
          setError('')
        }
      } catch (reason) {
        failed = true
        previous = null
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
      setSampling(true)
      try {
        // Remove translated text from the compositor before native capture.
        await nextPaint()
        if (stopped) return
        const image = await invoke<string>('capture_translation_frame', { rect })
        if (stopped || session.current !== currentSession) return
        const current = await sampleFrame(image)
        if (stopped || session.current !== currentSession) return
        if (Date.now() >= retryAt) {
          const changed = hasMeaningfulFrameChange(previous, current)
          previous = current
          if (changed) void translate(image)
          else if (!translating) setStatus('unchanged')
        }
      } catch (reason) {
        if (!stopped && session.current === currentSession) {
          setError(reason instanceof Error ? reason.message : String(reason))
          setStatus('error')
        }
      } finally {
        if (!stopped && session.current === currentSession) {
          setSampling(false)
          timer = window.setTimeout(poll, 1200)
        }
      }
    }

    void poll()
    return () => {
      stopped = true
      session.current += 1
      if (timer) window.clearTimeout(timer)
    }
  }, [phase, selectedPixels?.x, selectedPixels?.y, selectedPixels?.width, selectedPixels?.height, targetLanguage])

  const startSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (phase !== 'selecting' || !snapshot) return
    dragStart.current = point(event)
    setSelection(null)
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  const moveSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (dragStart.current) setSelection(regionBetween(dragStart.current, point(event)))
  }

  const finishSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (!dragStart.current) return
    const next = regionBetween(dragStart.current, point(event))
    dragStart.current = null
    setSelection(next.width >= 120 && next.height >= 72 ? next : null)
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }

  const begin = () => {
    if (!selectedPixels || !targetLanguage) return
    setTranslation('')
    setError('')
    setStatus('waiting')
    setPhase('live')
  }

  return (
    <div className={`live-translate ${sampling ? 'is-sampling' : ''} ${phase !== 'selecting' ? 'is-active' : ''}`}>
      {phase === 'selecting' && (
        <div className="live-translate-picker" onPointerDown={startSelection} onPointerMove={moveSelection} onPointerUp={finishSelection} onPointerCancel={finishSelection}>
          {snapshot ? <img src={snapshot} alt="Screen preview for selecting a translation area" draggable={false} onLoad={(event) => setSourceSize({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })} /> : <div className="live-translate-loading">{error || 'Preparing screen preview…'}</div>}
          {selection && <div className="live-translate-selection" style={{ left: selection.x, top: selection.y, width: selection.width, height: selection.height }} />}
        </div>
      )}

      {phase !== 'selecting' && selection && (
        <div className="live-translate-result" style={{ left: selection.x, top: selection.y, width: selection.width, height: selection.height }} aria-live="polite">
          <div className="live-translate-result-label">{phase === 'paused' ? 'Paused' : status === 'translating' ? 'Translating…' : status === 'error' ? 'Capture error' : `Live · ${targetLanguage}`}</div>
          <div className="live-translate-result-text">{translation || 'Waiting for text in this area…'}</div>
        </div>
      )}

      <div className="live-translate-toolbar">
        <div className="live-translate-brand"><span aria-hidden="true">◈</span><div><strong>Live translate</strong><small>{phase === 'selecting' ? 'Draw a frame around the text' : phase === 'paused' ? 'Translation paused' : status === 'translating' ? 'Updating translation' : status === 'error' ? 'Waiting to retry' : 'Watching selected area'}</small></div></div>
        <label className="live-translate-language">To
          <select value={languageChoice} onChange={(event) => setLanguageChoice(event.target.value)} aria-label="Target language">
            {LANGUAGES.map((language) => <option key={language} value={language}>{language}</option>)}
            <option value="custom">Other language…</option>
          </select>
        </label>
        {languageChoice === 'custom' && <input className="live-translate-custom-language" value={customLanguage} onChange={(event) => setCustomLanguage(event.target.value)} maxLength={64} placeholder="Language name" aria-label="Custom target language" />}
        {phase === 'selecting' ? (
          <button type="button" className="live-translate-primary" onClick={begin} disabled={!selectedPixels || !targetLanguage}>Start translation</button>
        ) : (
          <>
            <button type="button" onClick={() => setPhase(phase === 'live' ? 'paused' : 'live')}>{phase === 'live' ? 'Pause' : 'Resume'}</button>
            <button type="button" onClick={() => { setPhase('selecting'); setTranslation('') }}>Change area</button>
          </>
        )}
        <button type="button" className="live-translate-close" onClick={() => void window.screenPickerApi?.closePicker()} aria-label="Close live translate">×</button>
      </div>
      {phase === 'selecting' && <div className="live-translate-help">{selection ? `${Math.round(selection.width)} × ${Math.round(selection.height)} px selected` : 'Drag across the text to select an area (at least 120 × 72 px)'} · Only changed frames are sent for translation · Esc to close</div>}
      {phase !== 'selecting' && status === 'error' && <div className="live-translate-error" role="alert">{error}</div>}
    </div>
  )
}
