import React from 'react'
import { DEFAULT_CONFIG } from '../SettingsWindow'

interface AutomationTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
  handleOpenWorkflows: () => void
  handleReloadWorkflows: () => void
  isDesktopApp?: boolean
}

export default function AutomationTab({
  config,
  updateField,
  handleOpenWorkflows,
  handleReloadWorkflows,
  isDesktopApp = false
}: AutomationTabProps) {
  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Browser</p>
            <h2 className="section-title">Automation Engine</h2>
          </div>
        </div>
        <div className="form-grid single">
          <div className="setting-row">
            <label>Browser Engine</label>
            <select value={config.automationBrowser} onChange={(e) => updateField('automationBrowser', e.target.value)}>
              <option value="chromium">Chromium (Bundled)</option>
              <option value="/usr/bin/firefox">Firefox (System - Linux)</option>
            </select>
          </div>
          <div className="setting-row">
            <label>Chromium DevTools Endpoint</label>
            <input type="text" value={config.browserDebugUrl} onChange={(e) => updateField('browserDebugUrl', e.target.value)} />
            <p className="hint">Required for native tab reading and selector clicks.</p>
          </div>
          <div className="setting-row">
            <label>Browser Extension Context Endpoint</label>
            <input type="text" value={config.browserExtensionContextUrl} onChange={(e) => updateField('browserExtensionContextUrl', e.target.value)} />
            <p className="hint">Fallback endpoint used when Chromium remote debugging is unavailable.</p>
          </div>
        </div>
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Background tasks</p>
            <h2 className="section-title">Native Headless Queue</h2>
          </div>
        </div>
        <div className="toggle-row">
          <div>
            <label>Process queued tasks automatically</label>
            <p className="hint">Allow the bounded Rust worker to process pending tasks every 15 seconds.</p>
          </div>
          <label className="settings-toggle-switch">
            <input
              type="checkbox"
              checked={config.enableHeadlessTaskQueue}
              onChange={(e) => updateField('enableHeadlessTaskQueue', e.target.checked)}
            />
            <span className="settings-toggle-slider"></span>
          </label>
        </div>
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Awareness</p>
            <h2 className="section-title">Proactive Assistant</h2>
          </div>
          <p className="section-description">Tune screen analysis frequency and suggestion timing.</p>
        </div>
        <div className="slider-stack">
          <div className="setting-row">
            <label>Screen Capture Frequency</label>
            <div className="slider-group">
              <input 
                type="range" 
                min="30" 
                max="300" 
                step="30" 
                value={config.proactiveInterval} 
                onChange={(e) => updateField('proactiveInterval', parseInt(e.target.value))} 
                className="range-slider" 
              />
              <span className="range-value">{config.proactiveInterval} sec</span>
            </div>
            <p className="hint">Lower values respond faster but use more API calls.</p>
          </div>
          <div className="setting-row">
            <label>Suggestion Cooldown</label>
            <div className="slider-group">
              <input 
                type="range" 
                min="60" 
                max="600" 
                step="60" 
                value={config.proactiveCooldown} 
                onChange={(e) => updateField('proactiveCooldown', parseInt(e.target.value))} 
                className="range-slider" 
              />
              <span className="range-value">{Math.round(config.proactiveCooldown / 60)} min</span>
            </div>
            <p className="hint">Minimum time between repeat suggestions.</p>
          </div>
        </div>
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Messaging</p>
            <h2 className="section-title">Native Channel Bridges</h2>
          </div>
          <p className="section-description">Configure credentials used directly by the Rust channel workers.</p>
        </div>
        <div className="form-grid">
          <label className="toggle-row"><span>Telegram Bot API</span><input type="checkbox" checked={config.enableTelegramBridge} onChange={(e) => updateField('enableTelegramBridge', e.target.checked)} /></label>
          <input type="password" placeholder="Telegram bot token" value={config.telegramBotToken} onChange={(e) => updateField('telegramBotToken', e.target.value)} />
          <label className="toggle-row"><span>Discord Gateway</span><input type="checkbox" checked={config.enableDiscordBridge} onChange={(e) => updateField('enableDiscordBridge', e.target.checked)} /></label>
          <input type="password" placeholder="Discord bot token" value={config.discordBotToken} onChange={(e) => updateField('discordBotToken', e.target.value)} />
          <input type="text" placeholder="Discord application ID for Rich Presence" value={config.discordApplicationId} onChange={(e) => updateField('discordApplicationId', e.target.value)} />
          <label className="toggle-row"><span>Slack Socket Mode</span><input type="checkbox" checked={config.enableSlackBridge} onChange={(e) => updateField('enableSlackBridge', e.target.checked)} /></label>
          <input type="password" placeholder="Slack bot token (xoxb-...)" value={config.slackBotToken} onChange={(e) => updateField('slackBotToken', e.target.value)} />
          <input type="password" placeholder="Slack app token (xapp-...)" value={config.slackAppToken} onChange={(e) => updateField('slackAppToken', e.target.value)} />
          <label className="toggle-row"><span>LINE Webhook</span><input type="checkbox" checked={config.enableLineBridge} onChange={(e) => updateField('enableLineBridge', e.target.checked)} /></label>
          <input type="password" placeholder="LINE channel access token" value={config.lineChannelAccessToken} onChange={(e) => updateField('lineChannelAccessToken', e.target.value)} />
          <input type="password" placeholder="LINE channel secret" value={config.lineChannelSecret} onChange={(e) => updateField('lineChannelSecret', e.target.value)} />
          <label className="toggle-row"><span>WhatsApp Cloud API</span><input type="checkbox" checked={config.enableWhatsappBridge} onChange={(e) => updateField('enableWhatsappBridge', e.target.checked)} /></label>
          <input type="password" placeholder="WhatsApp Cloud access token" value={config.whatsappCloudAccessToken} onChange={(e) => updateField('whatsappCloudAccessToken', e.target.value)} />
          <input type="text" placeholder="WhatsApp phone number ID" value={config.whatsappPhoneNumberId} onChange={(e) => updateField('whatsappPhoneNumberId', e.target.value)} />
          <input type="password" placeholder="WhatsApp verify token" value={config.whatsappVerifyToken} onChange={(e) => updateField('whatsappVerifyToken', e.target.value)} />
          <input type="password" placeholder="WhatsApp app secret (optional HMAC validation)" value={config.whatsappAppSecret} onChange={(e) => updateField('whatsappAppSecret', e.target.value)} />
        </div>
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Rules</p>
            <h2 className="section-title">Custom Workflows</h2>
          </div>
        </div>
        <div className="toggle-row">
          <div>
            <label>Enable Custom Workflows</label>
            <p className="hint">Run "If This Then Mint" rules from the workflow JSON file.</p>
          </div>
          <label className="settings-toggle-switch">
            <input 
              type="checkbox" 
              checked={config.enableCustomWorkflows} 
              onChange={(e) => updateField('enableCustomWorkflows', e.target.checked)} 
            />
            <span className="settings-toggle-slider"></span>
          </label>
        </div>
        <div className="button-row" style={{ display: 'flex', gap: '10px', marginTop: '10px' }}>
          <button className="btn btn-secondary" onClick={handleOpenWorkflows} disabled={!isDesktopApp}>Open workflows.json</button>
          <button className="btn btn-primary" onClick={handleReloadWorkflows} disabled={!isDesktopApp}>Reload Rules</button>
        </div>
        {!isDesktopApp && <p className="hint">Opening and reloading local workflow files requires the desktop app.</p>}
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Code mode</p>
            <h2 className="section-title">Agent Collaboration</h2>
          </div>
        </div>
        <div className="toggle-row">
          <div>
            <label>Enable Multi-Agent Review</label>
            <p className="hint">Allow a secondary model to review code written by the primary model.</p>
          </div>
          <label className="settings-toggle-switch">
            <input 
              type="checkbox" 
              checked={config.enableAgentCollaboration} 
              onChange={(e) => updateField('enableAgentCollaboration', e.target.checked)} 
            />
            <span className="settings-toggle-slider"></span>
          </label>
        </div>
      </section>
    </div>
  )
}
