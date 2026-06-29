import React from 'react'
import { DEFAULT_CONFIG } from '../SettingsWindow' // we can import it or define type locally

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
        <div className="theme-grid" style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '12px' }}>
          {(['dark', 'light', 'midnight', 'custom'] as const).map(t => (
            <button 
              key={t}
              className={`theme-card ${config.theme === t ? 'active' : ''}`} 
              onClick={() => updateField('theme', t)}
            >
              <div className={`theme-preview ${t}-preview`}></div>
              <span style={{ textTransform: 'capitalize' }}>{t}</span>
            </button>
          ))}
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
        <div className="color-section">
          <div>
            <label>Accent Color</label>
            <div className="color-presets">
              {['#8b5cf6', '#06b6d4', '#10b981', '#f59e0b', '#ef4444', '#ec4899'].map(c => (
                <button 
                  key={c}
                  className="color-dot" 
                  style={{ backgroundColor: c, border: config.accentColor === c ? '2px solid white' : 'none' }} 
                  onClick={() => updateField('accentColor', c)}
                />
              ))}
            </div>
          </div>
          <div className="color-inputs" style={{ display: 'flex', gap: '15px', marginTop: '12px' }}>
            <div>
              <label>Custom Accent</label>
              <input type="color" value={config.accentColor} onChange={(e) => updateField('accentColor', e.target.value)} style={{ display: 'block', marginTop: '4px' }} />
            </div>
            <div>
              <label>System Text</label>
              <input type="color" value={config.systemTextColor} onChange={(e) => updateField('systemTextColor', e.target.value)} style={{ display: 'block', marginTop: '4px' }} />
            </div>
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
              <option value="'Mali', cursive">Mali (Cute Thai Font)</option>
              <option value="'Prompt', sans-serif">Prompt (Modern Thai)</option>
              <option value="'Sarabun', sans-serif">Sarabun (Formal Thai)</option>
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
