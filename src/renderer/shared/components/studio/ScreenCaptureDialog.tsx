import { useEffect, useRef, useState, type PointerEvent } from 'react'
import './ScreenCaptureDialog.css'

type Selection = { x: number; y: number; width: number; height: number }
type Point = { x: number; y: number }

interface Props {
  onClose: () => void
  onAttach: (image: string) => void
  capture: () => Promise<string>
  onLiveTranslate?: () => void
}

function pointInImage(event: PointerEvent, element: HTMLElement): Point {
  const bounds = element.getBoundingClientRect()
  return {
    x: Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)),
    y: Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height)),
  }
}

function selectionFromPoints(start: Point, end: Point): Selection {
  return {
    x: Math.min(start.x, end.x),
    y: Math.min(start.y, end.y),
    width: Math.abs(end.x - start.x),
    height: Math.abs(end.y - start.y),
  }
}

export default function ScreenCaptureDialog({ onClose, onAttach, capture, onLiveTranslate }: Props) {
  const [image, setImage] = useState('')
  const [imageReady, setImageReady] = useState(false)
  const [selection, setSelection] = useState<Selection | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [mode, setMode] = useState<'region' | 'full'>('region')
  const [attaching, setAttaching] = useState(false)
  const dragStart = useRef<Point | null>(null)
  const imageRef = useRef<HTMLImageElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)
  const closeRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    closeRef.current?.focus()
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
      if (event.key !== 'Tab') return
      const buttons = Array.from(dialogRef.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])
      if (!buttons.length) return
      const first = buttons[0]
      const last = buttons[buttons.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      previousFocus?.focus()
    }
  }, [onClose])

  const takeCapture = async () => {
    if (busy) return
    setBusy(true)
    setError('')
    setSelection(null)
    setImage('')
    setImageReady(false)
    try {
      const result = await capture()
      if (!result.startsWith('data:image/')) throw new Error('The screen capture returned no image.')
      setImage(result)
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason)
      setError(message || 'Screen capture failed. Please try again.')
    } finally {
      setBusy(false)
    }
  }

  const startSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (mode !== 'region' || !image) return
    const start = pointInImage(event, event.currentTarget)
    dragStart.current = start
    setSelection(null)
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  const moveSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (!dragStart.current) return
    setSelection(selectionFromPoints(dragStart.current, pointInImage(event, event.currentTarget)))
  }

  const finishSelection = (event: PointerEvent<HTMLDivElement>) => {
    if (!dragStart.current) return
    const next = selectionFromPoints(dragStart.current, pointInImage(event, event.currentTarget))
    dragStart.current = null
    setSelection(next.width > 0.003 && next.height > 0.003 ? next : null)
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }

  const attach = () => {
    if (!image || !imageReady || attaching || (mode === 'region' && !selection)) return
    setAttaching(true)
    try {
      if (mode === 'full') {
        onAttach(image)
        return
      }
      const source = imageRef.current
      if (!source || !source.naturalWidth || !source.naturalHeight || !selection) {
        throw new Error('The screenshot is still loading. Please try again.')
      }
      const canvas = document.createElement('canvas')
      const x = Math.floor(selection.x * source.naturalWidth)
      const y = Math.floor(selection.y * source.naturalHeight)
      canvas.width = Math.max(1, Math.min(source.naturalWidth - x, Math.ceil(selection.width * source.naturalWidth)))
      canvas.height = Math.max(1, Math.min(source.naturalHeight - y, Math.ceil(selection.height * source.naturalHeight)))
      const context = canvas.getContext('2d')
      if (!context) throw new Error('Unable to prepare the selected image.')
      context.drawImage(source, x, y, canvas.width, canvas.height, 0, 0, canvas.width, canvas.height)
      onAttach(canvas.toDataURL('image/png'))
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
      setAttaching(false)
    }
  }

  return (
    <div ref={dialogRef} className={`screen-capture-dialog ${busy ? 'is-capturing' : ''}`} role="dialog" aria-modal="true" aria-labelledby="screen-capture-title">
      <div className="screen-capture-panel">
        <header className="screen-capture-header">
          <div>
            <p className="screen-capture-eyebrow">Mint Agent · Screen capture</p>
            <h2 id="screen-capture-title">Choose what Mint can see</h2>
            <p>Only the image you attach will be included in your message.</p>
          </div>
          <button ref={closeRef} type="button" className="screen-capture-close" onClick={onClose} aria-label="Close screen capture">×</button>
        </header>

        <div className="screen-capture-body">
          {image ? (
            <div className="screen-capture-stage">
              <div
                className={`screen-capture-image-wrap ${mode === 'region' ? 'is-selecting' : ''}`}
                onPointerDown={startSelection}
                onPointerMove={moveSelection}
                onPointerUp={finishSelection}
                onPointerCancel={finishSelection}
              >
                <img
                  ref={imageRef}
                  src={image}
                  alt="Captured screen preview"
                  draggable={false}
                  onLoad={() => setImageReady(true)}
                  onError={() => { setImage(''); setError('Unable to display the screenshot. Please capture it again.') }}
                />
                {mode === 'region' && selection && (
                  <div className="screen-capture-selection" style={{ left: `${selection.x * 100}%`, top: `${selection.y * 100}%`, width: `${selection.width * 100}%`, height: `${selection.height * 100}%` }} />
                )}
              </div>
            </div>
          ) : (
            <div className="screen-capture-empty">
              <span className="screen-capture-empty-icon" aria-hidden="true">▣</span>
              <h3>{busy ? 'Waiting for your screen…' : 'Capture your screen'}</h3>
              <p>{busy ? 'Choose the screen or window in your system prompt.' : 'You can review and crop it before attaching it to the chat.'}</p>
              {!busy && <button type="button" className="screen-capture-primary" onClick={takeCapture}>Capture screen</button>}
            </div>
          )}
        </div>

        {error && <p className="screen-capture-error" role="alert">{error}</p>}

        <footer className="screen-capture-footer">
          <div className="screen-capture-modes" role="group" aria-label="Capture area">
            <button type="button" className={mode === 'region' ? 'active' : ''} onClick={() => setMode('region')} aria-pressed={mode === 'region'}>Select area</button>
            <button type="button" className={mode === 'full' ? 'active' : ''} onClick={() => setMode('full')} aria-pressed={mode === 'full'}>Full screen</button>
          </div>
          <p className="screen-capture-instruction">{mode === 'region' ? 'Drag across the preview to select an area.' : 'The entire captured image will be attached.'}</p>
          <div className="screen-capture-actions">
            {onLiveTranslate && <button type="button" className="screen-capture-secondary" onClick={onLiveTranslate}>Live translate</button>}
            {image && <button type="button" className="screen-capture-secondary" onClick={takeCapture} disabled={busy || attaching}>Retake</button>}
            <button type="button" className="screen-capture-secondary" onClick={onClose}>Cancel</button>
            <button type="button" className="screen-capture-primary" onClick={attach} disabled={!image || !imageReady || busy || attaching || (mode === 'region' && !selection)}>Attach to chat</button>
          </div>
        </footer>
      </div>
    </div>
  )
}
