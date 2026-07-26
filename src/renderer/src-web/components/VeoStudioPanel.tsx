import React, { useState, useRef, useCallback, useEffect } from 'react'
import {
  generateVideo,
  getVideoGenProviders,
  convertFileSrc,
  type VideoGenRequest,
  type VideoGenResponse,
  type VideoGenProviders,
  type VideoGenEntry,
} from '../tauri'

type AspectRatio = '16:9' | '9:16' | '1:1'
type Duration = 5 | 8

interface VeoStudioPanelProps {
  view: string
  onSendToChat?: (prompt: string) => void
  onToggleMobileSidebar?: () => void
}

const ASPECT_OPTIONS: { value: AspectRatio; label: string; icon: string }[] = [
  { value: '16:9', label: '16:9', icon: '▬' },
  { value: '9:16', label: '9:16', icon: '▮' },
  { value: '1:1',  label: '1:1',  icon: '⬛' },
]

const DURATION_OPTIONS: { value: Duration; label: string }[] = [
  { value: 5, label: '5s' },
  { value: 8, label: '8s' },
]

const STYLE_SUGGESTIONS = [
  'cinematic', 'slow motion', 'time-lapse', 'aerial view',
  'documentary', 'animation', 'action', 'nature',
]

const PROVIDER_MODELS: Record<string, { value: string; label: string }[]> = {
  veo: [
    { value: 'veo-2.0-generate-001', label: 'Veo 2.0 Generate (Default)' },
    { value: 'veo-2.0-flash-001', label: 'Veo 2.0 Flash' },
  ],
}

const PROVIDER_LABELS: Record<string, string> = {
  veo: 'Google Veo (Gemini Videos)',
}

function providerLabel(key: string) {
  return PROVIDER_LABELS[key] ?? key
}

function defaultModelForProvider(provider: string): string {
  const models = PROVIDER_MODELS[provider]
  return models?.[0]?.value ?? ''
}

function VideoSkeletonCard() {
  return (
    <div className="veo-studio-skeleton">
      <div className="veo-studio-skeleton-shine" />
      <div className="veo-studio-skeleton-icon" aria-hidden="true">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
          <polygon points="23 7 16 12 23 17 23 7" />
          <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
        </svg>
      </div>
    </div>
  )
}

