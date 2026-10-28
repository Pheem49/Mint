import React, { useState, useRef, useMemo } from 'react'

// Official AI SVGs from @lobehub/icons-static-svg
import anthropicSvg from '@lobehub/icons-static-svg/icons/anthropic.svg?raw'
import deepseekSvg from '@lobehub/icons-static-svg/icons/deepseek-color.svg?raw'
import geminiSvg from '@lobehub/icons-static-svg/icons/gemini-color.svg?raw'
import groqSvg from '@lobehub/icons-static-svg/icons/groq.svg?raw'
import huggingFaceSvg from '@lobehub/icons-static-svg/icons/huggingface-color.svg?raw'
import lmStudioSvg from '@lobehub/icons-static-svg/icons/lmstudio.svg?raw'
import mistralSvg from '@lobehub/icons-static-svg/icons/mistral-color.svg?raw'
import ollamaSvg from '@lobehub/icons-static-svg/icons/ollama.svg?raw'
import openAiSvg from '@lobehub/icons-static-svg/icons/openai.svg?raw'
import openRouterSvg from '@lobehub/icons-static-svg/icons/openrouter-color.svg?raw'
import perplexitySvg from '@lobehub/icons-static-svg/icons/perplexity-color.svg?raw'
import qwenSvg from '@lobehub/icons-static-svg/icons/qwen-color.svg?raw'
import togetherSvg from '@lobehub/icons-static-svg/icons/together-color.svg?raw'

export function svgStringToDataUrl(svg: string): string {
  let processed = svg.trim()
  if (!processed.includes('xmlns=')) {
    processed = processed.replace('<svg', '<svg xmlns="http://www.w3.org/2000/svg"')
  }
  return `data:image/svg+xml;utf8,${encodeURIComponent(processed)}`
}

export function emojiToDataUrl(emoji: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40"><text x="50%" y="54%" dominant-baseline="middle" text-anchor="middle" font-size="28">${emoji}</text></svg>`
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
}

export interface PresetItem {
  id: string
  name: string
  dataUrl: string
}

const PRESET_LOGOS: PresetItem[] = [
  { id: 'deepseek', name: 'DeepSeek', dataUrl: svgStringToDataUrl(deepseekSvg) },
  { id: 'ollama', name: 'Ollama', dataUrl: svgStringToDataUrl(ollamaSvg) },
  { id: 'openrouter', name: 'OpenRouter', dataUrl: svgStringToDataUrl(openRouterSvg) },
  { id: 'groq', name: 'Groq', dataUrl: svgStringToDataUrl(groqSvg) },
  { id: 'mistral', name: 'Mistral AI', dataUrl: svgStringToDataUrl(mistralSvg) },
  { id: 'together', name: 'Together AI', dataUrl: svgStringToDataUrl(togetherSvg) },
  { id: 'perplexity', name: 'Perplexity', dataUrl: svgStringToDataUrl(perplexitySvg) },
  { id: 'lmstudio', name: 'LM Studio', dataUrl: svgStringToDataUrl(lmStudioSvg) },
  { id: 'huggingface', name: 'Hugging Face', dataUrl: svgStringToDataUrl(huggingFaceSvg) },
  { id: 'openai', name: 'OpenAI', dataUrl: svgStringToDataUrl(openAiSvg) },
  { id: 'anthropic', name: 'Anthropic', dataUrl: svgStringToDataUrl(anthropicSvg) },
  { id: 'gemini', name: 'Gemini', dataUrl: svgStringToDataUrl(geminiSvg) },
  { id: 'qwen', name: 'Qwen', dataUrl: svgStringToDataUrl(qwenSvg) },
]

const POPULAR_EMOJIS = ['🤖', '🧠', '⚡', '🐋', '🦙', '🚀', '🔮', '🌐', '💡', '💎', '🛠️', '🦉', '✨', '🔥', '🎯', '🦄']

export interface CustomProviderLogoPickerProps {
  value?: string
  onChange: (logoDataUrl?: string) => void
  providerName?: string
  onRequestCrop?: (fileDataUrl: string, fileName: string) => void
  defaultExpanded?: boolean
}

