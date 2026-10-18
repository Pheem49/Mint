import React, { useEffect, useRef, useState, type RefObject } from 'react'
import * as PIXI from 'pixi.js'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { Status } from './state'
import { calculateGazeTarget, smoothGaze, calculateStatusGesture } from './tracking'

// Ensure PIXI is available globally for the live2d library
;(window as any).PIXI = PIXI

interface Live2DStageProps {
  mouthLevel?: RefObject<number>
  mouthActive?: RefObject<boolean>
  scale: number
  offsetY?: number
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

const MAX_DEVICE_PIXEL_RATIO = 1.25
const MAX_FPS = 60
const GAZE_TRACKING_FACTOR = 0.22
const GAZE_DEADZONE = 0.003
const GLOBAL_POLL_INTERVAL_MS = 20

export default function Live2DStage({ mouthLevel, mouthActive, scale, offsetY = 0, expressionIndex, accessoryIndex, isLocked, isActive = true, onLoadComplete, onLoadError, status = 'idle' }: Live2DStageProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const modelRef = useRef<any>(null)
  const appRef = useRef<PIXI.Application | null>(null)
  const [loading, setLoading] = useState(true)
  const [shouldRender, setShouldRender] = useState(isActive)
  const baseWidthRef = useRef<number | null>(null)
  const baseHeightRef = useRef<number | null>(null)
  const targetGazeRef = useRef({ x: 0, y: 0 })
  const live = useRef({ scale, offsetY, expressionIndex, accessoryIndex, isLocked, isActive, status, changedAt: performance.now() })
  if (live.current.status !== status) live.current.changedAt = performance.now()
  Object.assign(live.current, { scale, offsetY, expressionIndex, accessoryIndex, isLocked, isActive, status })
  const mouth = useRef(mouthLevel); mouth.current = mouthLevel
  const speaking = useRef(mouthActive); speaking.current = mouthActive
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

  const setRenderActive = (active: boolean) => {
    const app = appRef.current
    const model = modelRef.current
    if (!app) return

    app.ticker.maxFPS = MAX_FPS
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
          sharedTicker: true,
          resizeTo: containerRef.current!,
        })
        appInstance.ticker.maxFPS = MAX_FPS
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
          
          const modelScale = Math.min(widthScale, heightScale) * 1.55 * live.current.scale
          m.scale.set(modelScale)
          
