import { useEffect, useState } from 'react'

export function ContextCompactionStatus({ elapsedSeconds }: { elapsedSeconds?: number }) {
  const [seconds, setSeconds] = useState(0)
  useEffect(() => {
    const started = Date.now()
    const timer = setInterval(() => setSeconds(Math.floor((Date.now() - started) / 1000)), 250)
    return () => clearInterval(timer)
  }, [])
  return <div role="status" aria-live="polite" className="context-compaction-status">
    <div className="thinking-status">
      <span className="thinking-status-label"><span aria-hidden="true">✦ </span>Compacting context</span>
      <span className="thinking-status-meta">{` · ${elapsedSeconds ?? seconds}s · Esc to cancel`}</span>
    </div>
    <div className="thinking-status-meta">Summarizing earlier steps to continue working</div>
  </div>
}
