import React, { useState, useRef, useCallback } from 'react'
import {
  generateImages,
  convertFileSrc,
  type ImageGenRequest,
  type ImageGenResponse,
  type PictureEntry,
} from '../tauri'

type AspectRatio = '1:1' | '16:9' | '9:16' | '4:3'

interface ImageStudioPanelProps {
  view: string
  onRefreshPictures?: () => void
  onSendToChat?: (dataUri: string, prompt: string) => void
}

const ASPECT_OPTIONS: { value: AspectRatio; label: string; icon: string }[] = [
  { value: '1:1',  label: '1:1',  icon: '⬛' },
  { value: '16:9', label: '16:9', icon: '▬' },
  { value: '9:16', label: '9:16', icon: '▮' },
  { value: '4:3',  label: '4:3',  icon: '⬜' },
]

const STYLE_SUGGESTIONS = [
  'photorealistic', 'anime', 'oil painting', 'watercolor',
  'cinematic', 'digital art', '3D render', 'sketch',
]

function SkeletonCard() {
  return (
    <div className="img-studio-skeleton">
      <div className="img-studio-skeleton-shine" />
    </div>
  )
}

export default function ImageStudioPanel({ view, onRefreshPictures, onSendToChat }: ImageStudioPanelProps) {
  const [prompt, setPrompt] = useState('')
  const [negativePrompt, setNegativePrompt] = useState('')
  const [showNegative, setShowNegative] = useState(false)
  const [selectedModel, setSelectedModel] = useState('gemini-2.5-flash-image')
  const [customModel, setCustomModel] = useState('')
  const [aspectRatio, setAspectRatio] = useState<AspectRatio>('1:1')
  const [numImages, setNumImages] = useState(1)
  const [generating, setGenerating] = useState(false)
  const [error, setError] = useState('')
  const [result, setResult] = useState<ImageGenResponse | null>(null)
  const [promptHistory, setPromptHistory] = useState<string[]>([])
  const promptRef = useRef<HTMLTextAreaElement>(null)

  const handleGenerate = useCallback(async () => {
    const trimmed = prompt.trim()
    if (!trimmed || generating) return

    setGenerating(true)
    setError('')
    setResult(null)

    const request: ImageGenRequest = {
      prompt: trimmed,
      negativePrompt: negativePrompt.trim() || undefined,
      aspectRatio,
      numImages,
      model: selectedModel === 'custom' ? customModel.trim() : selectedModel,
    }

    try {
      const response = await generateImages(request)
      setResult(response)
      setPromptHistory((prev) =>
        [trimmed, ...prev.filter((p) => p !== trimmed)].slice(0, 8),
      )
      onRefreshPictures?.()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setGenerating(false)
    }
  }, [prompt, negativePrompt, aspectRatio, numImages, generating, onRefreshPictures, selectedModel, customModel])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault()
      handleGenerate()
    }
  }

  const downloadImage = (entry: PictureEntry, index: number) => {
    const a = document.createElement('a')
    a.href = entry.url || entry.path
    a.download = `mint-imagine-${index + 1}.png`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
  }

  if (view !== 'imagine') return null

  return (
    <div className="img-studio" id="image-studio-panel" role="main" aria-label="Image Studio">
      {/* Header */}
      <header className="img-studio-header">
        <div className="img-studio-header-title">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
          </svg>
          <h1>Image Studio</h1>
          <span className="img-studio-badge">
            {selectedModel === 'custom' ? (customModel.trim() || 'custom') : selectedModel}
          </span>
        </div>
        <p className="img-studio-subtitle">Generate images with Google's Gemini image model</p>
      </header>

      {/* Main content */}
      <div className="img-studio-content">
        {/* Left: controls */}
        <section className="img-studio-controls" aria-label="Generation settings">
          {/* Model selection */}
          <div className="img-studio-field">
            <label className="img-studio-label" htmlFor="img-studio-model">
              Model
              <span className="img-studio-label-hint">Select the generation model</span>
            </label>
            <select
              id="img-studio-model"
              className="img-studio-textarea"
              style={{ padding: '8px 10px', height: '38px', cursor: 'pointer' }}
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value)}
              disabled={generating}
            >
              <option value="gemini-2.5-flash-image">Gemini 2.5 Flash Image (Default)</option>
              <option value="gemini-3.1-flash-image">Gemini 3.1 Flash Image</option>
              <option value="gemini-3-pro-image">Gemini 3 Pro Image</option>
              <option value="imagen-3.0-generate-002">Imagen 3</option>
              <option value="custom">Custom Model ID...</option>
            </select>
            {selectedModel === 'custom' && (
              <input
                type="text"
                className="img-studio-textarea img-studio-textarea--sm"
                style={{ marginTop: '5px' }}
                placeholder="e.g. gemini-3.1-flash-image"
                value={customModel}
                onChange={(e) => setCustomModel(e.target.value)}
                disabled={generating}
              />
            )}
          </div>

          {/* Prompt */}
          <div className="img-studio-field">
            <label className="img-studio-label" htmlFor="img-studio-prompt">
              Prompt
              <span className="img-studio-label-hint">Describe the image you want</span>
            </label>
            <textarea
              id="img-studio-prompt"
              ref={promptRef}
              className="img-studio-textarea"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="a dragon flying over Bangkok at sunset, golden hour, ultra-detailed..."
              rows={4}
              disabled={generating}
              aria-required="true"
            />
            <div className="img-studio-prompt-hint">⌘+Enter to generate</div>
          </div>

          {/* Style suggestion chips */}
          <div className="img-studio-field">
            <span className="img-studio-label">Style suggestions</span>
            <div className="img-studio-chips" role="group" aria-label="Style suggestions">
              {STYLE_SUGGESTIONS.map((style) => (
                <button
                  key={style}
                  type="button"
                  className="img-studio-chip"
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
          <div className="img-studio-field">
            <button
              type="button"
              className="img-studio-toggle"
              onClick={() => setShowNegative((v) => !v)}
              aria-expanded={showNegative}
              aria-controls="img-studio-negative-wrap"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"
                style={{ transform: showNegative ? 'rotate(90deg)' : 'none', transition: 'transform 0.2s' }}>
                <polyline points="9 18 15 12 9 6"/>
              </svg>
              Negative prompt
              {negativePrompt.trim() && <span className="img-studio-badge-dot" aria-label="has content" />}
            </button>
            {showNegative && (
              <div id="img-studio-negative-wrap">
                <textarea
                  id="img-studio-negative"
                  className="img-studio-textarea img-studio-textarea--sm"
                  value={negativePrompt}
                  onChange={(e) => setNegativePrompt(e.target.value)}
                  placeholder="blurry, low quality, distorted, text, watermark..."
                  rows={2}
                  disabled={generating}
                />
              </div>
            )}
          </div>

          {/* Aspect ratio */}
          <div className="img-studio-field">
            <span className="img-studio-label">Aspect ratio</span>
            <div className="img-studio-aspect-group" role="radiogroup" aria-label="Aspect ratio">
              {ASPECT_OPTIONS.map(({ value, label, icon }) => (
                <button
                  key={value}
                  type="button"
                  role="radio"
                  aria-checked={aspectRatio === value}
                  className={`img-studio-aspect-btn ${aspectRatio === value ? 'is-active' : ''}`}
                  onClick={() => setAspectRatio(value)}
                  disabled={generating}
                  id={`img-studio-aspect-${value.replace(':', 'x')}`}
                >
                  <span className="img-studio-aspect-icon" aria-hidden="true">{icon}</span>
                  <span>{label}</span>
                </button>
              ))}
            </div>
          </div>

          {/* Count */}
          <div className="img-studio-field">
            <span className="img-studio-label">Number of images</span>
            <div className="img-studio-count-group" role="radiogroup" aria-label="Image count">
              {[1, 2, 3, 4].map((n) => (
                <button
                  key={n}
                  type="button"
                  role="radio"
                  aria-checked={numImages === n}
                  className={`img-studio-count-btn ${numImages === n ? 'is-active' : ''}`}
                  onClick={() => setNumImages(n)}
                  disabled={generating}
                  id={`img-studio-count-${n}`}
                >
                  {n}
                </button>
              ))}
            </div>
          </div>

          {/* Generate button */}
          <button
            id="img-studio-generate-btn"
            type="button"
            className={`img-studio-generate-btn ${generating ? 'is-loading' : ''}`}
            onClick={handleGenerate}
            disabled={!prompt.trim() || generating}
            aria-busy={generating}
          >
            {generating ? (
              <>
                <span className="img-studio-spinner" aria-hidden="true" />
                Generating...
              </>
            ) : (
              <>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
                Generate Image
              </>
            )}
          </button>

          {/* Error */}
          {error && (
            <div className="img-studio-error" role="alert">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
              </svg>
              <span>{error}</span>
              <button type="button" onClick={() => setError('')} aria-label="Dismiss error">×</button>
            </div>
          )}

          {/* Prompt history */}
          {promptHistory.length > 0 && (
            <div className="img-studio-field">
              <span className="img-studio-label">Recent prompts</span>
              <div className="img-studio-history">
                {promptHistory.map((p) => (
                  <button
                    key={p}
                    type="button"
                    className="img-studio-history-item"
                    onClick={() => { setPrompt(p); promptRef.current?.focus() }}
                    disabled={generating}
                    title={p}
                  >
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
                    </svg>
                    <span>{p}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </section>

        {/* Right: results */}
        <section className="img-studio-results" aria-label="Generated images" aria-live="polite">
          {generating && (
            <div className="img-studio-skeleton-grid">
              {Array.from({ length: numImages }).map((_, i) => (
                <SkeletonCard key={i} />
              ))}
              <p className="img-studio-generating-label" aria-live="assertive">
                Creating {numImages} image{numImages > 1 ? 's' : ''} — this may take a few seconds…
              </p>
            </div>
          )}

          {!generating && result && (
            <>
              {result.description && (
                <p className="img-studio-description">{result.description}</p>
              )}
              <div
                className="img-studio-grid"
                style={{ '--img-count': result.images.length } as React.CSSProperties}
              >
                {result.images.map((entry, idx) => (
                  <article key={entry.id} className="img-studio-card" aria-label={`Generated image ${idx + 1}`}>
                    <div className="img-studio-card-img-wrap">
                      <img
                        src={convertFileSrc(entry.url || entry.path)}
                        alt={entry.message || prompt}
                        className="img-studio-card-img"
                        loading="eager"
                        decoding="async"
                      />
                    </div>
                    <div className="img-studio-card-actions">
                      <button
                        type="button"
                        className="img-studio-action-btn"
                        onClick={() => downloadImage(entry, idx)}
                        title="Download image"
                        id={`img-studio-download-${idx}`}
                      >
                        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
                        </svg>
                        Download
                      </button>
                      {onSendToChat && (
                        <button
                          type="button"
                          className="img-studio-action-btn img-studio-action-btn--primary"
                          onClick={() => onSendToChat(entry.url || entry.path, prompt)}
                          title="Send to Chat"
                          id={`img-studio-send-${idx}`}
                        >
                          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                            <line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>
                          </svg>
                          Send to Chat
                        </button>
                      )}
                    </div>
                  </article>
                ))}
              </div>
              <p className="img-studio-meta">
                Model: <strong>{result.model}</strong> · {result.images.length} image{result.images.length > 1 ? 's' : ''} · Saved to gallery
              </p>
            </>
          )}

          {!generating && !result && (
            <div className="img-studio-empty">
              <div className="img-studio-empty-icon" aria-hidden="true">
                <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
              </div>
              <p>Your generated images will appear here</p>
              <span>Type a prompt and click Generate Image to start</span>
            </div>
          )}
        </section>
      </div>
    </div>
  )
}
