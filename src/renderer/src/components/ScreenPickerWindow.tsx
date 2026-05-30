import React, { useEffect, useRef, useState } from 'react'

export default function ScreenPickerWindow() {
    const bgCanvasRef = useRef<HTMLCanvasElement>(null)
    const overlayCanvasRef = useRef<HTMLCanvasElement>(null)
    const baseImageRef = useRef<HTMLImageElement | null>(null)
    const [isDrawing, setIsDrawing] = useState(false)
    const [isTranslateMode, setIsTranslateMode] = useState(false)
    const [isContinuousTranslateActive, setIsContinuousTranslateActive] = useState(false)
    const [hintText, setHintText] = useState('Click and drag to select a region')
    const [translationText, setTranslationText] = useState('')
    const [translationBoxStyle, setTranslationBoxStyle] = useState<React.CSSProperties>({ display: 'none' })

    const startPosRef = useRef({ x: 0, y: 0 })
    const selectedRectRef = useRef<any>(null)
    const isOverlayInteractableRef = useRef(true)

    useEffect(() => {
        const bgCanvas = bgCanvasRef.current
        const overlayCanvas = overlayCanvasRef.current
        if (!bgCanvas || !overlayCanvas) return

        const handleResize = () => {
            bgCanvas.width = window.innerWidth
            bgCanvas.height = window.innerHeight
            overlayCanvas.width = window.innerWidth
            overlayCanvas.height = window.innerHeight
            if (baseImageRef.current) {
                const bgCtx = bgCanvas.getContext('2d')
                bgCtx?.drawImage(baseImageRef.current, 0, 0, bgCanvas.width, bgCanvas.height)
                drawDarkOverlay()
            }
        }

        window.addEventListener('resize', handleResize)
        handleResize()

        if (window.electronPicker) {
            window.electronPicker.onScreenshot((base64Data) => {
                const img = new Image()
                img.onload = () => {
                    baseImageRef.current = img
                    const bgCtx = bgCanvas.getContext('2d')
                    bgCtx?.drawImage(img, 0, 0, bgCanvas.width, bgCanvas.height)
                    drawDarkOverlay()
                }
                img.src = base64Data
            })

            window.electronPicker.onTranslationResult((thaiText) => {
                setTranslationText(thaiText)
            })
        }

        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                e.preventDefault()
                stopTranslationMode()
            }
        }
        window.addEventListener('keydown', handleKeyDown)

        return () => {
            window.removeEventListener('resize', handleResize)
            window.removeEventListener('keydown', handleKeyDown)
        }
    }, [isContinuousTranslateActive])

    const drawDarkOverlay = () => {
        const canvas = overlayCanvasRef.current
        const ctx = canvas?.getContext('2d')
        if (canvas && ctx) {
            ctx.clearRect(0, 0, canvas.width, canvas.height)
            ctx.fillStyle = 'rgba(0, 0, 0, 0.5)'
            ctx.fillRect(0, 0, canvas.width, canvas.height)
        }
    }

    const drawSelectionOutline = (rect: any) => {
        const canvas = overlayCanvasRef.current
        const ctx = canvas?.getContext('2d')
        if (canvas && ctx && rect) {
            ctx.clearRect(0, 0, canvas.width, canvas.height)
            ctx.strokeStyle = isTranslateMode ? '#8b5cf6' : '#00ff88'
            ctx.lineWidth = 3
            ctx.strokeRect(rect.x, rect.y, rect.width, rect.height)
        }
    }

    const drawSelection = (currentX: number, currentY: number) => {
        const canvas = overlayCanvasRef.current
        const ctx = canvas?.getContext('2d')
        if (!canvas || !ctx) return

        ctx.clearRect(0, 0, canvas.width, canvas.height)
        ctx.fillStyle = 'rgba(0, 0, 0, 0.5)'
        ctx.fillRect(0, 0, canvas.width, canvas.height)

        const startX = startPosRef.current.x
        const startY = startPosRef.current.y
        const x = Math.min(startX, currentX)
        const y = Math.min(startY, currentY)
        const width = Math.abs(currentX - startX)
        const height = Math.abs(currentY - startY)

        ctx.clearRect(x, y, width, height)
        ctx.strokeStyle = isTranslateMode ? '#8b5cf6' : '#00ff88'
        ctx.lineWidth = 2
        ctx.strokeRect(x, y, width, height)
    }

    const setOverlayInteractable = (isInteractable: boolean) => {
        if (isOverlayInteractableRef.current === isInteractable) return
        isOverlayInteractableRef.current = isInteractable
        window.electronPicker?.setOverlayInteractable(isInteractable)
    }

    const stopTranslationMode = () => {
        setIsContinuousTranslateActive(false)
        setTranslationBoxStyle({ display: 'none' })
        setOverlayInteractable(true)
        window.electronPicker?.stopContinuousTranslation()

        const bgCanvas = bgCanvasRef.current
        if (bgCanvas && baseImageRef.current) {
            const bgCtx = bgCanvas.getContext('2d')
            bgCtx?.clearRect(0, 0, bgCanvas.width, bgCanvas.height)
            bgCtx?.drawImage(baseImageRef.current, 0, 0, bgCanvas.width, bgCanvas.height)
        }

        selectedRectRef.current = null
        drawDarkOverlay()
    }

    const setTranslationBoxPosition = (rect: any) => {
        const margin = 10
        const boxWidth = Math.min(400, Math.max(240, rect.width))
        const preferredTop = rect.y + rect.height + margin

        // Simple height approximation since offsetHeight is not available before render
        const boxHeight = 100 
        const fallbackTop = Math.max(margin, rect.y - margin - boxHeight)
        const top = preferredTop + boxHeight <= window.innerHeight
            ? preferredTop
            : fallbackTop

        setTranslationBoxStyle({
            display: 'block',
            maxWidth: `${boxWidth}px`,
            left: `${Math.max(margin, Math.min(rect.x, window.innerWidth - boxWidth - margin))}px`,
            top: `${top}px`
        })
    }

    const cropAndSend = (rect: any) => {
        if (rect.width === 0 || rect.height === 0 || !baseImageRef.current) return

        const cropCanvas = document.createElement('canvas')
        cropCanvas.width = rect.width
        cropCanvas.height = rect.height
        const cropCtx = cropCanvas.getContext('2d')
        
        cropCtx?.drawImage(
            baseImageRef.current, 
            rect.x, rect.y, rect.width, rect.height, 
            0, 0, rect.width, rect.height
        )

        const croppedBase64 = cropCanvas.toDataURL('image/png')
        
        if (isTranslateMode) {
            setIsContinuousTranslateActive(true)
            selectedRectRef.current = rect
            drawSelectionOutline(rect)
            setTranslationText('Auto-Translating...')
            setTranslationBoxPosition(rect)
            setOverlayInteractable(false)

            window.electronPicker?.startContinuousTranslation(rect)
        } else {
            window.electronPicker?.sendSelection(croppedBase64)
        }
    }

    const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
        if (isContinuousTranslateActive) return
        setIsDrawing(true)
        startPosRef.current = { x: e.clientX, y: e.clientY }
    }

    const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
        if (!isDrawing) return
        drawSelection(e.clientX, e.clientY)
    }

    const handleMouseUp = (e: React.MouseEvent<HTMLCanvasElement>) => {
        if (!isDrawing) return
        setIsDrawing(false)

        const startX = startPosRef.current.x
        const startY = startPosRef.current.y
        const x = Math.min(startX, e.clientX)
        const y = Math.min(startY, e.clientY)
        const width = Math.abs(e.clientX - startX)
        const height = Math.abs(e.clientY - startY)

        cropAndSend({ x, y, width, height })
    }

    const handleTranslateToggle = () => {
        const nextMode = !isTranslateMode
        setIsTranslateMode(nextMode)
        if (nextMode) {
            setHintText('Drag over text to translate to Thai')
            setTranslationBoxStyle({ display: 'none' })
            selectedRectRef.current = null
            drawDarkOverlay()
        } else {
            if (isContinuousTranslateActive) {
                stopTranslationMode()
            }
            setHintText('Click and drag to select a region')
            setTranslationBoxStyle({ display: 'none' })
            selectedRectRef.current = null
            drawDarkOverlay()
        }
    }

    const handleFullScreen = () => {
        if (baseImageRef.current && !isTranslateMode) {
            window.electronPicker?.sendSelection(baseImageRef.current.src)
        }
    }

    const handleCancel = () => {
        window.electronPicker?.closePicker()
    }

    const handleTranslationBoxMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
        if (!isContinuousTranslateActive) return
        const box = e.currentTarget.getBoundingClientRect()
        const isInside = e.clientX >= box.left && e.clientX <= box.right && e.clientY >= box.top && e.clientY <= box.bottom
        setOverlayInteractable(isInside)
    }

    return (
        <div style={{ cursor: 'crosshair', width: '100vw', height: '100vh', overflow: 'hidden', position: 'relative' }}>
            <style>{`
                .loading-spinner {
                    display: inline-block;
                    width: 16px;
                    height: 16px;
                    border: 2px solid rgba(255,255,255,0.3);
                    border-radius: 50%;
                    border-top-color: #fff;
                    animation: spin 1s ease-in-out infinite;
                    vertical-align: middle;
                    margin-right: 8px;
                }
                @keyframes spin {
                    to { transform: rotate(360deg); }
                }
            `}</style>
            
            <div className="vision-glow" style={{
                position: 'absolute',
                inset: 0,
                pointerEvents: 'none',
                boxShadow: 'inset 0 0 100px rgba(139, 92, 246, 0.15)',
                zIndex: 5
            }} />

            {!isContinuousTranslateActive && (
                <div id="toolbar" style={{
                    position: 'absolute',
                    top: '20px',
                    left: '50%',
                    transform: 'translateX(-50%)',
                    zIndex: 10,
                    backgroundColor: 'rgba(30, 30, 30, 0.9)',
                    backdropFilter: 'blur(10px)',
                    padding: '10px 20px',
                    borderRadius: '8px',
                    boxShadow: '0 4px 15px rgba(0,0,0,0.5)',
                    display: 'flex',
                    gap: '10px',
                    color: 'white',
                    alignItems: 'center',
                    fontFamily: 'Inter, sans-serif'
                }}>
                    <span className="hint" style={{ fontSize: '12px', color: '#aaa', marginRight: '15px' }}>{hintText}</span>
                    <button className={`btn btn-translate ${isTranslateMode ? 'active' : ''}`} onClick={handleTranslateToggle} style={{
                        backgroundColor: '#8b5cf6', color: 'white', borderColor: '#7c3aed',
                        padding: '8px 16px', borderRadius: '6px', cursor: 'pointer', fontSize: '14px', border: '1px solid'
                    }}>
                        {isTranslateMode ? 'Stop Translate' : '🌐 Live Translate'}
                    </button>
                    <button className="btn btn-primary" onClick={handleFullScreen} style={{
                        backgroundColor: '#28a745', color: 'white', borderColor: '#28a745',
                        padding: '8px 16px', borderRadius: '6px', cursor: 'pointer', fontSize: '14px', border: '1px solid'
                    }}>Full Screen</button>
                    <button className="btn btn-danger" onClick={handleCancel} style={{
                        backgroundColor: '#dc3545', color: 'white', borderColor: '#dc3545',
                        padding: '8px 16px', borderRadius: '6px', cursor: 'pointer', fontSize: '14px', border: '1px solid'
                    }}>Cancel</button>
                </div>
            )}

            <canvas ref={bgCanvasRef} id="bg-canvas" style={{ position: 'absolute', top: 0, left: 0, zIndex: 1 }} />
            <canvas
                ref={overlayCanvasRef}
                id="overlay-canvas"
                onMouseDown={handleMouseDown}
                onMouseMove={handleMouseMove}
                onMouseUp={handleMouseUp}
                style={{ position: 'absolute', top: 0, left: 0, zIndex: 2 }}
            />

            <div
                id="translation-box"
                style={{
                    position: 'absolute',
                    backgroundColor: 'rgba(20, 20, 20, 0.95)',
                    backdropFilter: 'blur(10px)',
                    border: '1px solid #444',
                    color: '#fff',
                    padding: '12px 16px',
                    borderRadius: '8px',
                    fontFamily: 'Inter, sans-serif',
                    fontSize: '14px',
                    lineHeight: 1.5,
                    boxShadow: '0 8px 25px rgba(0,0,0,0.6)',
                    zIndex: 100,
                    pointerEvents: 'auto',
                    ...translationBoxStyle
                }}
                onMouseMove={handleTranslationBoxMouseMove}
            >
                <div
                    className="close-translate-btn"
                    onClick={stopTranslationMode}
                    style={{
                        position: 'absolute',
                        top: '-18px',
                        right: '-14px',
                        backgroundColor: '#dc3545',
                        color: 'white',
                        border: '2px solid rgba(20, 20, 20, 0.95)',
                        borderRadius: '999px',
                        minWidth: '40px',
                        height: '24px',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        cursor: 'pointer',
                        fontSize: '12px',
                        fontWeight: 'bold',
                        letterSpacing: '0.04em',
                        textTransform: 'uppercase',
                        boxShadow: '0 2px 5px rgba(0,0,0,0.5)',
                    }}
                >
                    Esc
                </div>
                <div id="translation-content">
                    {translationText === 'Auto-Translating...' ? (
                        <>
                            <span className="loading-spinner" />
                            Auto-Translating...
                        </>
                    ) : (
                        translationText
                    )}
                </div>
            </div>
        </div>
    )
}
