import React from 'react'
import { DEFAULT_CONFIG } from '../SettingsWindow'

interface AudioTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
}

export default function AudioTab({ config, updateField }: AudioTabProps) {
  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Speech</p>
            <h2 className="section-title">Voice Reply</h2>
          </div>
          <p className="section-description">Control spoken responses and TTS behavior.</p>
        </div>

        <div className="toggle-row">
          <div>
            <label>Enable Voice Reply</label>
            <p className="hint">Mint will speak responses out loud when this is enabled.</p>
          </div>
          <label className="settings-toggle-switch">
            <input 
              type="checkbox" 
              checked={config.enableVoiceReply} 
              onChange={(e) => updateField('enableVoiceReply', e.target.checked)} 
            />
            <span className="settings-toggle-slider"></span>
          </label>
        </div>

        <div className="form-grid single">
          <div className="setting-row">
            <label>Voice Engine</label>
            <select value={config.ttsProvider} onChange={(e) => updateField('ttsProvider', e.target.value)}>
              <option value="google">Google Cloud (Natural, Auto Lang)</option>
              <option value="native">OS Native (Supports Pitch)</option>
            </select>
          </div>
        </div>

        <div className="slider-stack">
          <div className="setting-row">
            <label>Volume</label>
            <div className="slider-group">
              <input 
                type="range" 
                min="0" 
                max="1" 
                step="0.1" 
                value={config.ttsVolume} 
                onChange={(e) => updateField('ttsVolume', parseFloat(e.target.value))} 
                className="range-slider" 
              />
              <span className="range-value">{Math.round(config.ttsVolume * 100)}%</span>
            </div>
          </div>
          <div className="setting-row">
            <label>Speed</label>
            <div className="slider-group">
              <input 
                type="range" 
                min="0.5" 
                max="2" 
                step="0.1" 
                value={config.ttsSpeed} 
                onChange={(e) => updateField('ttsSpeed', parseFloat(e.target.value))} 
                className="range-slider" 
              />
              <span className="range-value">{parseFloat(String(config.ttsSpeed)).toFixed(1)}x</span>
            </div>
          </div>
          <div className="setting-row">
            <label>Pitch</label>
            <div className="slider-group">
              <input 
                type="range" 
                min="0" 
                max="2" 
                step="0.1" 
                value={config.ttsPitch} 
                onChange={(e) => updateField('ttsPitch', parseFloat(e.target.value))} 
                className="range-slider" 
              />
              <span className="range-value">{parseFloat(String(config.ttsPitch)).toFixed(1)}</span>
            </div>
            <p className="hint">Pitch applies to OS native voice only.</p>
          </div>
        </div>
      </section>
    </div>
  )
}
