import React, { useState } from 'react'
import { DEFAULT_CONFIG } from '../SettingsWindow'
import { GEMINI_LIVE_MODELS, GEMINI_LIVE_VOICES } from '../../../shared/constants/models'

interface AudioTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
}

export default function AudioTab({ config, updateField }: AudioTabProps) {
  const [customLiveModel, setCustomLiveModel] = useState(false)
  const knownLiveModels = GEMINI_LIVE_MODELS as readonly string[]
  const isCustomLiveModel = customLiveModel || !knownLiveModels.includes(config.geminiLiveModel)
  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Speech</p>
            <h2 className="section-title">Voice Reply</h2>
          </div>
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

        <div className="toggle-row">
          <div>
            <label>Enable Gemini Live (Beta)</label>
            <p className="hint">
              Adds a separate "Live" button next to the mic that streams your voice straight to Google's realtime
              voice model for faster, more natural back-and-forth — including running Mint's tools by voice.
              Requires a Gemini API key.
            </p>
          </div>
          <label className="settings-toggle-switch">
            <input
              type="checkbox"
              checked={config.voiceMode === 'geminiLive'}
              onChange={(e) => updateField('voiceMode', e.target.checked ? 'geminiLive' : 'legacy')}
            />
            <span className="settings-toggle-slider"></span>
          </label>
        </div>

        {config.voiceMode === 'geminiLive' && (
          <div className="form-grid single">
            <div className="setting-row">
              <label>Realtime Live Model</label>
              <select
                value={isCustomLiveModel ? 'custom' : config.geminiLiveModel}
                onChange={(e) => {
                  if (e.target.value === 'custom') {
                    setCustomLiveModel(true)
                  } else {
                    setCustomLiveModel(false)
                    updateField('geminiLiveModel', e.target.value)
                  }
                }}
              >
                {GEMINI_LIVE_MODELS.map((model) => (
                  <option key={model} value={model}>{model}</option>
                ))}
                <option value="custom">Custom...</option>
              </select>
              <p className="hint">
                The model Gemini Live uses for realtime voice conversations.
                "gemini-3.1-flash-live-preview" requires requesting allowlist access from Google first — if it
                connects but never responds, try the 2.5 model instead.
              </p>
            </div>
            {isCustomLiveModel && (
              <div className="setting-row">
                <label>Custom Realtime Live Model</label>
                <input
                  type="text"
                  value={config.geminiLiveModel}
                  onChange={(e) => updateField('geminiLiveModel', e.target.value)}
                  placeholder="e.g. gemini-2.5-flash-native-audio-preview-12-2025"
                />
              </div>
            )}
            <div className="setting-row">
              <label>Live Voice</label>
              <select value={config.geminiLiveVoice} onChange={(e) => updateField('geminiLiveVoice', e.target.value)}>
                {GEMINI_LIVE_VOICES.map((voiceName) => (
                  <option key={voiceName} value={voiceName}>{voiceName}</option>
                ))}
              </select>
              <p className="hint">
                The voice Gemini Live speaks with. Can also be changed mid-call from the Live overlay itself.
              </p>
            </div>
          </div>
        )}

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
