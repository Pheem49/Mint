interface GeminiLiveOverlayProps {
  status: 'listening' | 'speaking' | 'thinking' | 'paused'
  transcript: string
  isPaused: boolean
  onTogglePause: () => void
  onEndCall: () => void
}

const STATUS_LABEL: Record<GeminiLiveOverlayProps['status'], string> = {
  listening: 'Listening',
  speaking: 'Speaking',
  thinking: 'Thinking',
  paused: 'Paused'
}

export default function GeminiLiveOverlay({ status, transcript, isPaused, onTogglePause, onEndCall }: GeminiLiveOverlayProps) {
  return (
    <div className="gemini-live-overlay" role="dialog" aria-modal="true" aria-label="Gemini Live conversation">
      <div className="gemini-live-card">
        <span className="gemini-live-badge">Gemini Live</span>
        <div className={`gemini-live-orb ${status}`} aria-hidden="true" />
        <div className="gemini-live-status">{STATUS_LABEL[status]}</div>
        <div className="gemini-live-transcript">{transcript || 'Say something to get started...'}</div>

        <div className="gemini-live-controls">
          <button
            type="button"
            className="gemini-live-btn"
            onClick={onTogglePause}
            title={isPaused ? 'Resume' : 'Pause'}
          >
            {isPaused ? (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
                <path d="M8 5v14l11-7z" />
              </svg>
            ) : (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
                <rect x="6" y="5" width="4" height="14" rx="1" />
                <rect x="14" y="5" width="4" height="14" rx="1" />
              </svg>
            )}
          </button>
          <button
            type="button"
            className="gemini-live-btn gemini-live-end"
            onClick={onEndCall}
            title="End Live conversation"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>
    </div>
  )
}
