import { useEffect, useRef, useState } from 'react'
import '../css/pull-to-refresh.css'

const SCROLL_SURFACES = '.chat-container, .pictures-library, .img-studio-content, .veo-studio-content, .web-view-scroll'
const BLOCKED_TARGETS = 'input, textarea, select, button, [contenteditable="true"], .workspace-sidebar, .sidebar-backdrop, [role="dialog"], .model-spotlight-popover'
const REFRESH_THRESHOLD = 64

type Gesture = {
  startX: number
  startY: number
  surface: HTMLElement
  distance: number
}

export default function PullToRefresh({ disabled }: { disabled: boolean }) {
  const [distance, setDistance] = useState(0)
  const [refreshing, setRefreshing] = useState(false)
  const gesture = useRef<Gesture | null>(null)
  const refreshingRef = useRef(false)

  useEffect(() => {
    let reloadTimer: ReturnType<typeof setTimeout> | undefined

    const reset = () => {
      gesture.current = null
      setDistance(0)
    }

    const onTouchStart = (event: TouchEvent) => {
      gesture.current = null
      if (disabled || refreshingRef.current || event.touches.length !== 1 || !window.matchMedia('(max-width: 760px)').matches) return
      if (document.querySelector('#send-btn.stop-btn')) return

      const target = event.target
      if (!(target instanceof Element) || target.closest(BLOCKED_TARGETS)) return
      const surface = target.closest(SCROLL_SURFACES)
      if (!(surface instanceof HTMLElement) || surface.scrollTop > 1) return

      // A nested list must reach its own top before the page can refresh.
      for (let node = target.parentElement; node && node !== surface; node = node.parentElement) {
        const overflow = window.getComputedStyle(node).overflowY
        if ((overflow === 'auto' || overflow === 'scroll') && node.scrollHeight > node.clientHeight + 1 && node.scrollTop > 1) return
      }

      gesture.current = {
        startX: event.touches[0].clientX,
        startY: event.touches[0].clientY,
        surface,
        distance: 0,
      }
    }

    const onTouchMove = (event: TouchEvent) => {
      const current = gesture.current
      if (!current || event.touches.length !== 1) return
      const deltaX = event.touches[0].clientX - current.startX
      const deltaY = event.touches[0].clientY - current.startY
      if (deltaY < 0 || (Math.abs(deltaX) > Math.abs(deltaY) && Math.abs(deltaX) > 8) || current.surface.scrollTop > 1) {
        reset()
        return
      }
      if (deltaY < 8) return
      event.preventDefault()
      current.distance = Math.min(96, deltaY * 0.72)
      setDistance(current.distance)
    }

    const onTouchEnd = () => {
      const shouldRefresh = (gesture.current?.distance ?? 0) >= REFRESH_THRESHOLD
      gesture.current = null
      if (shouldRefresh) {
        refreshingRef.current = true
        setRefreshing(true)
        setDistance(REFRESH_THRESHOLD)
        reloadTimer = setTimeout(() => window.location.reload(), 180)
      } else {
        setDistance(0)
      }
    }

    document.addEventListener('touchstart', onTouchStart, { passive: true })
    document.addEventListener('touchmove', onTouchMove, { passive: false })
    document.addEventListener('touchend', onTouchEnd)
    document.addEventListener('touchcancel', reset)
    return () => {
      document.removeEventListener('touchstart', onTouchStart)
      document.removeEventListener('touchmove', onTouchMove)
      document.removeEventListener('touchend', onTouchEnd)
      document.removeEventListener('touchcancel', reset)
      if (reloadTimer) clearTimeout(reloadTimer)
    }
  }, [disabled])

  const visible = distance > 0 || refreshing
  const ready = distance >= REFRESH_THRESHOLD
  return (
    <div
      className={`mint-pull-refresh${visible ? ' is-visible' : ''}${refreshing ? ' is-refreshing' : ''}`}
      style={{ transform: `translate(-50%, ${Math.min(distance / 2, 32)}px)` }}
      role="status"
      aria-live="polite"
    >
      <span className="mint-pull-refresh-icon" aria-hidden="true">↻</span>
      <span>{refreshing ? 'Refreshing…' : ready ? 'Release to refresh' : 'Pull to refresh'}</span>
    </div>
  )
}
