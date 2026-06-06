import React, { useEffect, useRef, useState } from 'react'
import * as PIXI from 'pixi.js'

// Ensure PIXI is available globally for the live2d library
;(window as any).PIXI = PIXI

interface Live2DStageProps {
  scale: number
  expressionIndex: number
  accessoryIndex: number
  isLocked: boolean
  isActive?: boolean
  onLoadComplete?: () => void
}

// Map Expression Index to Live2D Expression Name
const EXPRESSION_MAP: Record<number, string | null> = {
  0: null,          // Default (normal)
  1: 'Dazed',       // 呆猫 (Dumb Cat)
  2: 'DazedEyes',   // 呆猫眼珠摇晃 (Dumb Cat Eye Roll)
  3: 'Photo',       // 拍照 (Take Photo)
  4: 'Click',       // 点一下 (Poke)
  5: 'CatFilter',   // 猫咪滤镜 (Cat Filter)
}

// Map Accessory Index to Live2D Expression Name
const ACCESSORY_MAP: Record<number, string | null> = {
  0: null,          // Default (none)
  1: 'Apron',       // ผ้ากันเปื้อน (Apron)
  2: 'Glasses',     // 眼鏡 (Glasses)
  3: 'Pen',         // 拿笔 (Hold Pen)
}

const TRACKING_SPEED = 1.25
const MAX_DEVICE_PIXEL_RATIO = 1.25
const ACTIVE_MAX_FPS = 30
const INACTIVE_MAX_FPS = 8

const clampToUnitCircle = (x: number, y: number) => {
  if (isNaN(x) || isNaN(y) || !isFinite(x) || !isFinite(y)) {
    return { x: 0, y: 0 }
  }
  const distance = Math.hypot(x, y)
  if (distance <= 1) return { x, y }

  return {
    x: x / distance,
    y: y / distance,
  }
}

