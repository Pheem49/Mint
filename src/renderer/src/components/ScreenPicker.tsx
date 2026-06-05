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
      const pendingCapture = window.sessionStorage.getItem('mint:pending-screen-capture')
      if (pendingCapture) {
        window.sessionStorage.removeItem('mint:pending-screen-capture')
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

    const cropCanvas = document.createElement('canvas')
    cropCanvas.width = w
    cropCanvas.height = h
    const cropCtx = cropCanvas.getContext('2d')
    if (!cropCtx) return

    cropCtx.drawImage(baseImage, x, y, w, h, 0, 0, w, h)
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
    <>
      <div className="vision-glow"></div>
      {!isContinuousTranslateActive && (
        <div id="toolbar" style={{ display: 'flex' }}>
          <span className="hint" id="hint-text">
            {isTranslateMode ? 'Drag over text to translate to Thai' : 'Click and drag to select a region'}
          </span>
          <button
            className={`btn btn-translate ${isTranslateMode ? 'active' : ''}`}
            onClick={handleToggleTranslateMode}
          >
            {isTranslateMode ? 'Stop Translate' : '🌐 Live Translate'}
          </button>
          {!isTranslateMode && (
            <button className="btn btn-primary" onClick={handleFullscreen}>
              Full Screen
            </button>
          )}
          <button className="btn btn-danger" onClick={handleCancel}>
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
    </>
  )
}
