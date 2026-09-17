import React, { useState } from 'react'
import { DEFAULT_CONFIG } from '../../constants/config'
import { THEME_PRESETS, ACCENT_FLAVORS, ThemePreset, AccentFlavor } from '../../theme/themes'

interface ThemeTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
  updateFields?: (fields: Partial<typeof DEFAULT_CONFIG>) => void
}

export default function ThemeTab({ config, updateField, updateFields }: ThemeTabProps) {
  const [showCustomAccent, setShowCustomAccent] = useState(false)

  const isPresetActive = (preset: ThemePreset) => {
    if (config.theme !== preset.theme) return false
    return (config.accentColor || '').toLowerCase() === preset.accentColor.toLowerCase()
  }

  const handleApplyPreset = (preset: ThemePreset) => {
    const patch: Partial<typeof DEFAULT_CONFIG> = {
      theme: preset.theme,
      accentColor: preset.accentColor,
      systemTextColor: preset.systemTextColor,
      chatTextColor: preset.chatTextColor,
      surfaceStyle: preset.theme === 'light' ? 'opaque' : 'glass',
    }
    if (updateFields) {
      updateFields(patch)
    } else {
      updateField('theme', preset.theme)
      updateField('accentColor', preset.accentColor)
      updateField('systemTextColor', preset.systemTextColor)
      updateField('chatTextColor', preset.chatTextColor)
      updateField('surfaceStyle', patch.surfaceStyle)
    }
  }

  const handleSelectFlavor = (flavor: AccentFlavor) => {
    if (updateFields) {
      updateFields({
        accentColor: flavor.color,
      })
    } else {
      updateField('accentColor', flavor.color)
    }
  }

  const COLOR_MODES = [
    {
      id: 'system',
      name: 'System',
      description: 'Auto-sync with OS',
      icon: (
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
          <line x1="8" y1="21" x2="16" y2="21"></line>
          <line x1="12" y1="17" x2="12" y2="21"></line>
        </svg>
      ),
    },
    {
      id: 'dark',
      name: 'Dark',
      description: 'Deep slate comfort',
      icon: (
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path>
        </svg>
      ),
    },
    {
      id: 'light',
      name: 'Light',
      description: 'Crisp daylight clarity',
      icon: (
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="5"></circle>
          <line x1="12" y1="1" x2="12" y2="3"></line>
          <line x1="12" y1="21" x2="12" y2="23"></line>
          <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line>
          <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line>
          <line x1="1" y1="12" x2="3" y2="12"></line>
          <line x1="21" y1="12" x2="23" y2="12"></line>
          <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line>
          <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line>
        </svg>
      ),
    },
    {
      id: 'midnight',
      name: 'Midnight',
      description: 'OLED pure black',
      icon: (
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
        </svg>
      ),
    },
  ]

  const currentAccent = (config.accentColor || '').toLowerCase()

  return (
    <div className="tab-pane active">
      {/* ── Section 1: Quick Presets ── */}
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Appearance</p>
            <h2 className="section-title">Theme Presets</h2>
            <p className="section-subtitle">Curated, complete visual profiles ready in one click</p>
          </div>
        </div>

        <div className="theme-preset-grid" role="radiogroup" aria-label="Theme Presets">
          {THEME_PRESETS.map((preset, index) => {
            const active = isPresetActive(preset)
            return (
              <button
                key={preset.id}
                type="button"
                role="radio"
                aria-checked={active}
                className={`theme-preset-card ${active ? 'active' : ''}`}
                onClick={() => handleApplyPreset(preset)}
              >
                <div
                  className="theme-preset-preview"
                  style={{ background: preset.bgPreview }}
                >
                  <div
                    className="preset-mini-window"
                    style={{ background: preset.panelPreview }}
                  >
                    <div className="preset-mini-header">
                      <span className="preset-mini-dot dot-red" />
                      <span className="preset-mini-dot dot-yellow" />
                      <span className="preset-mini-dot dot-green" />
                      <span
                        className="preset-mini-pill"
                        style={{
                          background: preset.accentPreview,
                          color: preset.accentPreview === '#ffffff' ? '#0a0a0b' : '#ffffff',
                        }}
                      >
                        Mint
                      </span>
                    </div>
                    <div className="preset-mini-body">
                      <div
                        className="preset-mini-text"
                        style={{ color: preset.textPreview }}
                      >
                        Aa
                      </div>
                      <div className="preset-mini-lines">
                        <span
                          className="preset-mini-line-main"
                          style={{ background: preset.textPreview, opacity: 0.85 }}
                        />
                        <span
                          className="preset-mini-line-accent"
                          style={{ background: preset.accentPreview }}
                        />
                      </div>
                    </div>
                  </div>
                </div>

                <div className="theme-preset-meta">
                  <div className="theme-preset-title-row">
                    <span className="theme-preset-number">{index + 1}.</span>
                    <span className="theme-preset-title">{preset.name}</span>
                    {active && (
                      <span className="theme-preset-active-check" title="Active preset">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                          <polyline points="20 6 9 17 4 12"></polyline>
                        </svg>
                      </span>
                    )}
                  </div>
                  <p className="theme-preset-desc">{preset.description}</p>
                </div>
              </button>
            )
          })}
        </div>
      </section>

      {/* ── Section 2: Core Customization (Mode & Accent) ── */}
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Core System</p>
            <h2 className="section-title">Color Mode & Accent</h2>
          </div>
        </div>

        <div className="form-grid">
          {/* Color Mode Cards */}
          <div className="setting-row stacked">
            <label>Color Mode</label>
            <div className="color-mode-grid" role="radiogroup" aria-label="Color Mode">
              {COLOR_MODES.map((mode) => {
                const isSelected = (config.theme || 'dark') === mode.id
                return (
                  <button
                    key={mode.id}
                    type="button"
                    role="radio"
                    aria-checked={isSelected}
                    className={`color-mode-card ${isSelected ? 'active' : ''}`}
                    onClick={() => {
                      if (mode.id === 'light') {
                        if (updateFields) {
                          updateFields({ theme: 'light', surfaceStyle: 'opaque' })
                        } else {
                          updateField('theme', 'light')
                          updateField('surfaceStyle', 'opaque')
                        }
                      } else {
                        updateField('theme', mode.id)
                      }
                    }}
                  >
                    <div className="mode-card-icon">{mode.icon}</div>
                    <div className="mode-card-text">
                      <span className="mode-card-title">{mode.name}</span>
                      <span className="mode-card-desc">{mode.description}</span>
                    </div>
                    {isSelected && (
                      <span className="mode-card-badge">
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                          <polyline points="20 6 9 17 4 12"></polyline>
                        </svg>
                      </span>
                    )}
                  </button>
                )
              })}
            </div>
          </div>

          {/* Accent Flavors */}
          <div className="setting-row stacked">
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <label>Accent Flavor</label>
              <button
                type="button"
                className="custom-accent-toggle"
                onClick={() => setShowCustomAccent(!showCustomAccent)}
              >
                {showCustomAccent ? 'Use Curated Flavors' : 'Custom Hex...'}
              </button>
            </div>

            {!showCustomAccent ? (
              <div className="flavor-grid" role="radiogroup" aria-label="Accent Flavors">
                {ACCENT_FLAVORS.map((flavor) => {
                  const isSelected = currentAccent === flavor.color.toLowerCase()
                  const isWhite = flavor.color.toLowerCase() === '#ffffff'
                  return (
                    <button
                      key={flavor.id}
                      type="button"
                      role="radio"
                      aria-checked={isSelected}
                      className={`flavor-card ${isSelected ? 'active' : ''}`}
                      onClick={() => handleSelectFlavor(flavor)}
                      title={flavor.description}
                    >
                      <span
                        className="flavor-swatch"
                        style={{
                          background: flavor.color,
                          border: isWhite ? '1px solid rgba(0,0,0,0.2)' : 'none',
                        }}
                      >
                        {isSelected && (
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke={flavor.contrastText} strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round">
                            <polyline points="20 6 9 17 4 12"></polyline>
                          </svg>
                        )}
                      </span>
                      <span className="flavor-name">{flavor.name}</span>
                    </button>
                  )
                })}
              </div>
            ) : (
              <div className="custom-accent-box">
                <input
                  type="color"
                  value={config.accentColor || '#10b981'}
                  onChange={(e) => updateField('accentColor', e.target.value)}
                  className="custom-accent-input"
                />
                <span className="custom-accent-value">{config.accentColor || '#10b981'}</span>
              </div>
            )}
          </div>

          {/* Surface Style: Opaque vs Glassmorphism */}
          <div className="setting-row stacked">
            <label>Surface Style</label>
            <div className="surface-style-grid" role="radiogroup" aria-label="Surface Style">
              <button
                type="button"
                role="radio"
                aria-checked={(config.surfaceStyle || 'glass') === 'opaque' || config.glassBlur === 'none'}
                className={`surface-card ${(config.surfaceStyle === 'opaque' || config.glassBlur === 'none') ? 'active' : ''}`}
                onClick={() => {
                  if (updateFields) {
                    updateFields({ surfaceStyle: 'opaque', glassBlur: 'none' })
                  } else {
                    updateField('surfaceStyle', 'opaque')
                    updateField('glassBlur', 'none')
                  }
                }}
              >
                <span className="surface-card-title">Opaque</span>
                <span className="surface-card-desc">Solid surfaces, maximal readability & zero bleed-through</span>
              </button>

              <button
                type="button"
                role="radio"
                aria-checked={(config.surfaceStyle || 'glass') === 'glass' && config.glassBlur !== 'none'}
                className={`surface-card ${(config.surfaceStyle !== 'opaque' && config.glassBlur !== 'none') ? 'active' : ''}`}
                onClick={() => {
                  if (updateFields) {
                    updateFields({ surfaceStyle: 'glass', glassBlur: 'blur(16px)' })
                  } else {
                    updateField('surfaceStyle', 'glass')
                    updateField('glassBlur', 'blur(16px)')
                  }
                }}
              >
                <span className="surface-card-title">Glassmorphism</span>
                <span className="surface-card-desc">Frosted translucent panels with protected modal overlays</span>
              </button>
            </div>
          </div>

          {/* Glass Blur Options (if glass is active) */}
          {config.surfaceStyle !== 'opaque' && config.glassBlur !== 'none' && (
            <div className="setting-row">
              <label>Blur Intensity</label>
              <div className="pill-segmented" role="radiogroup" aria-label="Glass Blur Intensity">
                {[
                  { id: 'blur(4px)', label: 'Low' },
                  { id: 'blur(16px)', label: 'Medium' },
                  { id: 'blur(32px)', label: 'High' },
                ].map((o) => (
                  <button
                    key={o.id}
                    type="button"
                    role="radio"
                    aria-checked={config.glassBlur === o.id}
                    className={`pill-segmented-btn ${config.glassBlur === o.id ? 'active' : ''}`}
                    onClick={() => updateField('glassBlur', o.id)}
                  >
                    {o.label}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      </section>

      {/* ── Section 3: Typography & Scale ── */}
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Typography</p>
            <h2 className="section-title">Font & Scale</h2>
          </div>
        </div>
        <div className="form-grid">
          <div className="setting-row">
            <label>Font Family</label>
            <select value={config.fontFamily} onChange={(e) => updateField('fontFamily', e.target.value)}>
              <option value="'Prompt', 'Noto Sans Thai', 'Inter', sans-serif">Prompt / Noto Sans Thai (ChatGPT Style - Default)</option>
              <option value="'Outfit', sans-serif">Outfit (Geometric Modern)</option>
              <option value="'Inter', sans-serif">Inter (Clean Sans-Serif)</option>
              <option value="'Prompt', sans-serif">Prompt (Modern Thai)</option>
              <option value="'Noto Sans Thai', sans-serif">Noto Sans Thai (Clean Thai)</option>
              <option value="'Sarabun', sans-serif">Sarabun (Formal Thai)</option>
              <option value="'Kanit', sans-serif">Kanit (Trendy Thai)</option>
              <option value="'Mitr', sans-serif">Mitr (Friendly Thai)</option>
              <option value="'Mali', cursive">Mali (Cute Thai Font)</option>
              <option value="'Fira Code', monospace">Fira Code (Developer Code Font)</option>
            </select>
          </div>
          <div className="setting-row">
            <label>Font Size</label>
            <div className="pill-segmented" role="radiogroup" aria-label="Font Size">
              {[
                { id: '16px', label: 'Small' },
                { id: '18px', label: 'Medium' },
                { id: '22px', label: 'Large' },
                { id: '26px', label: 'XL' },
                { id: '30px', label: 'XXL' },
              ].map((o) => (
                <button
                  key={o.id}
                  type="button"
                  role="radio"
                  aria-checked={config.fontSize === o.id}
                  className={`pill-segmented-btn ${config.fontSize === o.id ? 'active' : ''}`}
                  onClick={() => updateField('fontSize', o.id)}
                >
                  {o.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      </section>
    </div>
  )
}
