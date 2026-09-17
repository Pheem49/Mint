import React from 'react'
import { DEFAULT_CONFIG } from '../../constants/config'

interface ThemePreset {
  id: string
  name: string
  description: string
  theme: 'dark' | 'light' | 'midnight'
  accentColor: string
  systemTextColor: string
  chatTextColor: string
  bgPreview: string
  panelPreview: string
  accentPreview: string
  textPreview: string
}

const THEME_PRESETS: ThemePreset[] = [
  {
    id: 'preset-dark-mint',
    name: 'Dark Mint',
    description: 'Black background · Green elements · White text',
    theme: 'dark',
    accentColor: '#10b981',
    systemTextColor: '#ffffff',
    chatTextColor: '#ffffff',
    bgPreview: '#0a0a0b',
    panelPreview: '#18181a',
    accentPreview: '#10b981',
    textPreview: '#ffffff',
  },
  {
    id: 'preset-light-mint',
    name: 'Light Mint',
    description: 'White background · Green elements · Black text',
    theme: 'light',
    accentColor: '#10b981',
    systemTextColor: '#000000',
    chatTextColor: '#000000',
    bgPreview: '#f8fafc',
    panelPreview: '#ffffff',
    accentPreview: '#10b981',
    textPreview: '#0f172a',
  },
  {
    id: 'preset-dark-mono',
    name: 'Dark Monochrome',
    description: 'Black background · White elements · White text',
    theme: 'dark',
    accentColor: '#ffffff',
    systemTextColor: '#ffffff',
    chatTextColor: '#ffffff',
    bgPreview: '#0a0a0b',
    panelPreview: '#18181a',
    accentPreview: '#ffffff',
    textPreview: '#ffffff',
  },
]

interface ThemeTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
  updateFields?: (fields: Partial<typeof DEFAULT_CONFIG>) => void
}

