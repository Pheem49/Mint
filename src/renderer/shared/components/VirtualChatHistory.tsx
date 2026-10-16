import { forwardRef, memo, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react'
import { defaultRangeExtractor, useVirtualizer, type Range } from '@tanstack/react-virtual'

export interface VirtualChatHistoryHandle {
  scrollToEnd: () => void
}

interface Props {
  items: Array<{ id: string | number }>
  scrollRef: RefObject<HTMLDivElement | null>
  renderRow: (index: number) => ReactNode
  tail?: ReactNode
  hasOlder?: boolean
}

// The live turn is a measured row too: image, tool, and approval growth obey the
// same anchoring policy as history. Only the viewport and a few neighbors mount.
export const VirtualChatHistory = memo(forwardRef<VirtualChatHistoryHandle, Props>(function VirtualChatHistory({ items, scrollRef, renderRow, tail, hasOlder }, ref) {
  const container = useRef<HTMLDivElement>(null)
  const [scrollMargin, setScrollMargin] = useState(0)
  const measuredMargin = useRef<number | null>(null)
  // Parent DOM refs attach after child layout effects. Publish the scroll host
  // after the commit so a newly mounted list always installs its observers.
  const [scrollElement, setScrollElement] = useState<HTMLDivElement | null>(null)
  useEffect(() => { setScrollElement(scrollRef.current) }, [scrollRef])
  const getItemKey = useCallback((index: number) => index < items.length ? `turn:${items[index].id}` : 'live-turn', [items])
  const hasTail = Boolean(tail)
  const rangeExtractor = useCallback((range: Range) => {
    const indexes = defaultRangeExtractor(range)
    // Approval inputs and other live controls must retain their local state
    // while the reader scrolls into history. Keep just this one extra row alive.
    if (hasTail && !indexes.includes(items.length)) indexes.push(items.length)
    return indexes
  }, [items.length, hasTail])
  const virtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count: items.length + (tail ? 1 : 0),
    getScrollElement: () => scrollElement,
    getItemKey,
    rangeExtractor,
    estimateSize: () => 320,
    overscan: 4,
    gap: 10,
    scrollMargin,
    anchorTo: 'end',
    followOnAppend: true,
    scrollEndThreshold: 240,
    useAnimationFrameWithResizeObserver: true,
  })

  useImperativeHandle(ref, () => ({ scrollToEnd: () => virtualizer.scrollToEnd() }), [virtualizer])

  useLayoutEffect(() => {
    const scroll = scrollElement
    const list = container.current
    if (!scroll || !list) return
    const measureStart = () => {
      const margin = list.getBoundingClientRect().top - scroll.getBoundingClientRect().top - scroll.clientTop + scroll.scrollTop
      const previous = measuredMargin.current
      measuredMargin.current = margin
      setScrollMargin(margin)
      // A disappearing Load older button shifts the list without changing a
      // row's measured size. Compensate that prefix separately from row anchors.
      if (previous !== null && previous !== margin && scroll.scrollTop > Math.min(previous, margin)) {
        virtualizer.scrollToOffset(scroll.scrollTop + margin - previous)
      }
    }
    measureStart()
    const observer = new ResizeObserver(measureStart)
    observer.observe(scroll)
    return () => observer.disconnect()
  }, [scrollElement, hasOlder, virtualizer])

  useLayoutEffect(() => { if (scrollElement) virtualizer.scrollToEnd() }, [virtualizer, scrollElement])

  return <div ref={container} className="virtual-chat-history" style={{ position: 'relative', width: '100%', height: virtualizer.getTotalSize(), flexShrink: 0, overflowAnchor: 'none' }}>
    {virtualizer.getVirtualItems().map(row => <div key={row.key} data-index={row.index}
      ref={virtualizer.measureElement} className="virtual-chat-row"
      style={{ position: 'absolute', top: 0, left: 0, width: '100%', display: 'flow-root', transform: `translateY(${row.start - scrollMargin}px)` }}>
      {row.index < items.length ? renderRow(row.index) : tail}
    </div>)}
  </div>
}))
