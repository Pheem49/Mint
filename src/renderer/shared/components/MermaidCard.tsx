import { useEffect, useState } from 'react'
import { Workflow } from 'lucide-react'
import { ChatCodeBlock } from './ChatCodeBlock'

let diagramCounter = 0

// `mermaid.initialize()` + `mermaid.render()` mutate shared state on the single
// imported `mermaid` module. When several MermaidCards mount at once (e.g. opening
// a chat whose history has multiple diagrams), their render() calls interleave and
// clobber each other — one card's initialize() can land mid-flight through
// another's render(), so the loser silently resolves with an empty/corrupted SVG
// instead of throwing. Serialize every card's render through one queue so only one
// mermaid.initialize()+render() pair is ever in flight at a time.
let renderQueue: Promise<unknown> = Promise.resolve()
function enqueueMermaidRender<T>(task: () => Promise<T>): Promise<T> {
  const run = renderQueue.then(task, task)
  renderQueue = run.then(
    () => undefined,
    () => undefined,
  )
  return run
}

// Rendered SVGs are cached by (theme, source) so revisiting a chat — switching
// away and back, reopening old history — reuses the previous result instead of
// re-running mermaid's layout pass from scratch every time the card remounts.
// Keyed by theme too, since the same source renders different colors per theme.
const svgCache = new Map<string, string>()
function cacheKey(code: string): string {
  const theme = document.documentElement.getAttribute('data-theme') || 'dark'
  return `${theme}::${code}`
}

function buildMermaidTheme(): Record<string, string> {
  const cs = getComputedStyle(document.documentElement)
  const v = (name: string, fallback: string) => cs.getPropertyValue(name).trim() || fallback
  return {
    background: v('--panel-bg', '#1e1e20'),
    primaryColor: v('--accent', '#10b981'),
    primaryTextColor: v('--text-main', '#e8e8ea'),
    primaryBorderColor: v('--border', 'rgba(255,255,255,0.15)'),
    lineColor: v('--text-muted', '#8f8f94'),
    textColor: v('--text-main', '#e8e8ea'),
    secondaryColor: v('--panel-raised', '#2a2a2d'),
    tertiaryColor: v('--panel-soft', '#242426'),
  }
}

// When a parse fails, mermaid still leaves its throwaway render/measurement
// containers attached to <body> — the `d`-prefixed sizing div and, on older
// minors that ignore `suppressErrorRendering`, the "Syntax error" bomb graphic.
// They carry no styling of ours, so they escape the card and show up as a stray
// white strip pinned to the page edge. Sweep any such node that mermaid parked
// directly on <body>; a successfully rendered diagram lives inside a card div,
// so the parent check leaves it alone.
function cleanupOrphanMermaidNodes(): void {
  document
    .querySelectorAll('[id^="dmermaid-diagram-"], [id^="mermaid-diagram-"]')
    .forEach((el) => {
      if (el.parentElement === document.body) el.remove()
    })
}

type Status = 'loading' | 'ready' | 'error'