export default function ThemeTab({ config, updateField, updateFields }: ThemeTabProps) {
  const isPresetActive = (preset: ThemePreset) => {
    if (config.theme !== preset.theme) return false
    if ((config.accentColor || '').toLowerCase() !== preset.accentColor.toLowerCase()) return false

    const curText = (config.systemTextColor || '').toLowerCase()
    if (preset.systemTextColor === '#ffffff') {
      return curText === '#ffffff' || curText === '#f8fafc' || curText === ''
    }
    if (preset.systemTextColor === '#000000') {
      return curText === '#000000' || curText === '#0f172a'
    }
    return curText === preset.systemTextColor.toLowerCase()
  }

  const handleApplyPreset = (preset: ThemePreset) => {
    const patch = {
      theme: preset.theme,
      accentColor: preset.accentColor,
      systemTextColor: preset.systemTextColor,
      chatTextColor: preset.chatTextColor,
    }
    if (updateFields) {
      updateFields(patch)
    } else {
      updateField('theme', preset.theme)
      updateField('accentColor', preset.accentColor)
      updateField('systemTextColor', preset.systemTextColor)
      updateField('chatTextColor', preset.chatTextColor)
    }
  }

  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Appearance</p>
            <h2 className="section-title">Theme</h2>
          </div>
        </div>

        {/* Theme Presets */}
        <div className="setting-row stacked">
          <div className="theme-preset-header">
            <div>
              <label>Theme Presets</label>
              <p className="hint">Select from 3 curated presets (Background · Elements · Typography)</p>
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
                    <div className="theme-preset-swatches">
                      <span className="preset-swatch-item" title="Background color">
                        <span className="preset-swatch-circle" style={{ background: preset.bgPreview, border: '1px solid rgba(255,255,255,0.25)' }} />
                        <span className="preset-swatch-label">Background</span>
                      </span>
                      <span className="preset-swatch-item" title="Elements color">
                        <span className="preset-swatch-circle" style={{ background: preset.accentPreview, border: preset.accentPreview === '#ffffff' ? '1px solid rgba(0,0,0,0.2)' : 'none' }} />
                        <span className="preset-swatch-label">Elements</span>
                      </span>
                      <span className="preset-swatch-item" title="Typography color">
                        <span className="preset-swatch-circle" style={{ background: preset.textPreview, border: preset.textPreview === '#ffffff' ? '1px solid rgba(255,255,255,0.4)' : '1px solid rgba(0,0,0,0.2)' }} />
                        <span className="preset-swatch-label">Text</span>
                      </span>
                    </div>
                  </div>
                </button>
              )
            })}
          </div>
        </div>

        <div className="setting-row stacked">
          <label>Appearance</label>
          <div className="theme-segmented" role="radiogroup" aria-label="Theme">
            {[
              {
                id: 'dark',
                label: 'Dark',
                icon: (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path>
                  </svg>
                )
              },
              {
                id: 'light',
                label: 'Light',
                icon: (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
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
                )
              },
              {
                id: 'midnight',
                label: 'Midnight',
                icon: (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
                  </svg>
                )
              },
              {
                id: 'custom',
                label: 'Custom',
                icon: (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="13.5" cy="6.5" r=".5" fill="currentColor"></circle>
                    <circle cx="17.5" cy="10.5" r=".5" fill="currentColor"></circle>
                    <circle cx="8.5" cy="7.5" r=".5" fill="currentColor"></circle>
                    <circle cx="6.5" cy="12.5" r=".5" fill="currentColor"></circle>
                    <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.92 0 1.7-.74 1.7-1.67 0-.44-.18-.86-.48-1.17-.3-.3-.48-.73-.48-1.16 0-.92.74-1.67 1.67-1.67h2.29c3.09 0 5.6-2.51 5.6-5.6 0-5.25-4.25-9.7-9.7-9.7z"></path>
                  </svg>
                )
              }
            ].map(t => (
              <button
                key={t.id}
                type="button"
                role="radio"
                aria-checked={config.theme === t.id}
                title={t.label}
                className={`theme-segmented-btn ${config.theme === t.id ? 'active' : ''}`}
                onClick={() => updateField('theme', t.id)}
              >
                {t.icon}
                <span className={`theme-segmented-swatch ${t.id}-preview`} aria-hidden="true" />
              </button>
            ))}
          </div>
        </div>

        {config.theme === 'custom' && (
          <div className="custom-theme-panel" style={{ marginTop: '15px' }}>
            <div className="setting-row">
              <label>Background Gradient</label>
              <div className="color-range" style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                <input type="color" value={config.customBgStart} onChange={(e) => updateField('customBgStart', e.target.value)} />
                <span style={{ display: 'inline-flex', alignItems: 'center', color: 'var(--text-soft)' }}>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <line x1="5" y1="12" x2="19" y2="12"></line>
                    <polyline points="12 5 19 12 12 19"></polyline>
                  </svg>
                </span>
                <input type="color" value={config.customBgEnd} onChange={(e) => updateField('customBgEnd', e.target.value)} />
              </div>
            </div>
            <div className="setting-row">
              <label>Panel Background</label>
              <input type="color" value={config.customPanelBg} onChange={(e) => updateField('customPanelBg', e.target.value)} />
            </div>
          </div>
        )}
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Color</p>
            <h2 className="section-title">Accent & Text</h2>
          </div>
        </div>
        <div className="form-grid">
          <div className="setting-row">
            <label>Accent Color</label>
            <div className="color-presets">
              {['#8b5cf6', '#06b6d4', '#10b981', '#f59e0b', '#ef4444', '#ec4899', '#ffffff'].map(c => {
                const isSelected = (config.accentColor || '').toLowerCase() === c.toLowerCase()
                const isWhite = c.toLowerCase() === '#ffffff'
                return (
                  <button
                    key={c}
                    className="color-dot"
                    style={{
                      backgroundColor: c,
                      border: isSelected
                        ? (isWhite ? '2px solid #10b981' : '2px solid white')
                        : (isWhite ? '1px solid rgba(255, 255, 255, 0.35)' : '2px solid transparent'),
                      boxShadow: isSelected
                        ? (isWhite ? '0 0 10px rgba(255, 255, 255, 0.7)' : `0 0 10px ${c}`)
                        : 'none',
                    }}
                    onClick={() => updateField('accentColor', c)}
                    aria-label={`Select accent color ${c}`}
                  >
                    {isSelected && (
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke={isWhite ? '#09090b' : 'white'} strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round">
                        <polyline points="20 6 9 17 4 12"></polyline>
                      </svg>
                    )}
                  </button>
                )
              })}
            </div>
          </div>
          <div className="setting-row">
            <label>Custom Accent</label>
            <input type="color" value={config.accentColor} onChange={(e) => updateField('accentColor', e.target.value)} />
          </div>
          <div className="setting-row">
            <label>System Text</label>
            <input type="color" value={config.systemTextColor} onChange={(e) => updateField('systemTextColor', e.target.value)} />
          </div>
          <div className="setting-row">
            <label>Chat Text</label>
            <input type="color" value={config.chatTextColor || config.systemTextColor} onChange={(e) => updateField('chatTextColor', e.target.value)} />
            <p className="hint">Color of chat message body text and code snippets. Defaults to System Text if not set.</p>
          </div>
        </div>
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Surface</p>
            <h2 className="section-title">Interface Style</h2>
          </div>
        </div>
        <div className="form-grid">
          <div className="setting-row">
            <label>Glass Blur</label>
            <div className="pill-segmented" role="radiogroup" aria-label="Glass Blur">
              {[
                { id: 'blur(4px)', label: 'Low' },
                { id: 'blur(16px)', label: 'Medium' },
                { id: 'blur(32px)', label: 'High' },
                { id: 'none', label: 'Off' },
              ].map(o => (
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
              ].map(o => (
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
