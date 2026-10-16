import { useCallback, useEffect, useId, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Paperclip, X } from 'lucide-react'
import type { AgentProgress } from '../types'
import { useSessionSources } from '../utils/useSessionSources'
import SessionSources from './SessionSources'
import '../css/session-sources-popover.css'

export default function SessionSourcesPopover({ chatId, liveProgress, open, onOpenChange }: {
  chatId: string; liveProgress: AgentProgress[]; open: boolean; onOpenChange: (open: boolean) => void
}) {
  const [position, setPosition] = useState({ left: 12, top: 64, maxHeight: 400 })
  const trigger = useRef<HTMLButtonElement>(null)
  const panel = useRef<HTMLElement>(null)
  const closeButton = useRef<HTMLButtonElement>(null)
  const id = useId()
  const { sources, loading, error, retry } = useSessionSources(chatId, liveProgress, open)
  const close = useCallback((restoreFocus = false) => {
    onOpenChange(false)
    if (restoreFocus) trigger.current?.focus()
  }, [onOpenChange])
  const reposition = useCallback(() => {
    const bounds = trigger.current?.getBoundingClientRect()
    if (!bounds) return
    const width = Math.min(360, window.innerWidth - 24)
    const top = bounds.bottom + 8
    setPosition({ left: Math.max(12, Math.min(bounds.right - width, window.innerWidth - width - 12)), top, maxHeight: Math.max(0, window.innerHeight - top - 12) })
  }, [])
  useEffect(() => {
    if (!open) return
    reposition()
    closeButton.current?.focus()
    const outside = (event: MouseEvent) => {
      if (!trigger.current?.contains(event.target as Node) && !panel.current?.contains(event.target as Node)) close()
    }
    const key = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.preventDefault(); close(true) } }
    document.addEventListener('mousedown', outside)
    window.addEventListener('keydown', key)
    window.addEventListener('resize', reposition)
    window.addEventListener('scroll', reposition, true)
    return () => {
      document.removeEventListener('mousedown', outside)
      window.removeEventListener('keydown', key)
      window.removeEventListener('resize', reposition)
      window.removeEventListener('scroll', reposition, true)
    }
  }, [open, close, reposition])
  return <>
    <button ref={trigger} type="button" className={`session-sources-toggle${open ? ' is-open' : ''}`} aria-label="Open session sources" aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? id : undefined} title="Sources for this session" onClick={() => { reposition(); onOpenChange(!open) }}>
      <Paperclip size={15} aria-hidden="true" /><span>Sources</span>
    </button>
    {open && createPortal(<section ref={panel} id={id} className="session-sources-popover" role="dialog" aria-label="Session sources" style={position}>
      <button ref={closeButton} type="button" className="session-sources-close" aria-label="Close session sources" onClick={() => close(true)}><X size={16} aria-hidden="true" /></button>
      <SessionSources key={chatId} sources={sources} loading={loading} error={error} onRetry={retry} onOpenLibrary={url => {
        window.history.pushState({}, '', url)
        window.dispatchEvent(new PopStateEvent('popstate'))
        close()
      }} />
    </section>, document.body)}
  </>
}
