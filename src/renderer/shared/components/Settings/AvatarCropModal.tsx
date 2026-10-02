import React, { useState, useRef, useEffect, useCallback } from 'react'

interface AvatarCropModalProps {
  imageSrc: string
  fileName: string
  onClose: () => void
  onConfirm: (croppedDataUri: string) => Promise<void> | void
}

const CROP_SIZE = 280
const OUTPUT_SIZE = 512

export default function AvatarCropModal({
  imageSrc,
  fileName,
  onClose,
  onConfirm,
}: AvatarCropModalProps) {
  const [zoom, setZoom] = useState(1)
  const [rotation, setRotation] = useState(0) // in degrees (0, 90, 180, 270)
  const [position, setPosition] = useState({ x: 0, y: 0 })
  const [isProcessing, setIsProcessing] = useState(false)
  const [imgNaturalSize, setImgNaturalSize] = useState({ width: 0, height: 0 })

  const isDraggingRef = useRef(false)
  const dragStartRef = useRef({ x: 0, y: 0 })
  const initialTouchDistanceRef = useRef<number | null>(null)
  const initialZoomRef = useRef(1)
  const imageElementRef = useRef<HTMLImageElement | null>(null)

  // Load natural dimensions
  useEffect(() => {
    const img = new Image()
    img.onload = () => {
      setImgNaturalSize({ width: img.naturalWidth, height: img.naturalHeight })
    }
    img.src = imageSrc
  }, [imageSrc])

  // Calculate base scale so image covers the crop area
  const baseScale = (() => {
    if (!imgNaturalSize.width || !imgNaturalSize.height) return 1
    return Math.max(
      CROP_SIZE / imgNaturalSize.width,
      CROP_SIZE / imgNaturalSize.height
    )
  })()

  const zoomRef = useRef(zoom)
  zoomRef.current = zoom
  const rotationRef = useRef(rotation)
  rotationRef.current = rotation

  // Clamp position so image always completely covers the crop box (no empty space)
  const clampPosition = useCallback(
    (pos: { x: number; y: number }, currentZoom: number, currentRotation: number) => {
      if (!imgNaturalSize.width || !imgNaturalSize.height) return pos
      const isRotatedQuarter = currentRotation % 180 !== 0
      const naturalW = isRotatedQuarter ? imgNaturalSize.height : imgNaturalSize.width
      const naturalH = isRotatedQuarter ? imgNaturalSize.width : imgNaturalSize.height
      const effW = naturalW * baseScale * currentZoom
      const effH = naturalH * baseScale * currentZoom
      const maxX = Math.max(0, (effW - CROP_SIZE) / 2)
      const maxY = Math.max(0, (effH - CROP_SIZE) / 2)
      return {
        x: Math.min(Math.max(pos.x, -maxX), maxX),
        y: Math.min(Math.max(pos.y, -maxY), maxY),
      }
    },
    [imgNaturalSize.width, imgNaturalSize.height, baseScale]
  )

  // Mouse drag handlers
  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault()
    isDraggingRef.current = true
    dragStartRef.current = {
      x: e.clientX - position.x,
      y: e.clientY - position.y,
    }
  }

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isDraggingRef.current) return
    const rawX = e.clientX - dragStartRef.current.x
    const rawY = e.clientY - dragStartRef.current.y
    setPosition(clampPosition({ x: rawX, y: rawY }, zoomRef.current, rotationRef.current))
  }, [clampPosition])

  const handleMouseUp = useCallback(() => {
    isDraggingRef.current = false
  }, [])

  useEffect(() => {
    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)
    return () => {
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }, [handleMouseMove, handleMouseUp])

  // Touch drag & pinch-to-zoom handlers
  const handleTouchStart = (e: React.TouchEvent) => {
    if (e.touches.length === 1) {
      isDraggingRef.current = true
      dragStartRef.current = {
        x: e.touches[0].clientX - position.x,
        y: e.touches[0].clientY - position.y,
      }
    } else if (e.touches.length === 2) {
      isDraggingRef.current = false
      const dist = Math.hypot(
        e.touches[0].clientX - e.touches[1].clientX,
        e.touches[0].clientY - e.touches[1].clientY
      )
      initialTouchDistanceRef.current = dist
      initialZoomRef.current = zoom
    }
  }

  const handleTouchMove = (e: React.TouchEvent) => {
    if (e.touches.length === 1 && isDraggingRef.current) {
      const rawX = e.touches[0].clientX - dragStartRef.current.x
      const rawY = e.touches[0].clientY - dragStartRef.current.y
      setPosition(clampPosition({ x: rawX, y: rawY }, zoom, rotation))
    } else if (e.touches.length === 2 && initialTouchDistanceRef.current) {
      const dist = Math.hypot(
        e.touches[0].clientX - e.touches[1].clientX,
        e.touches[0].clientY - e.touches[1].clientY
      )
      const factor = dist / initialTouchDistanceRef.current
      const nextZoom = Math.min(Math.max(initialZoomRef.current * factor, 1), 3.5)
      setZoom(nextZoom)
      setPosition((pos) => clampPosition(pos, nextZoom, rotation))
    }
  }

  const handleTouchEnd = () => {
    isDraggingRef.current = false
    initialTouchDistanceRef.current = null
  }

  // Smooth zoom helper that clamps position accordingly
  const updateZoom = (newZoom: number) => {
    const clampedZoom = Math.min(Math.max(newZoom, 1), 3.5)
    setZoom(clampedZoom)
    setPosition((pos) => clampPosition(pos, clampedZoom, rotation))
  }

  // Mouse wheel zoom
  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault()
    const delta = e.deltaY < 0 ? 0.08 : -0.08
    updateZoom(zoom + delta)
  }

  // Rotate 90 degrees and re-clamp position
  const handleRotate = () => {
    const nextRot = (rotation + 90) % 360
    setRotation(nextRot)
    setPosition((pos) => clampPosition(pos, zoom, nextRot))
  }

  // Reset to default zoom, position, rotation
  const handleReset = () => {
    setZoom(1)
    setRotation(0)
    setPosition({ x: 0, y: 0 })
  }

  // Generate cropped output canvas
  const handleSave = async () => {
    if (!imgNaturalSize.width || !imgNaturalSize.height) return
    setIsProcessing(true)

    try {
      const img = new Image()
      img.crossOrigin = 'anonymous'
      await new Promise<void>((resolve, reject) => {
        img.onload = () => resolve()
        img.onerror = reject
        img.src = imageSrc
      })

      const canvas = document.createElement('canvas')
      canvas.width = OUTPUT_SIZE
      canvas.height = OUTPUT_SIZE
      const ctx = canvas.getContext('2d')
      if (!ctx) throw new Error('Could not get canvas context')

      ctx.imageSmoothingEnabled = true
      ctx.imageSmoothingQuality = 'high'

      // Ratio between export output and preview box
      const R = OUTPUT_SIZE / CROP_SIZE

      // Translate to center + screen-space position offset
      ctx.translate(OUTPUT_SIZE / 2 + position.x * R, OUTPUT_SIZE / 2 + position.y * R)
      ctx.rotate((rotation * Math.PI) / 180)

      const finalScale = baseScale * zoom * R
      ctx.scale(finalScale, finalScale)

      // Draw image centered
      ctx.drawImage(
        img,
        -imgNaturalSize.width / 2,
        -imgNaturalSize.height / 2,
        imgNaturalSize.width,
        imgNaturalSize.height
      )

      const croppedDataUri = canvas.toDataURL('image/jpeg', 0.95)
      await onConfirm(croppedDataUri)
    } catch (err) {
      console.error('Failed to crop image:', err)
      alert('Failed to process image.')
    } finally {
      setIsProcessing(false)
    }
  }

  return (
    <div className="crop-modal-overlay" onClick={onClose}>
      <div className="crop-modal-card" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="crop-modal-header">
          <button
            type="button"
            className="crop-modal-back-btn"
            onClick={onClose}
            aria-label="Back"
            title="ย้อนกลับ / ยกเลิก"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="19" y1="12" x2="5" y2="12"></line>
              <polyline points="12 19 5 12 12 5"></polyline>
            </svg>
          </button>
          <h3 className="crop-modal-title">ครอบตัดและหมุน</h3>
          <button
            type="button"
            className="crop-modal-reset-btn"
            onClick={handleReset}
            title="รีเซ็ตตำแหน่งและการหมุน"
          >
            รีเซ็ต
          </button>
        </div>

        {/* Viewport / Crop Box */}
        <div className="crop-modal-stage">
          <div
            className="crop-viewport"
            style={{ width: `${CROP_SIZE}px`, height: `${CROP_SIZE}px` }}
            onMouseDown={handleMouseDown}
            onTouchStart={handleTouchStart}
            onTouchMove={handleTouchMove}
            onTouchEnd={handleTouchEnd}
            onWheel={handleWheel}
          >
            {/* The transformed image */}
            {imageSrc && (
              <img
                ref={imageElementRef}
                src={imageSrc}
                alt="Crop preview"
                className="crop-image"
                draggable={false}
                style={{
                  transform: `translate(calc(-50% + ${position.x}px), calc(-50% + ${position.y}px)) rotate(${rotation}deg) scale(${baseScale * zoom})`,
                }}
              />
            )}

            {/* Circular cut-out shadow mask */}
            <div className="crop-mask-circle" />

            {/* 4 Corner Bracket Markers */}
            <div className="crop-bracket crop-bracket-tl" />
            <div className="crop-bracket crop-bracket-tr" />
            <div className="crop-bracket crop-bracket-bl" />
            <div className="crop-bracket crop-bracket-br" />
          </div>
          <p className="crop-hint-drag">ลากเพื่อเลื่อนตำแหน่ง · เลื่อนล้อเมาส์เพื่อซูม</p>
        </div>

        {/* Toolbar & Controls */}
        <div className="crop-modal-controls">
          {/* Zoom Slider */}
          <div className="crop-zoom-row">
            <button
              type="button"
              className="crop-zoom-btn"
              onClick={() => updateZoom(zoom - 0.15)}
              title="ซูมออก"
              disabled={zoom <= 1}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <line x1="5" y1="12" x2="19" y2="12"></line>
              </svg>
            </button>
            <input
              type="range"
              min="1"
              max="3"
              step="0.01"
              value={zoom}
              onChange={(e) => updateZoom(parseFloat(e.target.value))}
              className="crop-zoom-slider range-slider"
              aria-label="Zoom"
            />
            <button
              type="button"
              className="crop-zoom-btn"
              onClick={() => updateZoom(zoom + 0.15)}
              title="ซูมเข้า"
              disabled={zoom >= 3}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <line x1="12" y1="5" x2="12" y2="19"></line>
                <line x1="5" y1="12" x2="19" y2="12"></line>
              </svg>
            </button>
          </div>

          {/* Rotate Button */}
          <div className="crop-rotate-row">
            <button
              type="button"
              className="crop-rotate-btn"
              onClick={handleRotate}
              title="หมุน 90 องศา"
            >
              <div className="crop-rotate-icon">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
                </svg>
              </div>
              <span className="crop-rotate-label">หมุน</span>
            </button>
          </div>

          {/* Actions: Cancel & Next / Confirm */}
          <div className="crop-actions-row">
            <button
              type="button"
              className="btn-secondary crop-btn-cancel"
              onClick={onClose}
              disabled={isProcessing}
            >
              ยกเลิก
            </button>
            <button
              type="button"
              className="btn-primary crop-btn-confirm"
              onClick={handleSave}
              disabled={isProcessing}
            >
              {isProcessing ? 'กำลังบันทึก…' : 'ถัดไป'}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