export default function Live2DStage({ scale, expressionIndex, accessoryIndex, isLocked, isActive = true, onLoadComplete }: Live2DStageProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const modelRef = useRef<any>(null)
  const appRef = useRef<PIXI.Application | null>(null)
  const [loading, setLoading] = useState(true)
  const baseWidthRef = useRef<number | null>(null)
  const baseHeightRef = useRef<number | null>(null)

  const setRenderActive = (active: boolean) => {
    const app = appRef.current
    if (!app) return

    app.ticker.maxFPS = active ? ACTIVE_MAX_FPS : INACTIVE_MAX_FPS

    if (active) {
      app.start()
    } else {
      app.stop()
    }
  }

  useEffect(() => {
    if (!canvasRef.current || !containerRef.current) return

    let isMounted = true
    let appInstance: PIXI.Application | null = null

    // Load Live2D engine and model dynamically
    const initLive2D = async () => {
      try {
        // Dynamically import to ensure window.PIXI exists first
        const { Live2DModel } = await import('pixi-live2d-display/cubism4')

        // Register ticker
        try {
          Live2DModel.registerTicker(PIXI.Ticker as any)
        } catch (e) {
          // Already registered
        }

        if (!isMounted) return

        // Create Pixi Application
        appInstance = new PIXI.Application({
          view: canvasRef.current!,
          backgroundAlpha: 0,
          antialias: false,
          autoDensity: true,
          resolution: Math.min(window.devicePixelRatio || 1, MAX_DEVICE_PIXEL_RATIO),
          resizeTo: containerRef.current!,
        })
        appInstance.ticker.maxFPS = ACTIVE_MAX_FPS
        appInstance.ticker.minFPS = 10
        appRef.current = appInstance

        // Load the Live2D model (served from public/models)
        const modelUrl = './models/Shiroko_Model/Shiroko/Shiroko_Core/shiroko.model3.json'
        
        let json: any
        try {
          const res = await fetch(modelUrl)
          if (!res.ok) {
            throw new Error(`HTTP error ${res.status}: ${res.statusText}`)
          }
          json = await res.json()
          
          // Inject the URL so pixi-live2d-display can resolve relative paths
          json.url = modelUrl
        } catch (e) {
          console.error("Debug: Failed to fetch and parse JSON:", e)
          throw e
        }
        
        const model = await Live2DModel.from(json)
        if (!isMounted) {
          model.destroy()
          return
        }

        modelRef.current = model
        baseWidthRef.current = model.width || model.internalModel?.originalWidth || 1000
        baseHeightRef.current = model.height || model.internalModel?.originalHeight || 1000
        appInstance.stage.addChild(model as any)

        // Fit model size and position relative to canvas
        const fitModel = () => {
          if (!modelRef.current || !appRef.current) return
          const m = modelRef.current
          const stageHeight = appRef.current.screen.height
          const stageWidth = appRef.current.screen.width
          
          if (stageWidth < 100 || stageHeight < 100) return
          
          const baseWidth = baseWidthRef.current || m.width || 1000
          const baseHeight = baseHeightRef.current || m.height || 1000
          
          const widthScale = stageWidth / baseWidth
          const heightScale = stageHeight / baseHeight
          
          const modelScale = Math.min(widthScale, heightScale) * 1.85 * scale
          m.scale.set(modelScale)
          
          // Center-center alignment with 55% Y offset
          m.anchor.set(0.5, 0.5)
          m.x = stageWidth / 2
          m.y = stageHeight / 2 + stageHeight * 0.55
        }

        fitModel()
        
        // Handle resizing
        appInstance.renderer.on('resize', fitModel)

        // Enable mouse tracking interactions
        model.interactive = true

        const focusController = model.internalModel.focusController
        const updateFocus = focusController.update.bind(focusController)
        focusController.update = (deltaMs: number) => updateFocus(deltaMs * TRACKING_SPEED)

        // Match the extra mouse-driven parameters configured by the original VTube Studio model.
        model.internalModel.on('beforeModelUpdate', () => {
          const focus = model.internalModel.focusController
          const coreModel = model.internalModel.coreModel as {
            setParameterValueById: (parameterId: string, value: number) => void
          }

          coreModel.setParameterValueById('Param77', focus.x * 10)
          coreModel.setParameterValueById('Param78', focus.y)
          coreModel.setParameterValueById('Param83', focus.x)
          coreModel.setParameterValueById('Param86', focus.y)
        })

        applyExpressionAndAccessory(model, expressionIndex, accessoryIndex)
        setRenderActive(isActive && document.visibilityState === 'visible')
        setLoading(false)
        onLoadComplete?.()
      } catch (err) {
        console.error('Failed to load Live2D model:', err)
        setLoading(false)
        onLoadComplete?.()
      }
    }

    initLive2D()

    return () => {
      isMounted = false
      if (appInstance) {
        appInstance.destroy(true, {
          children: true,
          texture: true,
          baseTexture: true,
        })
        appRef.current = null
      }
      modelRef.current = null
    }
  }, [])

  // Update scale & position dynamically when scale changes or container resizes
  useEffect(() => {
    if (!containerRef.current) return

    const handleResize = () => {
      if (!isActive) return
      if (!modelRef.current || !appRef.current) return
      const app = appRef.current
      app.resize()
      
      const model = modelRef.current
      const stageHeight = app.screen.height
      const stageWidth = app.screen.width
      
      if (stageWidth < 100 || stageHeight < 100) return
      
      const baseWidth = baseWidthRef.current || model.width || 1000
      const baseHeight = baseHeightRef.current || model.height || 1000
      
      const widthScale = stageWidth / baseWidth
      const heightScale = stageHeight / baseHeight
      
      const modelScale = Math.min(widthScale, heightScale) * 1.85 * scale
      model.scale.set(modelScale)
      
      model.anchor.set(0.5, 0.5)
      model.x = stageWidth / 2
      model.y = stageHeight / 2 + stageHeight * 0.55
    }

    const resizeObserver = new ResizeObserver(() => {
      handleResize()
    })

    resizeObserver.observe(containerRef.current)

    // Trigger immediate resize/reposition
    handleResize()

    return () => {
      resizeObserver.disconnect()
    }
  }, [scale, loading, isActive])

  useEffect(() => {
    const updateRenderState = () => {
      setRenderActive(isActive && document.visibilityState === 'visible')
    }

    updateRenderState()
    document.addEventListener('visibilitychange', updateRenderState)

    return () => {
      document.removeEventListener('visibilitychange', updateRenderState)
    }
  }, [isActive])

  // Follow the pointer across the whole window and smoothly return to center when tracking stops.
  useEffect(() => {
    const focus = (x: number, y: number) => {
      if (isNaN(x) || isNaN(y) || !isFinite(x) || !isFinite(y)) {
        x = 0
        y = 0
      }
      modelRef.current?.internalModel?.focusController?.focus(x, y)
    }

    const centerFocus = () => focus(0, 0)

    const handlePointerMove = (event: PointerEvent) => {
      if (isLocked || !isActive || document.visibilityState !== 'visible' || !containerRef.current) return

      const rect = containerRef.current.getBoundingClientRect()
      if (!rect.width || !rect.height) return

      const x = (event.clientX - (rect.left + rect.width / 2)) / (rect.width / 2)
      const y = ((rect.top + rect.height / 2) - event.clientY) / (rect.height / 2)
      const normalized = clampToUnitCircle(x, y)

      focus(normalized.x, normalized.y)
    }

    if (isLocked) centerFocus()

    window.addEventListener('pointermove', handlePointerMove)
    window.addEventListener('blur', centerFocus)

    return () => {
      window.removeEventListener('pointermove', handlePointerMove)
      window.removeEventListener('blur', centerFocus)
    }
  }, [isLocked])

  // Update expressions & accessories dynamically
  useEffect(() => {
    if (!modelRef.current) return
    applyExpressionAndAccessory(modelRef.current, expressionIndex, accessoryIndex)
  }, [expressionIndex, accessoryIndex])

  const applyExpressionAndAccessory = (model: any, exprIdx: number, accIdx: number) => {
    const expManager = model.internalModel?.motionManager?.expressionManager
    if (!expManager) return

    const exprName = EXPRESSION_MAP[exprIdx]
    const accName = ACCESSORY_MAP[accIdx]

    if (!exprName && !accName) {
      expManager.resetExpression()
      return
    }

    // Apply the active expression or accessory
    if (exprName) {
      model.expression(exprName)
    } else if (accName) {
      model.expression(accName)
    }
  }

  return (
    <div
      ref={containerRef}
      style={{
        position: 'relative',
        width: '100%',
        height: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      {loading && (
        <div style={{ position: 'absolute', color: '#9f7aea', fontSize: '14px', fontFamily: 'Outfit, sans-serif' }}>
          Loading Shiroko Live2D Model...
        </div>
      )}
      <canvas
        ref={canvasRef}
        style={{
          width: '100%',
          height: '100%',
          opacity: loading ? 0 : 1,
          transition: 'opacity 0.3s ease-out',
          pointerEvents: isLocked ? 'none' : 'auto',
        }}
      />
    </div>
  )
}
