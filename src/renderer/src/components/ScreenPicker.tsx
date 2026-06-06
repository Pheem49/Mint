import React, { useEffect, useRef, useState } from 'react'

interface Rect {
  startX: number
  startY: number
  currentX: number
  currentY: number
  width: number
  height: number
}

interface NormalRect {
  x: number
  y: number
  width: number
  height: number
}

export default function ScreenPicker() {
  const bgCanvasRef = useRef<HTMLCanvasElement>(null)
  const overlayCanvasRef = useRef<HTMLCanvasElement>(null)
  const translationBoxRef = useRef<HTMLDivElement>(null)
  
  const [isDrawing, setIsDrawing] = useState(false)
  const [coords, setCoords] = useState<Rect>({ startX: 0, startY: 0, currentX: 0, currentY: 0, width: 0, height: 0 })
  const [baseImage, setBaseImage] = useState<HTMLImageElement | null>(null)
  const [selectedRect, setSelectedRect] = useState<NormalRect | null>(null)
  
  const [isTranslateMode, setIsTranslateMode] = useState(false)
  const [isContinuousTranslateActive, setIsContinuousTranslateActive] = useState(false)
  const [translationText, setTranslationText] = useState('')
  const [translationPos, setTranslationPos] = useState({ left: 0, top: 0, maxWidth: 400 })

  const baseImageRef = useRef<HTMLImageElement | null>(null)
  const isOverlayInteractableRef = useRef(true)
  const screenshotRequestedRef = useRef(false)

  // Initialize canvases and listen to screenshots
  useEffect(() => {
    const loadScreenshot = (base64Data: string) => {
      const img = new Image()
      img.onload = () => {
        baseImageRef.current = img
        setBaseImage(img)
        const bg = bgCanvasRef.current
        if (bg) {
          bg.width = window.innerWidth
          bg.height = window.innerHeight
          const bgCtx = bg.getContext('2d')
          bgCtx?.drawImage(img, 0, 0, bg.width, bg.height)
          drawDarkOverlay()
        }
      }
      img.src = base64Data
    }

    const handleResize = () => {
      const bg = bgCanvasRef.current
      const overlay = overlayCanvasRef.current
      if (bg && overlay) {
        bg.width = window.innerWidth
        bg.height = window.innerHeight
        overlay.width = window.innerWidth
        overlay.height = window.innerHeight
        if (baseImageRef.current) {
          const bgCtx = bg.getContext('2d')
          bgCtx?.drawImage(baseImageRef.current, 0, 0, bg.width, bg.height)
        }
        drawDarkOverlay()
      }
    }

    if (window.screenPickerApi) {
      const pendingCapture = window.localStorage.getItem('mint:pending-screen-capture')
      if (pendingCapture) {
        window.localStorage.removeItem('mint:pending-screen-capture')
        screenshotRequestedRef.current = true
        loadScreenshot(pendingCapture)
      } else if (!screenshotRequestedRef.current) {
        screenshotRequestedRef.current = true
        window.screenPickerApi.onScreenshot(loadScreenshot)
      }

      window.screenPickerApi.onTranslationResult((thaiText) => {
        setTranslationText(thaiText)
      })
    }

    window.addEventListener('resize', handleResize)
    handleResize()

    return () => {
      window.removeEventListener('resize', handleResize)
    }
  }, [])

  const drawDarkOverlay = () => {
    const overlay = overlayCanvasRef.current
    if (!overlay) return
    const overlayCtx = overlay.getContext('2d')
    if (!overlayCtx) return
    overlayCtx.clearRect(0, 0, overlay.width, overlay.height)
    overlayCtx.fillStyle = 'rgba(0, 0, 0, 0.5)'
    overlayCtx.fillRect(0, 0, overlay.width, overlay.height)
  }

  const normalizeRect = (r: Rect): NormalRect => {
    return {
      x: Math.min(r.startX, r.currentX),
      y: Math.min(r.startY, r.currentY),
      width: Math.abs(r.width),
      height: Math.abs(r.height)
    }
  }

  const drawSelectionOutline = (rect: NormalRect) => {
    const overlay = overlayCanvasRef.current
    if (!overlay || !rect || rect.width === 0 || rect.height === 0) return
    const overlayCtx = overlay.getContext('2d')
    if (!overlayCtx) return
    overlayCtx.clearRect(0, 0, overlay.width, overlay.height)
    overlayCtx.strokeStyle = isTranslateMode ? '#8b5cf6' : '#00ff88'
    overlayCtx.lineWidth = 3
    overlayCtx.strokeRect(rect.x, rect.y, rect.width, rect.height)
  }

  const drawSelection = (currentCoords: Rect) => {
    const overlay = overlayCanvasRef.current
    if (!overlay) return
    const overlayCtx = overlay.getContext('2d')
    if (!overlayCtx) return

    drawDarkOverlay()

    const rect = normalizeRect(currentCoords)
    overlayCtx.clearRect(rect.x, rect.y, rect.width, rect.height)
    overlayCtx.strokeStyle = isTranslateMode ? '#8b5cf6' : '#00ff88'
    overlayCtx.lineWidth = 2
    overlayCtx.strokeRect(rect.x, rect.y, rect.width, rect.height)
  }

  const resetSelectionOverlay = () => {
    setSelectedRect(null)
    const overlay = overlayCanvasRef.current
    if (overlay) overlay.style.pointerEvents = 'auto'
    isOverlayInteractableRef.current = true
    drawDarkOverlay()
  }

  const setOverlayInteractable = (isInteractable: boolean) => {
    if (isOverlayInteractableRef.current === isInteractable) return
    isOverlayInteractableRef.current = isInteractable
    window.screenPickerApi?.setOverlayInteractable(isInteractable)
  }

  const stopTranslationMode = () => {
    setIsContinuousTranslateActive(false)
    const overlay = overlayCanvasRef.current
    if (overlay) overlay.style.pointerEvents = 'auto'
    window.screenPickerApi?.stopContinuousTranslation()
    setOverlayInteractable(true)

    const bg = bgCanvasRef.current
    if (bg && baseImage) {
      const bgCtx = bg.getContext('2d')
      bgCtx?.clearRect(0, 0, bg.width, bg.height)
      bgCtx?.drawImage(baseImage, 0, 0, bg.width, bg.height)
    }

    resetSelectionOverlay()
  }

  const setTranslationBoxPosition = (rect: NormalRect) => {
    const margin = 10
    const boxWidth = Math.min(400, Math.max(240, rect.width))
    const left = Math.max(margin, Math.min(rect.x, window.innerWidth - boxWidth - margin))

    // Estimate translation box height
    const boxHeight = translationBoxRef.current?.offsetHeight || 80
    const preferredTop = rect.y + rect.height + margin
    const fallbackTop = Math.max(margin, rect.y - margin - boxHeight)
    const top = preferredTop + boxHeight <= window.innerHeight ? preferredTop : fallbackTop

    setTranslationPos({ left, top, maxWidth: boxWidth })
  }

  const cropAndSend = (rect: Rect) => {
    if (rect.width === 0 || rect.height === 0 || !baseImage) return
    const { x, y, width: w, height: h } = normalizeRect(rect)

    const bg = bgCanvasRef.current
    if (!bg) return

    const scaleX = baseImage.width / bg.width
    const scaleY = baseImage.height / bg.height

    const cropX = x * scaleX
    const cropY = y * scaleY
    const cropW = w * scaleX
    const cropH = h * scaleY

    const cropCanvas = document.createElement('canvas')
    cropCanvas.width = cropW
    cropCanvas.height = cropH
    const cropCtx = cropCanvas.getContext('2d')
    if (!cropCtx) return

    cropCtx.drawImage(baseImage, cropX, cropY, cropW, cropH, 0, 0, cropW, cropH)
    const croppedBase64 = cropCanvas.toDataURL('image/png')

    if (isTranslateMode) {
      setIsContinuousTranslateActive(true)
      const normRect = { x, y, width: w, height: h }
      setSelectedRect(normRect)

      const bg = bgCanvasRef.current
      if (bg) {
        const bgCtx = bg.getContext('2d')
        bgCtx?.clearRect(0, 0, bg.width, bg.height)
      }
      
      const overlay = overlayCanvasRef.current
      if (overlay) overlay.style.pointerEvents = 'none'
      
      drawSelectionOutline(normRect)
      setTranslationText('Auto-Translating...')
      setTranslationBoxPosition(normRect)
      setOverlayInteractable(false)

      window.screenPickerApi?.startContinuousTranslation(normRect)
    } else {
      window.screenPickerApi?.sendSelection(croppedBase64)
    }
  }

  // Keyboard controls
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (isContinuousTranslateActive) {
          e.preventDefault()
          stopTranslationMode()
        } else {
          window.screenPickerApi?.closePicker()
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isContinuousTranslateActive])

  // Mouse move tracker for translation box interactivity
  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isContinuousTranslateActive) return
      const box = translationBoxRef.current
      if (!box) return

      const rect = box.getBoundingClientRect()
      const isInsideBox =
        e.clientX >= rect.left &&
        e.clientX <= rect.right &&
        e.clientY >= rect.top &&
        e.clientY <= rect.bottom

      setOverlayInteractable(isInsideBox)
    }
    window.addEventListener('mousemove', handleMouseMove)
    return () => window.removeEventListener('mousemove', handleMouseMove)
  }, [isContinuousTranslateActive])

  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (isContinuousTranslateActive) return
    setIsDrawing(true)
    const newCoords = {
      startX: e.clientX,
      startY: e.clientY,
      currentX: e.clientX,
      currentY: e.clientY,
      width: 0,
      height: 0
    }
    setCoords(newCoords)
  }

  const handleCanvasMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!isDrawing) return
    const newCoords = {
      ...coords,
      currentX: e.clientX,
      currentY: e.clientY,
      width: e.clientX - coords.startX,
      height: e.clientY - coords.startY
    }
    setCoords(newCoords)
    drawSelection(newCoords)
  }

  const handleMouseUp = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!isDrawing) return
    setIsDrawing(false)
    const finalCoords = {
      ...coords,
      currentX: e.clientX,
      currentY: e.clientY,
      width: e.clientX - coords.startX,
      height: e.clientY - coords.startY
    }
    setCoords(finalCoords)
    cropAndSend(finalCoords)
  }

  const handleToggleTranslateMode = () => {
    const nextMode = !isTranslateMode
    setIsTranslateMode(nextMode)
    if (nextMode) {
      resetSelectionOverlay()
    } else {
      if (isContinuousTranslateActive) {
        stopTranslationMode()
      } else {
        resetSelectionOverlay()
      }
    }
  }

  const handleFullscreen = () => {
    if (baseImage && !isTranslateMode) {
      window.screenPickerApi?.sendSelection(baseImage.src)
    }
  }

  const handleCancel = () => {
    window.screenPickerApi?.closePicker()
  }

  return (
    <div style={{ width: '100vw', height: '100vh', overflow: 'hidden', position: 'relative', background: 'transparent' }}>
      <style>{`
        .vision-glow {
          position: absolute;
          inset: 0;
          pointer-events: none;
          box-shadow: inset 0 0 100px rgba(139, 92, 246, 0.25);
          z-index: 5;
          animation: vision-glow-pulse 3s ease-in-out infinite alternate;
        }

        @keyframes vision-glow-pulse {
          from { box-shadow: inset 0 0 60px rgba(139, 92, 246, 0.15); }
          to { box-shadow: inset 0 0 140px rgba(139, 92, 246, 0.35); }
        }

        #toolbar {
          position: absolute;
          top: 24px;
          left: 50%;
          transform: translateX(-50%);
          z-index: 10;
          background: rgba(15, 23, 42, 0.85);
          backdrop-filter: blur(16px);
          -webkit-backdrop-filter: blur(16px);
          border: 1px solid rgba(255, 255, 255, 0.08);
          padding: 8px 20px;
          border-radius: 999px;
          box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.05);
          display: flex;
          align-items: center;
          gap: 12px;
          color: #f8fafc;
          font-family: 'Outfit', 'Inter', sans-serif;
          animation: slide-down 0.4s cubic-bezier(0.16, 1, 0.3, 1);
        }

        @keyframes slide-down {
          from { transform: translate(-50%, -20px); opacity: 0; }
          to { transform: translate(-50%, 0); opacity: 1; }
        }

        .hint {
          font-size: 0.82rem;
          color: #94a3b8;
          margin-right: 12px;
          font-weight: 400;
        }

        .screen-picker-btn {
          font-family: inherit;
          font-size: 0.8rem;
          font-weight: 500;
          padding: 8px 16px;
          border-radius: 999px;
          border: 1px solid transparent;
          cursor: pointer;
          transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
          display: flex;
          align-items: center;
          gap: 6px;
          box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
        }

        .screen-picker-btn:hover {
          transform: translateY(-1px);
          box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }

        .screen-picker-btn:active {
          transform: translateY(0);
        }

        .btn-translate {
          background: rgba(139, 92, 246, 0.15);
          color: #c4b5fd;
          border-color: rgba(139, 92, 246, 0.3);
        }

        .btn-translate:hover {
          background: rgba(139, 92, 246, 0.3);
          color: #f5f3ff;
        }

        .btn-translate.active {
          background: #8b5cf6;
          color: #ffffff;
          border-color: #7c3aed;
          box-shadow: 0 0 15px rgba(139, 92, 246, 0.4);
        }

        .btn-primary {
          background: rgba(16, 185, 129, 0.15);
          color: #a7f3d0;
          border-color: rgba(16, 185, 129, 0.3);
        }

        .btn-primary:hover {
          background: rgba(16, 185, 129, 0.3);
          color: #ecfdf5;
        }

        .btn-danger {
          background: rgba(239, 68, 68, 0.15);
          color: #fecaca;
          border-color: rgba(239, 68, 68, 0.3);
        }

        .btn-danger:hover {
          background: rgba(239, 68, 68, 0.3);
          color: #fef2f2;
        }

        .loading-spinner {
          display: inline-block;
          width: 14px;
          height: 14px;
          border: 2px solid rgba(255, 255, 255, 0.2);
          border-radius: 50%;
          border-top-color: #8b5cf6;
          animation: picker-spin 0.8s linear infinite;
          margin-right: 8px;
        }

        @keyframes picker-spin {
          to { transform: rotate(360deg); }
        }

        #translation-box {
          position: absolute;
          background: rgba(15, 23, 42, 0.9);
          backdrop-filter: blur(20px);
          -webkit-backdrop-filter: blur(20px);
          border: 1px solid rgba(255, 255, 255, 0.1);
          color: #f8fafc;
          padding: 14px 18px;
          border-radius: 12px;
          font-family: 'Inter', sans-serif;
          font-size: 0.9rem;
          line-height: 1.6;
          box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6), 0 0 0 1px rgba(255, 255, 255, 0.05);
          z-index: 100;
          animation: fade-in 0.2s ease-out;
        }

        @keyframes fade-in {
          from { opacity: 0; transform: scale(0.95); }
          to { opacity: 1; transform: scale(1); }
        }

        .close-translate-btn {
          position: absolute;
          top: -10px;
          right: -10px;
          background: #ef4444;
          color: white;
          border: 2px solid rgba(15, 23, 42, 0.95);
          border-radius: 999px;
          min-width: 38px;
          height: 22px;
          display: flex;
          align-items: center;
          justify-content: center;
          cursor: pointer;
          font-size: 0.7rem;
          font-weight: 700;
          letter-spacing: 0.05em;
          text-transform: uppercase;
          box-shadow: 0 4px 10px rgba(0, 0, 0, 0.3);
          transition: all 0.2s;
        }

        .close-translate-btn:hover {
          background: #dc2626;
          transform: scale(1.05);
        }
      `}</style>

      <div className="vision-glow"></div>
      {!isContinuousTranslateActive && (
        <div id="toolbar">
          <span className="hint" id="hint-text">
            {isTranslateMode ? 'Drag over text to translate to Thai' : 'Click and drag to select a region'}
          </span>
          <button
            className={`screen-picker-btn btn-translate ${isTranslateMode ? 'active' : ''}`}
            onClick={handleToggleTranslateMode}
            style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}
          >
            {isTranslateMode ? (
              'Stop Translate'
            ) : (
              <>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="2" y1="12" x2="22" y2="12"></line>
                  <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
                </svg>
                <span>Live Translate</span>
              </>
            )}
          </button>
          {!isTranslateMode && (
            <button className="screen-picker-btn btn-primary" onClick={handleFullscreen}>
              Full Screen
            </button>
          )}
          <button className="screen-picker-btn btn-danger" onClick={handleCancel}>
            Cancel
          </button>
        </div>
      )}

      <canvas ref={bgCanvasRef} id="bg-canvas" style={{ position: 'absolute', top: 0, left: 0, zIndex: 1 }} />
      <canvas
        ref={overlayCanvasRef}
        id="overlay-canvas"
        style={{ position: 'absolute', top: 0, left: 0, zIndex: 2, cursor: 'crosshair' }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleCanvasMouseMove}
        onMouseUp={handleMouseUp}
      />

      <div
        ref={translationBoxRef}
        id="translation-box"
        style={{
          display: isContinuousTranslateActive ? 'block' : 'none',
          position: 'absolute',
          left: `${translationPos.left}px`,
          top: `${translationPos.top}px`,
          maxWidth: `${translationPos.maxWidth}px`,
          zIndex: 100
        }}
      >
        <div className="close-translate-btn" onClick={stopTranslationMode}>
          Esc
        </div>
        <div id="translation-content">
          {translationText === 'Auto-Translating...' && (
            <span className="loading-spinner"></span>
          )}
          {translationText}
        </div>
      </div>
    </div>
  )
}
