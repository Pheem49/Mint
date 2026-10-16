import React, { useEffect, useRef, useState } from 'react'
import * as PIXI from 'pixi.js'
import type { Status } from './state'

// Ensure PIXI is available globally for the live2d library
;(window as any).PIXI = PIXI

interface Live2DStageProps {
  scale: number
  expressionIndex: number
  accessoryIndex: number
  isLocked: boolean
  isActive?: boolean
  onLoadComplete?: () => void
  onLoadError?: (message: string) => void
  status?: Status | 'idle'
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
const IDLE_MAX_FPS = 24
const INTERACTION_MAX_FPS = 45

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

export default function Live2DStage({ scale, expressionIndex, accessoryIndex, isLocked, isActive = true, onLoadComplete, onLoadError, status = 'idle' }: Live2DStageProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const modelRef = useRef<any>(null)
  const appRef = useRef<PIXI.Application | null>(null)
  const [loading, setLoading] = useState(true)
  const [shouldRender, setShouldRender] = useState(isActive)
  const baseWidthRef = useRef<number | null>(null)
  const baseHeightRef = useRef<number | null>(null)
  const pointerInsideRef = useRef(false)
  const live = useRef({ scale, expressionIndex, accessoryIndex, isLocked, isActive, status, changedAt: performance.now() })
  if (live.current.status !== status) live.current.changedAt = performance.now()
  Object.assign(live.current, { scale, expressionIndex, accessoryIndex, isLocked, isActive, status })
  const expressions = useRef<Record<string, Array<{ Id: string; Value: number; Blend: string }>>>({})
  const reducedMotion = useRef(window.matchMedia('(prefers-reduced-motion: reduce)').matches)
  useEffect(() => {
    const query = window.matchMedia('(prefers-reduced-motion: reduce)')
    const update = () => { reducedMotion.current = query.matches }
    query.addEventListener('change', update)
    return () => query.removeEventListener('change', update)
  }, [])

  useEffect(() => {
    if (isActive && !shouldRender) {
      setShouldRender(true)
    }
  }, [isActive, shouldRender])

  const setRenderActive = (active: boolean, interactive = pointerInsideRef.current) => {
    const app = appRef.current
    const model = modelRef.current
    if (!app) return

    // The model idles for most of a chat session. Keep its animation smooth
    // without reserving a 60 FPS GPU budget until the user actually hovers it.
    app.ticker.maxFPS = interactive && !live.current.isLocked ? INTERACTION_MAX_FPS : IDLE_MAX_FPS
    if (model) {
      model.autoUpdate = active
      model.renderable = active
    }

    if (active) {
      app.start()
      app.render()
    } else {
      app.stop()
    }
  }

  useEffect(() => {
    if (!shouldRender) return
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
        appInstance.ticker.maxFPS = IDLE_MAX_FPS
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
        const modelDirectory = modelUrl.slice(0, modelUrl.lastIndexOf('/') + 1)
        await Promise.all((json.FileReferences.Expressions || []).map(async (entry: { Name: string; File: string }) => {
          const result = await fetch(modelDirectory + entry.File)
          if (!result.ok) throw new Error(`Expression could not load: ${entry.Name}`)
          expressions.current[entry.Name] = (await result.json()).Parameters
        }))
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
          
          const modelScale = Math.min(widthScale, heightScale) * 1.45 * live.current.scale
          m.scale.set(modelScale)
          
          // Center the character within the floating stage.
          m.anchor.set(0.5, 0.5)
          m.x = stageWidth / 2
          m.y = stageHeight / 2 + stageHeight * 0.28
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
            addParameterValueById: (parameterId: string, value: number) => void
            getParameterValueById: (parameterId: string) => number
          }

          coreModel.setParameterValueById('Param77', focus.x * 10)
          coreModel.setParameterValueById('Param78', focus.y)
          coreModel.setParameterValueById('Param83', focus.x)
          coreModel.setParameterValueById('Param86', focus.y)
          // The Cubism update restores its saved baseline after each frame,
          // so accessories and expressions can be composed without accumulation.
          for (const name of [EXPRESSION_MAP[live.current.expressionIndex], ACCESSORY_MAP[live.current.accessoryIndex]]) {
            for (const parameter of name ? expressions.current[name] || [] : []) {
              const value = coreModel.getParameterValueById(parameter.Id)
              coreModel.setParameterValueById(parameter.Id, parameter.Blend === 'Overwrite' ? parameter.Value : parameter.Blend === 'Multiply' ? value * parameter.Value : value + parameter.Value)
            }
          }
          if (!reducedMotion.current) {
            const elapsed = (performance.now() - live.current.changedAt) / 1000
            const state = live.current.status
            const angle = state === 'thinking' ? Math.sin(elapsed * 1.1) * 3 : state === 'working' ? Math.sin(elapsed * 1.5) * 2 : 0
            const nod = state === 'responding' ? Math.sin(elapsed * 2) * 2 : state === 'completed' && elapsed < 1 ? Math.sin(elapsed * Math.PI * 2) * 5 : 0
            coreModel.addParameterValueById('ParamAngleZ', angle)
            coreModel.addParameterValueById('ParamAngleY', nod)
          }

        })

        setRenderActive(live.current.isActive && document.visibilityState === 'visible')
        setLoading(false)
        onLoadComplete?.()
      } catch (err) {
        console.error('Failed to load Live2D model:', err)
        onLoadError?.(String(err))
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
  }, [shouldRender])

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
      
      const modelScale = Math.min(widthScale, heightScale) * 1.45 * live.current.scale
      model.scale.set(modelScale)
      
      model.anchor.set(0.5, 0.5)
      model.x = stageWidth / 2
      model.y = stageHeight / 2 + stageHeight * 0.28
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

  // Only observe pointer movement while it is over the model. Listening on the
  // entire window made normal chat typing and scrolling compete with Live2D.
  useEffect(() => {
    let rafId: number | null = null
    let pendingNormalized: { x: number; y: number } | null = null

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
      pendingNormalized = clampToUnitCircle(x, y)

      if (rafId === null) {
        rafId = requestAnimationFrame(() => {
          if (pendingNormalized) {
            focus(pendingNormalized.x, pendingNormalized.y)
            pendingNormalized = null
          }
          rafId = null
        })
      }
    }

    const handlePointerEnter = () => {
      if (isLocked || !isActive || document.visibilityState !== 'visible') return
      pointerInsideRef.current = true
      setRenderActive(true, true)
    }

    const handlePointerLeave = () => {
      pointerInsideRef.current = false
      centerFocus()
      setRenderActive(isActive && document.visibilityState === 'visible', false)
    }

    const container = containerRef.current
    if (!container) return
    if (isLocked) handlePointerLeave()

    container.addEventListener('pointermove', handlePointerMove)
    container.addEventListener('pointerenter', handlePointerEnter)
    container.addEventListener('pointerleave', handlePointerLeave)
    window.addEventListener('blur', centerFocus)

    return () => {
      if (rafId !== null) {
        cancelAnimationFrame(rafId)
        rafId = null
      }
      container.removeEventListener('pointermove', handlePointerMove)
      container.removeEventListener('pointerenter', handlePointerEnter)
      container.removeEventListener('pointerleave', handlePointerLeave)
      window.removeEventListener('blur', centerFocus)
    }
  }, [isLocked, isActive])

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
        <div style={{ position: 'absolute', color: 'var(--accent)', fontSize: '14px', fontFamily: 'inherit' }}>
          Loading Shiroko Live2D model...
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
