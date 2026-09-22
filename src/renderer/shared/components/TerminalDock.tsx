import { useCallback, useEffect, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'

interface TerminalDockProps {
  visible: boolean
  page?: boolean
  position: 'bottom' | 'right'
  size: number
  cwd?: string | null
  onClose: () => void
  onTogglePosition: () => void
  onResize: (size: number) => void
}

export default function TerminalDock({ visible, page = false, position, size, cwd, onClose, onTogglePosition, onResize }: TerminalDockProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitRef = useRef<FitAddon | null>(null)
  const sessionRef = useRef<string | null>(null)
  const decoderRef = useRef(new TextDecoder())
  const resizeCleanupRef = useRef<(() => void) | null>(null)
  const [resizing, setResizing] = useState(false)
  const [terminalReady, setTerminalReady] = useState(false)

  // Let the tab/header paint before xterm allocates its canvas, measurements,
  // and event listeners. This keeps opening a terminal responsive on slower
  // machines while the shell starts immediately after the first frame.
  useEffect(() => {
    const frame = window.requestAnimationFrame(() => setTerminalReady(true))
    return () => window.cancelAnimationFrame(frame)
  }, [])

  const clampSize = useCallback((next: number) => {
    const min = position === 'bottom' ? 180 : 280
    const max = position === 'bottom'
      ? Math.max(min, window.innerHeight - 180)
      : Math.max(min, Math.min(900, window.innerWidth - 560))
    return Math.round(Math.min(max, Math.max(min, next)))
  }, [position])

  const startResize = (event: ReactMouseEvent<HTMLDivElement>) => {
    event.preventDefault()
    const startCoordinate = position === 'bottom' ? event.clientY : event.clientX
    const startSize = size
    const previousCursor = document.body.style.cursor
    const previousUserSelect = document.body.style.userSelect
    document.body.style.cursor = position === 'bottom' ? 'row-resize' : 'col-resize'
    document.body.style.userSelect = 'none'
    setResizing(true)

    const handleMove = (moveEvent: MouseEvent) => {
      const current = position === 'bottom' ? moveEvent.clientY : moveEvent.clientX
      onResize(clampSize(startSize + startCoordinate - current))
    }
    const handleUp = () => {
      window.removeEventListener('mousemove', handleMove)
      window.removeEventListener('mouseup', handleUp)
      document.body.style.cursor = previousCursor
      document.body.style.userSelect = previousUserSelect
      resizeCleanupRef.current = null
      setResizing(false)
    }
    resizeCleanupRef.current?.()
    resizeCleanupRef.current = handleUp
    window.addEventListener('mousemove', handleMove)
    window.addEventListener('mouseup', handleUp)
  }

  const resizeWithKeyboard = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const increase = position === 'bottom' ? event.key === 'ArrowUp' : event.key === 'ArrowLeft'
    const decrease = position === 'bottom' ? event.key === 'ArrowDown' : event.key === 'ArrowRight'
    if (!increase && !decrease) return
    event.preventDefault()
    onResize(clampSize(size + (increase ? 16 : -16)))
  }

  const fit = useCallback(() => {
    const terminal = terminalRef.current
    const addon = fitRef.current
    const sessionId = sessionRef.current
    if (!terminal || !addon || !sessionId || !visible) return
    try {
      addon.fit()
      void invoke('resize_interactive_terminal', {
        sessionId,
        cols: terminal.cols,
        rows: terminal.rows,
      }).catch((error) => terminal.write(`\r\n\x1b[31mTerminal resize failed: ${String(error)}\x1b[0m\r\n`))
    } catch {
      // The panel may briefly have zero size during a window transition.
    }
  }, [visible])

  useEffect(() => {
    if (!terminalReady) return
    if (!hostRef.current) return
    const terminal = new Terminal({
      cursorBlink: true,
      convertEol: true,
      fontFamily: '"Fira Code", "Cascadia Code", monospace',
      fontSize: 13,
      theme: {
        background: '#111318',
        foreground: '#e5e7eb',
        cursor: '#7dd3a7',
        selectionBackground: '#365c4a',
      },
    })
    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)
    terminal.open(hostRef.current)
    terminalRef.current = terminal
    fitRef.current = fitAddon

    const dataListener = terminal.onData((data) => {
      const sessionId = sessionRef.current
      if (sessionId) void invoke('write_interactive_terminal', { sessionId, data })
    })
    let unlistenOutput: (() => void) | undefined
    let unlistenExit: (() => void) | undefined
    let mounted = true
    void listen<{ sessionId: string; data: string }>('terminal-output', (event) => {
      if (event.payload.sessionId === sessionRef.current) {
        const bytes = Uint8Array.from(atob(event.payload.data), (char) => char.charCodeAt(0))
        terminal.write(decoderRef.current.decode(bytes, { stream: true }))
      }
    }).then((unlisten) => {
      if (mounted) unlistenOutput = unlisten
      else unlisten()
    })
    void listen<{ sessionId: string; exitCode: number | null }>('terminal-exit', (event) => {
      if (event.payload.sessionId === sessionRef.current) {
        const trailingText = decoderRef.current.decode()
        if (trailingText) terminal.write(trailingText)
        sessionRef.current = null
        terminal.write(`\r\n\x1b[90m[process exited${event.payload.exitCode == null ? '' : ` with code ${event.payload.exitCode}`} ]\x1b[0m\r\n`)
      }
    }).then((unlisten) => {
      if (mounted) unlistenExit = unlisten
      else unlisten()
    })

    return () => {
      mounted = false
      dataListener.dispose()
      unlistenOutput?.()
      unlistenExit?.()
      if (sessionRef.current) void invoke('stop_interactive_terminal', { sessionId: sessionRef.current })
      sessionRef.current = null
      terminal.dispose()
      terminalRef.current = null
      fitRef.current = null
    }
  }, [terminalReady])

  useEffect(() => {
    if (!terminalReady || !visible || sessionRef.current) return
    const terminal = terminalRef.current
    if (!terminal) return
    terminal.clear()
    decoderRef.current = new TextDecoder()
    terminal.write('\x1b[90mStarting shell…\x1b[0m\r\n')
    let cancelled = false
    void invoke<string>('start_interactive_terminal', { cwd: cwd || null }).then((sessionId) => {
      if (cancelled) {
        void invoke('stop_interactive_terminal', { sessionId })
        return
      }
      sessionRef.current = sessionId
      terminal.clear()
      requestAnimationFrame(() => {
        fitRef.current?.fit()
        if (terminalRef.current) {
          void invoke('resize_interactive_terminal', {
            sessionId,
            cols: terminal.cols,
            rows: terminal.rows,
          })
          terminal.focus()
        }
      })
    }).catch((error) => {
      terminal.write(`\r\n\x1b[31mCould not start terminal: ${String(error)}\x1b[0m\r\n`)
    })
    return () => { cancelled = true }
  }, [terminalReady, visible, cwd])

  useEffect(() => {
    if (!terminalReady || !visible || !hostRef.current) return
    const observer = new ResizeObserver(fit)
    observer.observe(hostRef.current)
    requestAnimationFrame(fit)
    return () => observer.disconnect()
  }, [terminalReady, visible, fit])

  useEffect(() => () => resizeCleanupRef.current?.(), [])

  return (
    <section
      className={`terminal-dock ${visible ? 'is-open' : ''} ${page ? 'is-page' : ''} ${position === 'right' ? 'is-right' : ''} ${resizing ? 'is-resizing' : ''}`}
      style={{ '--terminal-dock-size': `${size}px` } as CSSProperties}
      aria-hidden={!visible}
    >
      {visible && (
        <div
          className="terminal-dock-resize-handle"
          role="separator"
          aria-label={position === 'bottom' ? 'Resize terminal height' : 'Resize terminal width'}
          aria-orientation={position === 'bottom' ? 'horizontal' : 'vertical'}
          aria-valuenow={size}
          tabIndex={0}
          onMouseDown={startResize}
          onKeyDown={resizeWithKeyboard}
        />
      )}
      <header className="terminal-dock-header">
        <div className="terminal-dock-title"><span className="terminal-dock-status" />Terminal{cwd ? <span className="terminal-dock-cwd">{cwd}</span> : null}</div>
        <div className="terminal-dock-header-actions">
          <button type="button" onClick={onTogglePosition} aria-label={position === 'bottom' ? 'Move terminal to the right' : 'Move terminal to the bottom'} title={position === 'bottom' ? 'Move terminal to the right' : 'Move terminal to the bottom'}>
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              {position === 'bottom' ? <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 14h18" /></> : <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M14 4v16" /></>}
            </svg>
          </button>
          <button type="button" onClick={onClose} aria-label="Close terminal" title="Close terminal">×</button>
        </div>
      </header>
      <div className="terminal-dock-surface" ref={hostRef} />
    </section>
  )
}
