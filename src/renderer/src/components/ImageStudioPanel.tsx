import React, { useState, useRef, useCallback, useEffect } from 'react'
import {
  generateImages,
  getImageGenProviders,
  convertFileSrc,
  listSavedPictures,
  type ImageGenRequest,
  type ImageGenResponse,
  type ImageGenProviders,
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

// Per-provider model presets shown in the model dropdown
const PROVIDER_MODELS: Record<string, { value: string; label: string }[]> = {
  nanobanana: [
    { value: 'gemini-2.5-flash-image', label: 'Gemini 2.5 Flash Image (Default)' },
    { value: 'gemini-2.0-flash-image', label: 'Gemini 2.0 Flash Image' },
  ],
  dalle: [
    { value: 'dall-e-3',    label: 'DALL·E 3 (Default)' },
    { value: 'gpt-image-1', label: 'GPT-Image-1' },
    { value: 'dall-e-2',    label: 'DALL·E 2' },
  ],
  stability: [
    { value: 'sd3.5-large',       label: 'SD 3.5 Large (Default)' },
    { value: 'sd3.5-large-turbo', label: 'SD 3.5 Large Turbo' },
    { value: 'sd3-medium',        label: 'SD 3 Medium' },
    { value: 'core',              label: 'Stable Image Core' },
  ],
  ideogram: [
    { value: 'V_3',       label: 'Ideogram V3 (Default)' },
    { value: 'V_2',       label: 'Ideogram V2' },
    { value: 'V_2_TURBO', label: 'Ideogram V2 Turbo' },
  ],
  replicate: [
    { value: 'black-forest-labs/flux-1.1-pro',          label: 'FLUX 1.1 Pro (Default)' },
    { value: 'black-forest-labs/flux-schnell',          label: 'FLUX Schnell (fast)' },
    { value: 'stability-ai/sdxl',                       label: 'SDXL' },
    { value: 'bytedance/sdxl-lightning-4step',          label: 'SDXL Lightning' },
  ],
}

const PROVIDER_LABELS: Record<string, string> = {
  nanobanana: '✦ NanoBanana (Gemini)',
  dalle:      '⬡ DALL·E (OpenAI)',
  stability:  '◈ Stability AI',
  ideogram:   '◉ Ideogram',
  replicate:  '⬢ Replicate',
}

function providerLabel(key: string) {
  return PROVIDER_LABELS[key] ?? key
}

function defaultModelForProvider(provider: string): string {
  const models = PROVIDER_MODELS[provider]
  return models?.[0]?.value ?? ''
}

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
  const [aspectRatio, setAspectRatio] = useState<AspectRatio>('1:1')
  const [numImages, setNumImages] = useState(1)
  const [generating, setGenerating] = useState(false)
  const [error, setError] = useState('')
  const [result, setResult] = useState<ImageGenResponse | null>(null)
  const [promptHistory, setPromptHistory] = useState<string[]>([])
  const [historyPictures, setHistoryPictures] = useState<PictureEntry[]>([])

  const loadHistory = useCallback(async () => {
    try {
      const pics = await listSavedPictures()
      const imageGenSources = ['nanobanana', 'dalle', 'stability', 'ideogram', 'replicate']
      const filtered = pics.filter((pic) =>
        imageGenSources.includes(pic.source?.toLowerCase()),
      )
      const sorted = [...filtered].sort((a, b) => {
        const da = a.createdAt ? new Date(a.createdAt).getTime() : 0
        const db = b.createdAt ? new Date(b.createdAt).getTime() : 0
        return db - da || b.filename.localeCompare(a.filename)
      })
      setHistoryPictures(sorted)
    } catch (err) {
      console.error('Failed to load history pictures:', err)
    }
  }, [])

  // Provider / model state
  const [providers, setProviders] = useState<ImageGenProviders>({ active: 'nanobanana', available: ['nanobanana'] })
  const [selectedProvider, setSelectedProvider] = useState('nanobanana')
  const [selectedModel, setSelectedModel] = useState(defaultModelForProvider('nanobanana'))
  const [customModel, setCustomModel] = useState('')

  const promptRef = useRef<HTMLTextAreaElement>(null)

  // Load available providers and history on mount
  useEffect(() => {
    let cancelled = false
    getImageGenProviders().then((data) => {
      if (cancelled) return
      setProviders(data)
      setSelectedProvider(data.active)
      setSelectedModel(defaultModelForProvider(data.active))
    }).catch(() => { /* keep defaults */ })
    loadHistory()
    return () => { cancelled = true }
  }, [loadHistory])

  // When provider changes, reset model to provider's default
  const handleProviderChange = (provider: string) => {
    setSelectedProvider(provider)
    setSelectedModel(defaultModelForProvider(provider))
    setCustomModel('')
  }

  const effectiveModel = selectedModel === 'custom' ? customModel.trim() : selectedModel

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
      model: effectiveModel || undefined,
      provider: selectedProvider,
    }

    try {
      const response = await generateImages(request)
      setResult(response)
      setPromptHistory((prev) =>
        [trimmed, ...prev.filter((p) => p !== trimmed)].slice(0, 8),
      )
      onRefreshPictures?.()
      loadHistory()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setGenerating(false)
    }
  }, [prompt, negativePrompt, aspectRatio, numImages, generating, onRefreshPictures, selectedProvider, effectiveModel])

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

  const modelOptions = PROVIDER_MODELS[selectedProvider] ?? []

  return (
    <div
      className={`img-studio ${view === 'imagine' ? 'is-visible' : ''}`}
      id="image-studio-panel"
      role="main"
      aria-label="Image Studio"
      aria-hidden={view !== 'imagine'}
    >
      {/* Header */}
      <header className="img-studio-header">
        <div className="img-studio-header-title">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
          </svg>
          <h1>Image Studio</h1>
          <span className="img-studio-badge">
            {providerLabel(selectedProvider)}
          </span>
        </div>
        <p className="img-studio-subtitle">
          Generate images with{' '}
          {providers.available.length > 1
            ? `${providers.available.length} configured providers`
            : providerLabel(selectedProvider)}
        </p>
      </header>

      {/* Main content */}
      <div className="img-studio-content">
        {/* Left: controls */}
        <section className="img-studio-controls" aria-label="Generation settings">

          {/* Provider & Model side-by-side dropdown selectors */}
          <div className="img-studio-field" style={{ display: 'flex', gap: '10px', width: '100%' }}>
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '4px' }}>
              <label className="img-studio-label" htmlFor="img-studio-provider" style={{ display: 'flex', alignItems: 'center', width: '100%', height: '18px' }}>
                <span>Provider</span>
              </label>
              <select
                id="img-studio-provider"
                className="img-studio-textarea"
                style={{ padding: '8px 10px', height: '38px', cursor: 'pointer' }}
                value={selectedProvider}
                onChange={(e) => handleProviderChange(e.target.value)}
                disabled={generating}
              >
                {providers.available.map((prov) => (
                  <option key={prov} value={prov}>
                    {providerLabel(prov).replace(/^[^a-zA-Z0-9]+/, '') /* remove prefix symbol if preferred or keep it */}
                  </option>
                ))}
              </select>
            </div>

            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '4px' }}>
              <label className="img-studio-label" htmlFor="img-studio-model" style={{ display: 'flex', alignItems: 'center', height: '18px' }}>
                Model
              </label>
              <select
                id="img-studio-model"
                className="img-studio-textarea"
                style={{ padding: '8px 10px', height: '38px', cursor: 'pointer' }}
                value={selectedModel}
                onChange={(e) => setSelectedModel(e.target.value)}
                disabled={generating}
              >
                {modelOptions.map(({ value, label }) => (
                  <option key={value} value={value}>{label}</option>
                ))}
                <option value="custom">Custom Model ID...</option>
              </select>
            </div>
          </div>

          {/* Custom Model ID input */}
          {selectedModel === 'custom' && (
            <div className="img-studio-field" style={{ marginTop: '-8px' }}>
              <input
                type="text"
                className="img-studio-textarea img-studio-textarea--sm"
                placeholder={
                  selectedProvider === 'replicate'
                    ? 'e.g. owner/model-name'
                    : selectedProvider === 'dalle'
                    ? 'e.g. dall-e-3'
                    : 'e.g. sd3.5-large'
                }
                value={customModel}
                onChange={(e) => setCustomModel(e.target.value)}
                disabled={generating}
              />
            </div>
          )}

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

          {/* Count — DALL·E 3 only supports n=1 */}
          <div className="img-studio-field">
            <span className="img-studio-label">
              Number of images
              {(selectedProvider === 'dalle' && (effectiveModel === 'dall-e-3' || effectiveModel === 'gpt-image-1')) && (
                <span className="img-studio-label-hint"> (DALL·E 3 / GPT-Image-1: max 1)</span>
              )}
            </span>
            <div className="img-studio-count-group" role="radiogroup" aria-label="Image count">
              {[1, 2, 3, 4].map((n) => {
                const isDisabled = generating ||
                  (selectedProvider === 'dalle' && (effectiveModel === 'dall-e-3' || effectiveModel === 'gpt-image-1') && n > 1)
                return (
                  <button
                    key={n}
                    type="button"
                    role="radio"
                    aria-checked={numImages === n}
                    className={`img-studio-count-btn ${numImages === n ? 'is-active' : ''}`}
                    onClick={() => setNumImages(n)}
                    disabled={isDisabled}
                    id={`img-studio-count-${n}`}
                  >
                    {n}
                  </button>
                )
              })}
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
          {/* Active Generation Session workspace */}
          <div className="img-studio-active-workspace">
            {generating && (
              <div className="img-studio-skeleton-grid">
                {Array.from({ length: numImages }).map((_, i) => (
                  <SkeletonCard key={i} />
                ))}
                <p className="img-studio-generating-label" aria-live="assertive">
                  Creating {numImages} image{numImages > 1 ? 's' : ''} with {providerLabel(selectedProvider)} — this may take a few seconds…
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
                          src={convertFileSrc(entry.path || entry.url)}
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
                  Provider: <strong>{providerLabel(result.provider)}</strong> · Model: <strong>{result.model}</strong> · {result.images.length} image{result.images.length > 1 ? 's' : ''} · Saved to gallery
                </p>
              </>
            )}

            {!generating && !result && (
              <div className="img-studio-active-empty">
                <div className="img-studio-empty-icon" aria-hidden="true">
                  <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                  </svg>
                </div>
                <p>No active generation</p>
                <span>Enter a prompt on the left pane and generate new images.</span>
              </div>
            )}
          </div>

          {/* 2. Middle divider / tab header */}
          <div className="img-studio-history-header">
            <div className="img-studio-history-title">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/>
              </svg>
              <span>Gallery & History</span>
              <span className="img-studio-history-badge">
                {(() => {
                  const currentIds = new Set(result?.images.map(img => img.id) || [])
                  return historyPictures.filter(img => !currentIds.has(img.id)).length
                })()}
              </span>
            </div>
          </div>

          {/* 3. Previously Generated Gallery list */}
          <div className="img-studio-history-gallery">
            {(() => {
              const currentIds = new Set(result?.images.map(img => img.id) || [])
              const filteredHistory = historyPictures.filter(img => !currentIds.has(img.id))
              
              if (filteredHistory.length === 0) {
                return (
                  <div className="img-studio-gallery-empty">
                    <p>Your saved gallery is empty</p>
                    <span>Generated images will be stored here automatically.</span>
                  </div>
                )
              }
              
              return (
                <div className="img-studio-grid">
                  {filteredHistory.map((picture, idx) => (
                    <article key={picture.id} className="img-studio-card" aria-label={`Saved image ${idx + 1}`}>
                      <div className="img-studio-card-img-wrap">
                        <img
                          src={convertFileSrc(picture.thumbnailPath || picture.thumbnailUrl || picture.path || picture.url)}
                          alt={picture.message || 'Generated image'}
                          className="img-studio-card-img"
                          loading="lazy"
                          decoding="async"
                        />
                      </div>
                      <div className="img-studio-card-actions">
                        <button
                          type="button"
                          className="img-studio-action-btn"
                          onClick={() => downloadImage(picture, idx)}
                          title="Download image"
                          id={`img-studio-gallery-download-${idx}`}
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
                            onClick={() => onSendToChat(picture.url || picture.path, picture.message || '')}
                            title="Send to Chat"
                            id={`img-studio-gallery-send-${idx}`}
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
              )
            })()}
          </div>
        </section>
      </div>
    </div>
  )
}
