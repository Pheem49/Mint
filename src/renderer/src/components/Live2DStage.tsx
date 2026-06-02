import React, { useEffect, useRef, useState } from 'react'
import * as PIXI from 'pixi.js'

// Ensure PIXI is available globally for the live2d library
;(window as any).PIXI = PIXI

interface Live2DStageProps {
  scale: number
  expressionIndex: number
  accessoryIndex: number
  isLocked: boolean
}

// Map Expression Index to Live2D Expression Name
const EXPRESSION_MAP: Record<number, string | null> = {
  0: null,          // Default (normal)
  1: 'Dazed',       // 呆猫 (Dumb Cat)
  2: 'DazedEyes',   // 呆猫眼珠摇晃 (Dumb Cat Eye Roll)
  3: 'Photo',       // 拍照 (Take Photo)
  4: 'Pen',         // 拿笔 (Hold Pen)
  5: 'Click',       // 点一下 (Poke)
  6: 'CatFilter',   // 猫咪滤镜 (Cat Filter)
  7: 'Glasses',     // 眼鏡 (Glasses)
}

// Map Accessory Index to Live2D Expression Name
const ACCESSORY_MAP: Record<number, string | null> = {
  0: null,          // Default (none)
  1: 'Apron',       // ผ้ากันเปื้อน (Apron)
  2: null,          // (Signature Rifle - not in expressions)
}

export default function Live2DStage({ scale, expressionIndex, accessoryIndex, isLocked }: Live2DStageProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const modelRef = useRef<any>(null)
  const appRef = useRef<PIXI.Application | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!canvasRef.current || !containerRef.current) return

    let isMounted = true
    let appInstance: PIXI.Application | null = null

    // Load Live2D engine and model dynamically
    const initLive2D = async () => {
      try {
        // Dynamically import to ensure window.PIXI exists first
        const { Live2DModel } = await import('pixi-live2d-display')

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
          antialias: true,
          autoDensity: true,
          resolution: window.devicePixelRatio || 1,
          resizeTo: containerRef.current!,
        })
        appRef.current = appInstance

        // Load the Live2D model (served from public/models)
        const modelUrl = './models/Shiroko_Model/Shiroko/Shiroko_Core/shiroko.model3.json'
        
        let json: any
        try {
          const res = await fetch(`${modelUrl}?t=${Date.now()}`)
          if (!res.ok) {
            throw new Error(`HTTP error ${res.status}: ${res.statusText}`)
          }
          json = await res.json()
          console.log("Debug: Loaded JSON:", json)
          
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
        appInstance.stage.addChild(model as any)

        // Fit model size and position relative to canvas
        const fitModel = () => {
          if (!modelRef.current || !appRef.current) return
          const m = modelRef.current
          const stageHeight = appRef.current.renderer.height
          const stageWidth = appRef.current.renderer.width
          
          // Fit height to 95% of canvas height
          const ratio = (stageHeight * 0.95) / m.height
          m.scale.set(ratio * scale)
          
          // Bottom-center alignment
          m.anchor.set(0.5, 1)
          m.x = stageWidth / 2
          m.y = stageHeight
        }

        fitModel()
        
        // Handle resizing
        appInstance.renderer.on('resize', fitModel)

        // Enable mouse tracking interactions
        model.interactive = true

        applyExpressionAndAccessory(model, expressionIndex, accessoryIndex)
        setLoading(false)
      } catch (err) {
        console.error('Failed to load Live2D model:', err)
        setLoading(false)
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

  // Update scale & position dynamically when scale changes
  useEffect(() => {
    if (!modelRef.current || !appRef.current) return
    const model = modelRef.current
    const app = appRef.current
    const stageHeight = app.renderer.height
    const stageWidth = app.renderer.width
    const ratio = (stageHeight * 0.95) / model.height
    model.scale.set(ratio * scale)
    model.x = stageWidth / 2
    model.y = stageHeight
  }, [scale])

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
      model.expression('')
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
