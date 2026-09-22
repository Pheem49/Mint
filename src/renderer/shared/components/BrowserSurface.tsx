import { useEffect, useState } from 'react'

interface Props {
  url: string
  onNavigate: (url: string) => void
  onClose: () => void
  onOpenExternal: (url: string) => void
}

function normalizeUrl(value: string) {
  const trimmed = value.trim()
  if (!trimmed) return 'https://www.google.com'
  return /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`
}

export default function BrowserSurface({ url, onNavigate, onClose, onOpenExternal }: Props) {
  const [address, setAddress] = useState(url)
  const [history, setHistory] = useState([url])
  const [historyIndex, setHistoryIndex] = useState(0)
  const [frameKey, setFrameKey] = useState(0)
  const [frameUrl, setFrameUrl] = useState<string | null>(null)

  useEffect(() => {
    setAddress(url)
    setFrameUrl(null)
    const frame = window.requestAnimationFrame(() => setFrameUrl(url))
    return () => window.cancelAnimationFrame(frame)
  }, [url])

  const navigate = () => {
    const next = normalizeUrl(address)
    setAddress(next)
    setHistory((current) => [...current.slice(0, historyIndex + 1), next])
    setHistoryIndex((current) => current + 1)
    onNavigate(next)
  }

  const moveHistory = (offset: number) => {
    const nextIndex = historyIndex + offset
    if (nextIndex < 0 || nextIndex >= history.length) return
    const next = history[nextIndex]
    setHistoryIndex(nextIndex)
    setAddress(next)
    onNavigate(next)
  }

  return (
    <section className="browser-surface" aria-label="Browser">
      <header className="browser-surface-toolbar">
        <button type="button" onClick={() => moveHistory(-1)} disabled={historyIndex === 0} aria-label="Go back" title="Back">‹</button>
        <button type="button" onClick={() => moveHistory(1)} disabled={historyIndex >= history.length - 1} aria-label="Go forward" title="Forward">›</button>
        <button type="button" onClick={() => setFrameKey((key) => key + 1)} aria-label="Reload page" title="Reload">↻</button>
        <form className="browser-surface-address" onSubmit={(event) => { event.preventDefault(); navigate() }}>
          <span aria-hidden="true">◎</span>
          <input value={address} onChange={(event) => setAddress(event.target.value)} aria-label="Web address" spellCheck={false} />
          <button type="submit" aria-label="Navigate" title="Go">Go</button>
        </form>
        <button type="button" onClick={onClose} aria-label="Close browser" title="Close browser">×</button>
      </header>
      <div className="browser-surface-frame-wrap">
        {frameUrl ? <iframe key={`${frameUrl}:${frameKey}`} loading="lazy" className="browser-surface-frame" src={frameUrl} title={frameUrl} /> : <div className="browser-surface-loading">Loading browser…</div>}
        <div className="browser-surface-hint">
          <span>Some websites block embedded previews.</span>
          <button type="button" onClick={() => onOpenExternal(url)}>Open in Mint Browser</button>
        </div>
      </div>
    </section>
  )
}