export default function CustomProviderLogoPicker({
  value,
  onChange,
  providerName,
  onRequestCrop,
  defaultExpanded = false,
}: CustomProviderLogoPickerProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded)
  const [activeTab, setActiveTab] = useState<'presets' | 'upload' | 'url_emoji'>('presets')
  const [urlInput, setUrlInput] = useState('')
  const [customEmojiInput, setCustomEmojiInput] = useState('')
  const [isDragging, setIsDragging] = useState(false)
  const [uploadError, setUploadError] = useState<string | null>(null)

  const fileInputRef = useRef<HTMLInputElement>(null)

  // Identify if current value matches one of our presets
  const activePresetId = useMemo(() => {
    if (!value) return null
    const found = PRESET_LOGOS.find(p => p.dataUrl === value)
    return found ? found.id : null
  }, [value])

  const handleFileProcess = async (file: File) => {
    setUploadError(null)
    const isSvg = file.type === 'image/svg+xml' || file.name.toLowerCase().endsWith('.svg')
    const isRaster = ['image/png', 'image/jpeg', 'image/webp'].includes(file.type) ||
      /\.(png|jpe?g|webp)$/i.test(file.name)

    if (!isSvg && !isRaster) {
      setUploadError('Please choose a PNG, JPEG, WebP, or SVG file.')
      return
    }

    if (isSvg) {
      const reader = new FileReader()
      reader.onload = () => {
        if (typeof reader.result === 'string') {
          onChange(svgStringToDataUrl(reader.result))
        }
      }
      reader.onerror = () => setUploadError('Could not read this SVG file.')
      reader.readAsText(file)
      return
    }

    // Raster image: read as data URL
    const reader = new FileReader()
    reader.onload = () => {
      const dataUri = reader.result as string
      if (onRequestCrop) {
        onRequestCrop(dataUri, file.name)
      } else {
        onChange(dataUri)
      }
    }
    reader.onerror = () => setUploadError('Could not read image file.')
    reader.readAsDataURL(file)
  }

  const handleApplyUrl = () => {
    const trimmed = urlInput.trim()
    if (!trimmed) return
    if (!/^https?:\/\//i.test(trimmed) && !trimmed.startsWith('data:image/')) {
      setUploadError('URL must start with http://, https://, or data:image/')
      return
    }
    setUploadError(null)
    onChange(trimmed)
    setUrlInput('')
  }

  const handleApplyEmoji = (emoji: string) => {
    setUploadError(null)
    onChange(emojiToDataUrl(emoji))
  }

  return (
    <div className={`custom-provider-logo-card ${isExpanded ? 'is-expanded' : 'is-collapsed'}`}>
      {/* Top Bar: Live Preview & Status */}
      <div
        className="custom-provider-logo-header"
        onClick={() => setIsExpanded(!isExpanded)}
        role="button"
        tabIndex={0}
        aria-expanded={isExpanded}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            setIsExpanded(!isExpanded)
          }
        }}
      >
        <div className="custom-provider-logo-avatar-wrap">
          <div className="custom-provider-logo-avatar" aria-label="Logo preview">
            {value ? (
              <img src={value} alt="" />
            ) : (
              <span className="custom-provider-logo-fallback">
                {providerName ? providerName.slice(0, 2).toUpperCase() : 'AI'}
              </span>
            )}
          </div>
          <div className="custom-provider-logo-meta">
            <span className="custom-provider-logo-title">
              {value ? 'Custom logo active' : 'No logo selected'}
            </span>
            <span className="custom-provider-logo-subtitle">
              {value
                ? activePresetId
                  ? `Preset: ${PRESET_LOGOS.find(p => p.id === activePresetId)?.name}`
                  : 'Custom image / icon'
                : 'Using fallback default mark'}
            </span>
          </div>
        </div>

        <div className="custom-provider-logo-top-actions" onClick={(e) => e.stopPropagation()}>
          {value && onRequestCrop && !activePresetId && !value.includes('data:image/svg+xml') && (
            <button
              type="button"
              className="btn btn-secondary btn-xs"
              onClick={() => onRequestCrop(value, 'provider-logo.png')}
              title="Crop & rotate current logo"
            >
              Crop / Rotate
            </button>
          )}

          {value && (
            <button
              type="button"
              className="btn btn-secondary btn-xs btn-danger-hover"
              onClick={() => onChange(undefined)}
              title="Remove logo and use default"
            >
              Remove
            </button>
          )}

          <button
            type="button"
            className="btn btn-secondary btn-xs custom-provider-toggle-btn"
            onClick={() => setIsExpanded(!isExpanded)}
            aria-expanded={isExpanded}
            title={isExpanded ? 'Collapse logo options' : 'Expand logo options'}
          >
            <span>{isExpanded ? 'Hide' : (value ? 'Change logo' : 'Choose logo')}</span>
            <svg
              className={`custom-provider-toggle-chevron ${isExpanded ? 'open' : ''}`}
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.4"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <polyline points="6 9 12 15 18 9"></polyline>
            </svg>
          </button>
        </div>
      </div>

      {/* Expanded Content */}
      {isExpanded && (
        <div className="custom-provider-expanded-wrap animate-fade-in">
          {/* Tabs */}
          <div className="custom-provider-tabs" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'presets'}
          className={`custom-provider-tab ${activeTab === 'presets' ? 'active' : ''}`}
          onClick={() => setActiveTab('presets')}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
          </svg>
          AI Presets
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'upload'}
          className={`custom-provider-tab ${activeTab === 'upload' ? 'active' : ''}`}
          onClick={() => setActiveTab('upload')}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
          Upload / Crop
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'url_emoji'}
          className={`custom-provider-tab ${activeTab === 'url_emoji' ? 'active' : ''}`}
          onClick={() => setActiveTab('url_emoji')}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="10" />
            <line x1="2" y1="12" x2="22" y2="12" />
            <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
          </svg>
          URL / Emoji
        </button>
      </div>

      {/* Tab Panels */}
      <div className="custom-provider-tab-content">
        {/* Tab 1: AI Presets */}
        {activeTab === 'presets' && (
          <div className="custom-provider-presets-panel">
            <p className="custom-provider-section-desc">
              Select an official vector mark for your provider:
            </p>
            <div className="custom-provider-presets-grid" role="radiogroup" aria-label="Popular AI Presets">
              {PRESET_LOGOS.map((preset) => {
                const isSelected = value === preset.dataUrl
                return (
                  <button
                    key={preset.id}
                    type="button"
                    role="radio"
                    aria-checked={isSelected}
                    className={`custom-provider-preset-btn ${isSelected ? 'selected' : ''}`}
                    onClick={() => onChange(preset.dataUrl)}
                    title={preset.name}
                  >
                    <div className="custom-provider-preset-icon">
                      <img src={preset.dataUrl} alt={preset.name} />
                    </div>
                    <span className="custom-provider-preset-label">{preset.name}</span>
                    {isSelected && (
                      <span className="custom-provider-preset-check">✓</span>
                    )}
                  </button>
                )
              })}
            </div>
          </div>
        )}

        {/* Tab 2: Upload / Drag & Drop */}
        {activeTab === 'upload' && (
          <div className="custom-provider-upload-panel">
            <input
              ref={fileInputRef}
              type="file"
              accept=".svg,image/svg+xml,image/png,image/jpeg,image/webp"
              style={{ display: 'none' }}
              onChange={(e) => {
                const file = e.currentTarget.files?.[0]
                if (file) handleFileProcess(file)
                e.currentTarget.value = ''
              }}
            />

            <div
              className={`custom-provider-dropzone ${isDragging ? 'dragging' : ''}`}
              onClick={() => fileInputRef.current?.click()}
              onDragOver={(e) => {
                e.preventDefault()
                setIsDragging(true)
              }}
              onDragLeave={(e) => {
                e.preventDefault()
                setIsDragging(false)
              }}
              onDrop={(e) => {
                e.preventDefault()
                setIsDragging(false)
                const file = e.dataTransfer.files?.[0]
                if (file) handleFileProcess(file)
              }}
            >
              <div className="custom-provider-dropzone-icon">
                <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                  <polyline points="17 8 12 3 7 8" />
                  <line x1="12" y1="3" x2="12" y2="15" />
                </svg>
              </div>
              <p className="custom-provider-dropzone-text">
                <strong>Click to browse</strong> or drag & drop image here
              </p>
              <p className="custom-provider-dropzone-hint">
                Supports SVG, PNG, WebP, JPEG · Auto-resize & Crop included
              </p>
            </div>
          </div>
        )}

        {/* Tab 3: URL / Emoji */}
        {activeTab === 'url_emoji' && (
          <div className="custom-provider-url-emoji-panel">
            {/* Direct Image URL */}
            <div className="custom-provider-field-group">
              <label className="custom-provider-field-label">Image Web URL</label>
              <div className="custom-provider-url-row">
                <input
                  type="url"
                  value={urlInput}
                  onChange={(e) => setUrlInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault()
                      handleApplyUrl()
                    }
                  }}
                  placeholder="https://example.com/logo.png"
                  className="custom-provider-url-input"
                />
                <button
                  type="button"
                  className="btn btn-secondary btn-small"
                  onClick={handleApplyUrl}
                  disabled={!urlInput.trim()}
                >
                  Apply URL
                </button>
              </div>
            </div>

            {/* Emoji Quick Picker */}
            <div className="custom-provider-field-group">
              <label className="custom-provider-field-label">Or pick an Emoji badge</label>
              <div className="custom-provider-emoji-grid">
                {POPULAR_EMOJIS.map((emoji) => (
                  <button
                    key={emoji}
                    type="button"
                    className="custom-provider-emoji-btn"
                    onClick={() => handleApplyEmoji(emoji)}
                    title={`Use ${emoji}`}
                  >
                    {emoji}
                  </button>
                ))}
              </div>
            </div>

            {/* Custom Emoji Input */}
            <div className="custom-provider-field-group">
              <label className="custom-provider-field-label">Or enter custom emoji</label>
              <div className="custom-provider-url-row">
                <input
                  type="text"
                  maxLength={4}
                  value={customEmojiInput}
                  onChange={(e) => setCustomEmojiInput(e.target.value)}
                  placeholder="e.g. 🐉"
                  className="custom-provider-url-input custom-provider-emoji-input"
                />
                <button
                  type="button"
                  className="btn btn-secondary btn-small"
                  onClick={() => {
                    if (customEmojiInput.trim()) {
                      handleApplyEmoji(customEmojiInput.trim())
                      setCustomEmojiInput('')
                    }
                  }}
                  disabled={!customEmojiInput.trim()}
                >
                  Apply Emoji
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Error message */}
        {uploadError && (
          <div className="custom-provider-error-banner" role="alert">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="8" x2="12" y2="12" />
              <line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
            <span>{uploadError}</span>
          </div>
        )}
      </div>
    </div>
  )}
</div>
)
}
