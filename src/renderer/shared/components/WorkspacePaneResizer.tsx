import React, { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { readWorkspacePaneWidth, workspacePaneKeyWidth, workspacePaneLayout, WORKSPACE_PANE_STORAGE_KEY } from '../utils/workspacePane'

export default function WorkspacePaneResizer() {
  const handle = useRef<HTMLDivElement>(null)
  const [preferred, setPreferred] = useState(() => readWorkspacePaneWidth(() => localStorage.getItem(WORKSPACE_PANE_STORAGE_KEY)))
  const preferredRef = useRef(preferred)
  const [available, setAvailable] = useState(1500)
  const [dragging, setDragging] = useState(false)
  const drag = useRef<{ x: number; width: number; preferred: number; pointer: number; element: HTMLDivElement; cursor: string; select: string } | null>(null)
  const layout = workspacePaneLayout(preferred, available)

  const change = (width: number, persist = false) => {
    preferredRef.current = width
    setPreferred(width)
    if (persist) {
      try { localStorage.setItem(WORKSPACE_PANE_STORAGE_KEY, String(width)) } catch { /* Storage may be disabled. */ }
    }
  }
  const finish = (cancel = false, unmount = false) => {
    const current = drag.current
    if (!current) return
    drag.current = null
    document.body.style.cursor = current.cursor
    document.body.style.userSelect = current.select
    if (current.element.hasPointerCapture(current.pointer)) current.element.releasePointerCapture(current.pointer)
    if (!unmount) {
      setDragging(false)
      change(cancel ? current.preferred : preferredRef.current, !cancel)
    }
  }

  useLayoutEffect(() => {
    const main = handle.current?.closest<HTMLElement>('.assistant-workspace')
    if (!main) return
    const measure = () => setAvailable(main.getBoundingClientRect().width)
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(main)
    return () => {
      observer.disconnect()
      main.style.removeProperty('--workspace-panel-width')
      delete main.dataset.workspaceStacked
      main.classList.remove('is-workspace-resizing')
    }
  }, [])

  useLayoutEffect(() => {
    const main = handle.current?.closest<HTMLElement>('.assistant-workspace')
    if (!main) return
    main.style.setProperty('--workspace-panel-width', `${layout.width}px`)
    main.dataset.workspaceStacked = String(layout.stacked)
    main.classList.toggle('is-workspace-resizing', dragging)
    if (layout.stacked) finish(true)
  }, [layout.width, layout.stacked, dragging])

  useEffect(() => {
    const blur = () => finish(true)
    window.addEventListener('blur', blur)
    return () => { window.removeEventListener('blur', blur); finish(true, true) }
  }, [])

  return <div ref={handle} className="workspace-pane-resizer" role="separator" aria-label="Resize Workspace"
    aria-orientation="vertical" aria-valuemin={layout.min} aria-valuemax={layout.max} aria-valuenow={layout.width}
    aria-controls="workspace-file-panel" tabIndex={layout.stacked ? -1 : 0} hidden={layout.stacked}
    title="Drag to resize Workspace. Double-click or press Enter to reset."
    onPointerDown={event => {
      if (event.button !== 0 || drag.current || layout.stacked) return
      event.preventDefault()
      event.currentTarget.focus()
      event.currentTarget.setPointerCapture(event.pointerId)
      drag.current = { x: event.clientX, width: layout.width, preferred: preferredRef.current, pointer: event.pointerId,
        element: event.currentTarget, cursor: document.body.style.cursor, select: document.body.style.userSelect }
      document.body.style.cursor = 'col-resize'
      document.body.style.userSelect = 'none'
      setDragging(true)
    }}
    onPointerMove={event => {
      const current = drag.current
      if (current?.pointer === event.pointerId) change(workspacePaneLayout(current.width + event.clientX - current.x, available).width)
    }}
    onPointerUp={event => { if (drag.current?.pointer === event.pointerId) finish() }}
    onPointerCancel={event => { if (drag.current?.pointer === event.pointerId) finish(true) }}
    onLostPointerCapture={() => finish(true)}
    onDoubleClick={() => change(workspacePaneLayout(400, available).width, true)}
    onKeyDown={event => {
      if (event.key === 'Escape' && drag.current) { event.preventDefault(); finish(true); return }
      if (drag.current) return
      const width = workspacePaneKeyWidth(event.key, layout)
      if (width !== null) { event.preventDefault(); change(width, true) }
    }} />
}