export default function MermaidCard({ code }: { code: string }) {
  const [status, setStatus] = useState<Status>('loading')
  const [svg, setSvg] = useState<string>('')
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    let cancelled = false
    let observer: MutationObserver | null = null
    let debounceTimer: ReturnType<typeof setTimeout> | null = null

    async function render() {
      const cached = svgCache.get(cacheKey(code))
      if (cached !== undefined) {
        // Already rendered this exact source under the current theme — reuse it
        // instead of re-running mermaid, which also sidesteps a real bug: calling
        // mermaid.render() again with a DOM id that's already in use elsewhere on
        // the page (the SVG we rendered last time) makes mermaid tear that live
        // element down as part of preparing its own render target.
        setSvg(cached)
        setStatus('ready')
        return
      }
      // Let the browser paint the loading spinner before the heavy, synchronous
      // mermaid layout computation (cose-bilkent for mindmaps especially) blocks
      // the main thread — otherwise the frozen tab can flash unpainted regions
      // before the spinner state ever reaches the screen.
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      if (cancelled) return
      try {
        const result = await enqueueMermaidRender(async () => {
          const mermaid = (await import('mermaid')).default
          mermaid.initialize({
            startOnLoad: false,
            securityLevel: 'strict',
            theme: 'base',
            themeVariables: buildMermaidTheme(),
            // Don't let mermaid inject its own "Syntax error" bomb graphic into
            // the DOM on a parse failure — we render our own fallback (the raw
            // fenced source) from the `error` status below.
            suppressErrorRendering: true,
          })
          // Validate before rendering. `parse` with `suppressErrors` resolves
          // falsy instead of throwing/DOM-injecting on bad syntax — models
          // (especially smaller local ones) routinely emit invalid mermaid, and
          // this keeps that off the main render path entirely.
          const parsed = await mermaid.parse(code, { suppressErrors: true })
          if (!parsed) throw new Error('Invalid mermaid syntax')
          // A fresh id every call, not one fixed per component instance — mermaid
          // treats the id as a DOM element id it may look up and reuse, so reusing
          // the same id across repeat renders risks it colliding with (and tearing
          // down) the SVG from a previous successful render still on screen.
          return mermaid.render(`mermaid-diagram-${++diagramCounter}`, code)
        })
        if (cancelled) return
        svgCache.set(cacheKey(code), result.svg)
        setSvg(result.svg)
        setStatus('ready')
      } catch (err) {
        console.error('Mermaid render failed:', err)
        cleanupOrphanMermaidNodes()
        if (!cancelled) setStatus('error')
      }
    }

    const cached = svgCache.get(cacheKey(code))
    if (cached !== undefined) {
      setSvg(cached)
      setStatus('ready')
    } else {
      // While the AI's response is still streaming in, `code` (the fenced block's
      // text) changes on nearly every token — each change re-runs this effect, and
      // mermaid's layout pass (cose-bilkent for mindmaps) is CPU-heavy, so firing it
      // on every partial, usually syntactically-invalid chunk keeps the main thread
      // busy for the entire streaming duration instead of once. Debounce so we only
      // attempt a real render after the fence has stopped changing for a moment.
      setStatus('loading')
      debounceTimer = setTimeout(render, 400)
    }

    observer = new MutationObserver(() => render())
    observer.observe(document.documentElement, { attributeFilter: ['data-theme'] })

    return () => {
      cancelled = true
      if (debounceTimer) clearTimeout(debounceTimer)
      observer?.disconnect()
      cleanupOrphanMermaidNodes()
    }
  }, [code])

  if (status === 'error') {
    return <ChatCodeBlock code={code} language="mermaid" />
  }

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy diagram source:', err)
    }
  }

  const handleDownload = () => {
    try {
      const blob = new Blob([svg], { type: 'image/svg+xml' })
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = 'diagram.svg'
      link.click()
      URL.revokeObjectURL(url)
    } catch (err) {
      console.error('Failed to download diagram:', err)
    }
  }

  return (
    <div
      style={{
        background: 'rgba(255, 255, 255, 0.03)',
        backdropFilter: 'blur(16px)',
        WebkitBackdropFilter: 'blur(16px)',
        color: '#f8fafc',
        borderRadius: '12px',
        padding: '16px 20px',
        margin: '12px 0',
        boxShadow: '0 8px 32px 0 rgba(0, 0, 0, 0.2)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        fontFamily: 'system-ui, -apple-system, sans-serif',
        minWidth: 0,
        maxWidth: '100%',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px' }}>
        <Workflow size={16} strokeWidth={2} style={{ opacity: 0.9 }} />
        <span
          style={{
            fontSize: '11px',
            fontWeight: 700,
            letterSpacing: '0.8px',
            color: 'rgba(255, 255, 255, 0.6)',
          }}
        >
          DIAGRAM
        </span>
        {status === 'ready' && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px', marginLeft: 'auto' }}>
            <button type="button" onClick={handleDownload} title="Download SVG" className="chat-code-action-btn">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
            </button>
            <button type="button" onClick={handleCopy} title="Copy diagram source" className="chat-code-action-btn">
              {copied ? (
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              ) : (
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              )}
            </button>
          </div>
        )}
      </div>

      {status === 'loading' ? (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', padding: '32px 0' }}>
          <div
            style={{
              width: '20px',
              height: '20px',
              borderRadius: '50%',
              border: '2px solid rgba(255, 255, 255, 0.15)',
              borderTopColor: 'var(--accent, #10b981)',
              animation: 'mermaid-card-spin 0.8s linear infinite',
            }}
          />
          <style>{'@keyframes mermaid-card-spin { to { transform: rotate(360deg) } }'}</style>
        </div>
      ) : (
        <div
          style={{ overflowX: 'auto', display: 'flex', justifyContent: 'center', minWidth: 0 }}
          // eslint-disable-next-line react/no-danger
          dangerouslySetInnerHTML={{ __html: svg }}
        />
      )}
    </div>
  )
}