export default function VeoStudioPanel({ view, onSendToChat, onToggleMobileSidebar }: VeoStudioPanelProps) {
  const [prompt, setPrompt] = useState('')
  const [negativePrompt, setNegativePrompt] = useState('')
  const [showNegative, setShowNegative] = useState(false)
  const [aspectRatio, setAspectRatio] = useState<AspectRatio>('16:9')
  const [duration, setDuration] = useState<Duration>(5)
  const [generating, setGenerating] = useState(false)
  const [error, setError] = useState('')
  const [result, setResult] = useState<VideoGenResponse | null>(null)
  const [promptHistory, setPromptHistory] = useState<string[]>([])
  const [historyVideos, setHistoryVideos] = useState<VideoGenEntry[]>([])

  const [providers, setProviders] = useState<VideoGenProviders>({ active: 'veo', available: ['veo'] })
  const [selectedProvider, setSelectedProvider] = useState('veo')
  const [selectedModel, setSelectedModel] = useState(defaultModelForProvider('veo'))

  const promptRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    let cancelled = false
    getVideoGenProviders().then((data) => {
      if (cancelled) return
      setProviders(data)
      setSelectedProvider(data.active)
      setSelectedModel(defaultModelForProvider(data.active))
    }).catch(() => { /* keep defaults */ })
    return () => { cancelled = true }
  }, [])

  const handleProviderChange = (provider: string) => {
    setSelectedProvider(provider)
    setSelectedModel(defaultModelForProvider(provider))
  }

  const handleGenerate = useCallback(async () => {
    const trimmed = prompt.trim()
    if (!trimmed || generating) return

    setGenerating(true)
    setError('')
    setResult(null)

    const request: VideoGenRequest = {
      prompt: trimmed,
      negativePrompt: negativePrompt.trim() || undefined,
      aspectRatio,
      duration,
      model: selectedModel || undefined,
      provider: selectedProvider,
    }

    try {
      const response = await generateVideo(request)
      setResult(response)
      setPromptHistory((prev) =>
        [trimmed, ...prev.filter((p) => p !== trimmed)].slice(0, 8),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setGenerating(false)
    }
  }, [prompt, negativePrompt, aspectRatio, duration, generating, selectedProvider, selectedModel])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault()
      handleGenerate()
    }
  }

  const downloadVideo = (entry: VideoGenEntry, index: number) => {
    const a = document.createElement('a')
    a.href = entry.url || entry.path
    a.download = `mint-veo-${index + 1}.mp4`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
  }

  const modelOptions = PROVIDER_MODELS[selectedProvider] ?? []

  return (
    <div
      className={`veo-studio ${view === 'veo' ? 'is-visible' : ''}`}
      id="veo-studio-panel"
      role="main"
      aria-label="Veo Studio"
      aria-hidden={view !== 'veo'}
    >
      {/* Header */}
      <header className="veo-studio-header">
        <div className="veo-studio-header-title">
          <button
            className="mobile-menu-btn"
            type="button"
            onClick={onToggleMobileSidebar}
            aria-label="Toggle menu"
            style={{ marginRight: '8px' }}
          >
            ☰
          </button>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polygon points="23 7 16 12 23 17 23 7" />
            <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
          </svg>
          <h1>Veo Studio</h1>
          <span className="veo-studio-badge">
            {providerLabel(selectedProvider)}
          </span>
        </div>
        <p className="veo-studio-subtitle">
          Generate videos with{' '}
          {providers.available.length > 1
            ? `${providers.available.length} configured providers`
            : providerLabel(selectedProvider)}
        </p>
      </header>

      {/* Main content */}
      <div className="veo-studio-content">
        {/* Left: controls */}
        <section className="veo-studio-controls" aria-label="Video generation settings">

          {/* Provider & Model side-by-side */}
          <div className="veo-studio-field" style={{ display: 'flex', gap: '10px', width: '100%' }}>
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '4px' }}>
              <label className="veo-studio-label" htmlFor="veo-studio-provider" style={{ display: 'flex', alignItems: 'center', width: '100%', height: '18px' }}>
                <span>Provider</span>
              </label>
              <select
                id="veo-studio-provider"
                className="veo-studio-textarea"
                style={{ padding: '8px 10px', height: '38px', cursor: 'pointer' }}
                value={selectedProvider}
                onChange={(e) => handleProviderChange(e.target.value)}
                disabled={generating}
              >
                {providers.available.map((prov) => (
                  <option key={prov} value={prov}>
                    {providerLabel(prov).replace(/^[^a-zA-Z0-9]+/, '')}
                  </option>
                ))}
              </select>
            </div>

            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '4px' }}>
              <label className="veo-studio-label" htmlFor="veo-studio-model" style={{ display: 'flex', alignItems: 'center', height: '18px' }}>
                Model
              </label>
              <select
                id="veo-studio-model"
                className="veo-studio-textarea"
                style={{ padding: '8px 10px', height: '38px', cursor: 'pointer' }}
                value={selectedModel}
                onChange={(e) => setSelectedModel(e.target.value)}
                disabled={generating}
              >
                {modelOptions.map(({ value, label }) => (
                  <option key={value} value={value}>{label}</option>
                ))}
              </select>
            </div>
          </div>

          {/* Prompt */}
          <div className="veo-studio-field">
            <label className="veo-studio-label" htmlFor="veo-studio-prompt">
              Prompt
              <span className="veo-studio-label-hint">Describe the video you want</span>
            </label>
            <textarea
              id="veo-studio-prompt"
              ref={promptRef}
              className="veo-studio-textarea"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="a golden eagle soaring over mountain peaks at sunset, cinematic 4K, slow motion..."
              rows={4}
              disabled={generating}
              aria-required="true"
            />
            <div className="veo-studio-prompt-hint">⌘+Enter to generate</div>
          </div>

          {/* Style suggestion chips */}
          <div className="veo-studio-field">
            <span className="veo-studio-label">Style suggestions</span>
            <div className="veo-studio-chips" role="group" aria-label="Style suggestions">
              {STYLE_SUGGESTIONS.map((style) => (
                <button
                  key={style}
                  type="button"
                  className="veo-studio-chip"
                  onClick={() => setPrompt((p) => p.trim() ? `${p.trim()}, ${style}` : style)}
                  disabled={generating}
                  title={`Add "${style}" to prompt`}
                >
                  {style}
                </button>
              ))}
            </div>
          </div>

          {/* Negative prompt (collapsible) */}
          <div className="veo-studio-field">
            <button
              type="button"
              className="veo-studio-toggle"
              onClick={() => setShowNegative((v) => !v)}
              aria-expanded={showNegative}
              aria-controls="veo-studio-negative-wrap"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"
                style={{ transform: showNegative ? 'rotate(90deg)' : 'none', transition: 'transform 0.2s' }}>
                <polyline points="9 18 15 12 9 6" />
              </svg>
              Negative prompt
              {negativePrompt.trim() && <span className="veo-studio-badge-dot" aria-label="has content" />}
            </button>
            {showNegative && (
              <div id="veo-studio-negative-wrap">
                <textarea
                  id="veo-studio-negative"
                  className="veo-studio-textarea veo-studio-textarea--sm"
                  value={negativePrompt}
                  onChange={(e) => setNegativePrompt(e.target.value)}
                  placeholder="blurry, shaky camera, low quality, watermark..."
                  rows={2}
                  disabled={generating}
                />
              </div>
            )}
          </div>

          {/* Aspect ratio */}
          <div className="veo-studio-field">
            <span className="veo-studio-label">Aspect ratio</span>
            <div className="veo-studio-aspect-group" role="radiogroup" aria-label="Aspect ratio">
              {ASPECT_OPTIONS.map(({ value, label, icon }) => (
                <button
                  key={value}
                  type="button"
                  role="radio"
                  aria-checked={aspectRatio === value}
                  className={`veo-studio-aspect-btn ${aspectRatio === value ? 'is-active' : ''}`}
                  onClick={() => setAspectRatio(value)}
                  disabled={generating}
                  id={`veo-studio-aspect-${value.replace(':', 'x')}`}
                >
                  <span className="veo-studio-aspect-icon" aria-hidden="true">{icon}</span>
                  <span>{label}</span>
                </button>
              ))}
            </div>
          </div>

          {/* Duration */}
          <div className="veo-studio-field">
            <span className="veo-studio-label">Duration</span>
            <div className="veo-studio-duration-group" role="radiogroup" aria-label="Video duration">
              {DURATION_OPTIONS.map(({ value, label }) => (
                <button
                  key={value}
                  type="button"
                  role="radio"
                  aria-checked={duration === value}
                  className={`veo-studio-duration-btn ${duration === value ? 'is-active' : ''}`}
                  onClick={() => setDuration(value)}
                  disabled={generating}
                  id={`veo-studio-duration-${value}`}
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" />
                  </svg>
                  {label}
                </button>
              ))}
            </div>
          </div>

          {/* Generate button */}
          <button
            id="veo-studio-generate-btn"
            type="button"
            className={`veo-studio-generate-btn ${generating ? 'is-loading' : ''}`}
            onClick={handleGenerate}
            disabled={!prompt.trim() || generating}
            aria-busy={generating}
          >
            {generating ? (
              <>
                <span className="veo-studio-spinner" aria-hidden="true" />
                Generating video...
              </>
            ) : (
              <>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <polygon points="23 7 16 12 23 17 23 7" />
                  <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
                </svg>
                Generate Video
              </>
            )}
          </button>

          {/* Error */}
          {error && (
            <div className="veo-studio-error" role="alert">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
              <span>{error}</span>
              <button type="button" onClick={() => setError('')} aria-label="Dismiss error">×</button>
            </div>
          )}

          {/* Prompt history */}
          {promptHistory.length > 0 && (
            <div className="veo-studio-field">
              <span className="veo-studio-label">Recent prompts</span>
              <div className="veo-studio-history">
                {promptHistory.map((p) => (
                  <button
                    key={p}
                    type="button"
                    className="veo-studio-history-item"
                    onClick={() => { setPrompt(p); promptRef.current?.focus() }}
                    disabled={generating}
                    title={p}
                  >
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" />
                    </svg>
                    <span>{p}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </section>

        {/* Right: results */}
        <section className="veo-studio-results" aria-label="Generated videos" aria-live="polite">
          {/* Active Generation Session workspace */}
          <div className="veo-studio-active-workspace">
            {generating && (
              <div className="veo-studio-skeleton-wrap">
                <VideoSkeletonCard />
                <p className="veo-studio-generating-label" aria-live="assertive">
                  Generating {duration}s video with {providerLabel(selectedProvider)} — this may take a moment…
                </p>
              </div>
            )}

            {!generating && result && (
              <>
                {result.description && (
                  <p className="veo-studio-description">{result.description}</p>
                )}
                <div className="veo-studio-video-grid">
                  {result.videos.map((entry, idx) => (
                    <article key={entry.id} className="veo-studio-card" aria-label={`Generated video ${idx + 1}`}>
                      <div className="veo-studio-card-video-wrap">
                        <video
                          src={convertFileSrc(entry.path || entry.url)}
                          className="veo-studio-card-video"
                          controls
                          preload="metadata"
                          aria-label={entry.message || prompt}
                        />
                      </div>
                      <div className="veo-studio-card-actions">
                        <button
                          type="button"
                          className="veo-studio-action-btn"
                          onClick={() => downloadVideo(entry, idx)}
                          title="Download video"
                          id={`veo-studio-download-${idx}`}
                        >
                          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" />
                          </svg>
                          Download
                        </button>
                        {onSendToChat && (
                          <button
                            type="button"
                            className="veo-studio-action-btn veo-studio-action-btn--primary"
                            onClick={() => onSendToChat(prompt)}
                            title="Send prompt to Chat"
                            id={`veo-studio-send-${idx}`}
                          >
                            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                              <line x1="22" y1="2" x2="11" y2="13" /><polygon points="22 2 15 22 11 13 2 9 22 2" />
                            </svg>
                            Send to Chat
                          </button>
                        )}
                      </div>
                    </article>
                  ))}
                </div>
                <p className="veo-studio-meta">
                  Provider: <strong>{providerLabel(result.provider)}</strong> · Model: <strong>{result.model}</strong> · {result.videos.length} video{result.videos.length > 1 ? 's' : ''}
                </p>
              </>
            )}

            {!generating && !result && (
              <div className="veo-studio-active-empty">
                <div className="veo-studio-empty-icon" aria-hidden="true">
                  <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
                    <polygon points="23 7 16 12 23 17 23 7" />
                    <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
                  </svg>
                </div>
                <p>No video generated yet</p>
                <span>Enter a prompt on the left and click Generate Video to begin.</span>
              </div>
            )}
          </div>

          {/* History divider */}
          <div className="veo-studio-history-header">
            <div className="veo-studio-history-title">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polygon points="23 7 16 12 23 17 23 7" />
                <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
              </svg>
              <span>Video History</span>
              <span className="veo-studio-history-badge">{historyVideos.length}</span>
            </div>
          </div>

          {/* History gallery */}
          <div className="veo-studio-history-gallery">
            {historyVideos.length === 0 ? (
              <div className="veo-studio-gallery-empty">
                <p>Your video gallery is empty</p>
                <span>Generated videos will appear here once the backend is connected.</span>
              </div>
            ) : (
              <div className="veo-studio-video-grid">
                {historyVideos.map((video, idx) => (
                  <article key={video.id} className="veo-studio-card" aria-label={`Saved video ${idx + 1}`}>
                    <div className="veo-studio-card-video-wrap">
                      <video
                        src={convertFileSrc(video.path || video.url)}
                        className="veo-studio-card-video"
                        controls
                        preload="metadata"
                      />
                    </div>
                    <div className="veo-studio-card-actions">
                      <button
                        type="button"
                        className="veo-studio-action-btn"
                        onClick={() => downloadVideo(video, idx)}
                        title="Download video"
                        id={`veo-studio-gallery-download-${idx}`}
                      >
                        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" />
                        </svg>
                        Download
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  )
}