          // Center the character within the floating stage.
          m.anchor.set(0.5, 0.5)
          m.x = stageWidth / 2
          m.y = stageHeight / 2 + stageHeight * (0.12 - (live.current.offsetY || 0))
        }

        fitModel()
        
        // Handle resizing
        appInstance.renderer.on('resize', fitModel)

        // Disable built-in auto-interaction to avoid conflicting pointer listeners
        model.interactive = false
        if ('autoInteract' in model) {
          ;(model as any).autoInteract = false
        }

        const focusController = model.internalModel.focusController
        // Update gaze target smoothly right before internal updateFocus() runs
        focusController.update = () => {
          const current = { x: focusController.x, y: focusController.y }
          const target = live.current.isLocked ? { x: 0, y: 0 } : targetGazeRef.current
          const smoothed = smoothGaze(current, target, GAZE_TRACKING_FACTOR, GAZE_DEADZONE)

          focusController.x = smoothed.x
          focusController.y = smoothed.y
          focusController.targetX = target.x
          focusController.targetY = target.y
          focusController.vx = 0
          focusController.vy = 0
        }

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
              coreModel.setParameterValueById(
                parameter.Id,
                parameter.Blend === 'Overwrite'
                  ? parameter.Value
                  : parameter.Blend === 'Multiply'
                  ? value * parameter.Value
                  : value + parameter.Value
              )
            }
          }
          if (speaking.current?.current) coreModel.setParameterValueById('ParamMouthOpenY', Math.max(0, Math.min(1, mouth.current?.current ?? 0)))
          if (!reducedMotion.current) {
            const elapsed = (performance.now() - live.current.changedAt) / 1000
            const gesture = calculateStatusGesture(live.current.status, elapsed)
            if (gesture.angleZ !== 0) {
              coreModel.addParameterValueById('ParamAngleZ', gesture.angleZ)
            }
            if (gesture.angleY !== 0) {
              coreModel.addParameterValueById('ParamAngleY', gesture.angleY)
            }
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
        // Detach this renderer before preserving the shared ticker. Leaving its
        // callback registered would invoke a destroyed renderer on later frames.
        appInstance.ticker.remove(appInstance.render, appInstance)
        ;(appInstance as any)._ticker = null
        appInstance.destroy(false, {
          children: true,
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
      
      const modelScale = Math.min(widthScale, heightScale) * 1.55 * live.current.scale
      model.scale.set(modelScale)
      
      model.anchor.set(0.5, 0.5)
      model.x = stageWidth / 2
      model.y = stageHeight / 2 + stageHeight * (0.12 - (live.current.offsetY || 0))
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
  }, [scale, offsetY, loading, isActive])

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

  // Track pointer across the companion window and across the entire OS desktop.
  // When inside the window, pointermove provides zero-latency local tracking.
  // When outside the window, Tauri's global cursor position query allows the character
  // to look towards the cursor anywhere on the desktop.
  useEffect(() => {
    let lastLocalMove = 0
    let active = true
    let pollInterval: ReturnType<typeof setInterval> | null = null

    const handlePointerMove = (event: PointerEvent) => {
      lastLocalMove = performance.now()
      if (live.current.isLocked || !live.current.isActive || document.visibilityState !== 'visible') {
        targetGazeRef.current = { x: 0, y: 0 }
        return
      }

      const container = containerRef.current
      if (!container) return

      const rect = container.getBoundingClientRect()
      const faceCenterRatio = 0.42 - (live.current.offsetY || 0)
      targetGazeRef.current = calculateGazeTarget({ x: event.clientX, y: event.clientY }, rect, faceCenterRatio)
    }

    // Poll global cursor position when cursor is outside the companion window
    pollInterval = setInterval(async () => {
      if (!active) return
      // If pointer is moving inside the window or app is locked/inactive, skip global poll
      if (performance.now() - lastLocalMove < 100) return
      if (live.current.isLocked || !live.current.isActive || document.visibilityState !== 'visible') {
        targetGazeRef.current = { x: 0, y: 0 }
        return
      }

      try {
        const pos = await invoke<{ x: number; y: number } | null>('get_cursor_position')
        if (pos && active) {
          const container = containerRef.current
          if (container) {
            const rect = container.getBoundingClientRect()
            const faceCenterRatio = 0.42 - (live.current.offsetY || 0)
            targetGazeRef.current = calculateGazeTarget({ x: pos.x, y: pos.y }, rect, faceCenterRatio)
          }
        }
      } catch {
        // Fall back gracefully if not running in Tauri (e.g. browser preview or tests)
      }
    }, GLOBAL_POLL_INTERVAL_MS)

    window.addEventListener('pointermove', handlePointerMove)

    return () => {
      active = false
      if (pollInterval) clearInterval(pollInterval)
      window.removeEventListener('pointermove', handlePointerMove)
    }
  }, [])

  return (
    <div
      ref={containerRef}
      data-tauri-drag-region
      onPointerDown={event => {
        if (event.button === 0 && !isLocked) {
          event.preventDefault()
          try { getCurrentWindow().startDragging().catch(() => {}) } catch {}
        }
      }}
      className={isLocked ? '' : 'cursor-grab active:cursor-grabbing'}
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
        data-tauri-drag-region
        onPointerDown={event => {
          if (event.button === 0 && !isLocked) {
            event.preventDefault()
            try { getCurrentWindow().startDragging().catch(() => {}) } catch {}
          }
        }}
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
