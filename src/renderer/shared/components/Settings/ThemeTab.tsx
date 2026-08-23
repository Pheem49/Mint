import React from 'react'
import { DEFAULT_CONFIG } from '@/components/SettingsWindow'

interface ThemeTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
}

export default function ThemeTab({ config, updateField }: ThemeTabProps) {
  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Appearance</p>
            <h2 className="section-title">Theme</h2>
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
              {['#8b5cf6', '#06b6d4', '#10b981', '#f59e0b', '#ef4444', '#ec4899'].map(c => (
                <button
                  key={c}
                  className="color-dot"
                  style={{
                    backgroundColor: c,
                    border: config.accentColor === c ? '2px solid white' : '2px solid transparent',
                    boxShadow: config.accentColor === c ? `0 0 10px ${c}` : 'none',
                  }}
                  onClick={() => updateField('accentColor', c)}
                  aria-label={`Select accent color ${c}`}
                >
                  {config.accentColor === c && (
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                  )}
                </button>
              ))}
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
            <select value={config.glassBlur} onChange={(e) => updateField('glassBlur', e.target.value)}>
              <option value="blur(4px)">Low (4px)</option>
              <option value="blur(16px)">Medium (16px) - Default</option>
              <option value="blur(32px)">High (32px)</option>
              <option value="none">Off (Solid)</option>
            </select>
          </div>
          <div className="setting-row">
            <label>Font Family</label>
            <select value={config.fontFamily} onChange={(e) => updateField('fontFamily', e.target.value)}>
              <option value="'Outfit', sans-serif">Outfit (Default)</option>
              <option value="'Prompt', sans-serif">Prompt (Modern Thai)</option>
              <option value="'Sarabun', sans-serif">Sarabun (Formal Thai)</option>
              <option value="'Kanit', sans-serif">Kanit (Trendy Thai)</option>
              <option value="'Mitr', sans-serif">Mitr (Friendly Thai)</option>
              <option value="'Noto Sans Thai', sans-serif">Noto Sans Thai (Clean Thai)</option>
              <option value="'Mali', cursive">Mali (Cute Thai Font)</option>
              <option value="'Inter', sans-serif">Inter (Clean Sans-Serif)</option>
              <option value="'Fira Code', monospace">Fira Code (Developer Code Font)</option>
            </select>
          </div>
          <div className="setting-row">
            <label>Font Size</label>
            <select value={config.fontSize} onChange={(e) => updateField('fontSize', e.target.value)}>
              <option value="16px">Small</option>
              <option value="18px">Medium (Default)</option>
              <option value="22px">Large</option>
              <option value="26px">Extra Large</option>
              <option value="30px">Extra Extra Large</option>
            </select>
          </div>
        </div>
      </section>
    </div>
  )
}
