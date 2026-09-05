import React from 'react'
import { DEFAULT_CONFIG } from '@/components/SettingsWindow'
import ApiKeyInput from './ApiKeyInput'

interface AutomationTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
}

export default function AutomationTab({
  config,
  updateField
}: AutomationTabProps) {
  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Browser</p>
            <h2 className="section-title" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
                <line x1="8" y1="21" x2="16" y2="21"></line>
                <line x1="12" y1="17" x2="12" y2="21"></line>
              </svg>
              Automation Engine
            </h2>
          </div>
        </div>
        <div className="form-grid single">
          <div className="setting-row">
            <label>Browser Engine</label>
            <div className="pill-segmented" role="radiogroup" aria-label="Browser Engine">
              {[
                { id: 'chromium', label: 'Chromium' },
                { id: '/usr/bin/firefox', label: 'Firefox' },
              ].map(o => (
                <button
                  key={o.id}
                  type="button"
                  role="radio"
                  aria-checked={config.automationBrowser === o.id}
                  className={`pill-segmented-btn ${config.automationBrowser === o.id ? 'active' : ''}`}
                  onClick={() => updateField('automationBrowser', o.id)}
                >
                  {o.label}
                </button>
              ))}
            </div>
            <p className="hint">Chromium is bundled; Firefox uses your system install (Linux).</p>
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
            <p className="section-kicker">Behavior</p>
            <h2 className="section-title">Agent Behavior</h2>
          </div>
        </div>
        <div className="toggle-card">
          <div className="toggle-row">
            <div>
              <label>Process queued tasks automatically</label>
              <p className="hint">Runs pending tasks in the background every 15 seconds.</p>
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
          <div className="toggle-row">
            <div>
              <label>Write a skill after finishing a hard task</label>
              <p className="hint">Saves a reusable skill after a multi-step task, so it won't have to re-derive the solution next time.</p>
            </div>
            <label className="settings-toggle-switch">
              <input
                type="checkbox"
                checked={config.autoSkillWriting}
                onChange={(e) => updateField('autoSkillWriting', e.target.checked)}
              />
              <span className="settings-toggle-slider"></span>
            </label>
          </div>
        </div>
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Awareness</p>
            <h2 className="section-title" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="12" y1="8" x2="12" y2="12"></line>
                <line x1="12" y1="16" x2="12.01" y2="16"></line>
              </svg>
              Proactive Assistant
            </h2>
          </div>
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
        </div>

        {/* Global Instant Ack Notification */}
        <div className={`bridge-card ${config.enableBridgeAckNotification !== false ? 'active' : ''}`} style={{ marginBottom: '16px' }}>
          <div className="bridge-card-header">
            <div className="bridge-info">
              <div className="bridge-icon-badge" style={{ background: 'linear-gradient(135deg, #10b981 0%, #059669 100%)' }}>
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"></polygon>
                </svg>
              </div>
              <div className="bridge-title-group">
                <span className="bridge-title">
                  Instant Remote Acknowledgement
                  <span className={`bridge-status-pill ${config.enableBridgeAckNotification !== false ? 'active' : 'inactive'}`}>
                    {config.enableBridgeAckNotification !== false ? 'Enabled' : 'Disabled'}
                  </span>
                </span>
                <span className="bridge-subtitle">Send instant feedback/typing status when remote commands arrive</span>
              </div>
            </div>
            <label className="settings-toggle-switch">
              <input
                type="checkbox"
                checked={config.enableBridgeAckNotification !== false}
                onChange={(e) => updateField('enableBridgeAckNotification', e.target.checked)}
              />
              <span className="settings-toggle-slider"></span>
            </label>
          </div>
          {config.enableBridgeAckNotification !== false && (
            <div className="bridge-inputs">
              <input
                type="text"
                className="bridge-input"
                placeholder="Instant Ack Message (default: [Mint Agent] Remote command received, processing...)"
                value={config.bridgeAckMessage || ''}
                onChange={(e) => updateField('bridgeAckMessage', e.target.value)}
              />
            </div>
          )}
        </div>

        <div className="bridge-cards-container">
          {/* Telegram Bot Card */}
          <div className={`bridge-card ${config.enableTelegramBridge ? 'active' : ''}`}>
            <div className="bridge-card-header">
              <div className="bridge-info">
                <div className="bridge-icon-badge bridge-icon-telegram">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm4.64 6.8c-.15 1.58-.8 5.42-1.13 7.19-.14.75-.42 1-.68 1.03-.58.05-1.02-.38-1.58-.75-.88-.58-1.38-.94-2.23-1.5-.99-.65-.35-1.01.22-1.59.15-.15 2.71-2.48 2.76-2.69.01-.03.01-.14-.07-.2-.08-.06-.19-.04-.27-.02-.12.02-1.96 1.25-5.54 3.69-.52.36-1 .54-1.42.53-.47-.01-1.37-.26-2.03-.48-.82-.27-1.47-.42-1.42-.88.03-.25.38-.51 1.07-.78 4.18-1.82 6.97-3.02 8.37-3.61 3.98-1.66 4.81-1.95 5.35-1.96.12 0 .38.03.55.17.14.12.18.28.2.4.02.11.02.26.01.39z"/>
                  </svg>
                </div>
                <div className="bridge-title-group">
                  <span className="bridge-title">
                    Telegram Bot API
                    <span className={`bridge-status-pill ${config.enableTelegramBridge ? 'active' : 'inactive'}`}>
                      {config.enableTelegramBridge ? 'Active' : 'Disabled'}
                    </span>
                  </span>
                  <span className="bridge-subtitle">Long Polling Engine (getUpdates)</span>
                </div>
              </div>
              <label className="settings-toggle-switch">
                <input
                  type="checkbox"
                  checked={config.enableTelegramBridge}
                  onChange={(e) => updateField('enableTelegramBridge', e.target.checked)}
                />
                <span className="settings-toggle-slider"></span>
              </label>
            </div>
            <div className="bridge-inputs">
              <ApiKeyInput
                className="bridge-input"
                placeholder="Telegram Bot Token (from @BotFather)"
                value={config.telegramBotToken}
                onChange={(value) => updateField('telegramBotToken', value)}
              />
            </div>
          </div>

          {/* Discord Gateway Card */}
          <div className={`bridge-card ${config.enableDiscordBridge ? 'active' : ''}`}>
            <div className="bridge-card-header">
              <div className="bridge-info">
                <div className="bridge-icon-badge bridge-icon-discord">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.061 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.893.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.028zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
                  </svg>
                </div>
                <div className="bridge-title-group">
                  <span className="bridge-title">
                    Discord Gateway
                    <span className={`bridge-status-pill ${config.enableDiscordBridge ? 'active' : 'inactive'}`}>
                      {config.enableDiscordBridge ? 'Active' : 'Disabled'}
                    </span>
                  </span>
                  <span className="bridge-subtitle">WebSocket Gateway (wss://gateway.discord.gg)</span>
                </div>
              </div>
              <label className="settings-toggle-switch">
                <input
                  type="checkbox"
                  checked={config.enableDiscordBridge}
                  onChange={(e) => updateField('enableDiscordBridge', e.target.checked)}
                />
                <span className="settings-toggle-slider"></span>
              </label>
            </div>
            <div className="bridge-inputs">
              <ApiKeyInput
                className="bridge-input"
                placeholder="Discord Bot Token"
                value={config.discordBotToken}
                onChange={(value) => updateField('discordBotToken', value)}
              />
              <input
                type="text"
                className="bridge-input"
                placeholder="Discord Application ID (for Rich Presence)"
                value={config.discordApplicationId}
                onChange={(e) => updateField('discordApplicationId', e.target.value)}
              />
            </div>
          </div>

          {/* Slack Socket Mode Card */}
          <div className={`bridge-card ${config.enableSlackBridge ? 'active' : ''}`}>
            <div className="bridge-card-header">
              <div className="bridge-info">
                <div className="bridge-icon-badge bridge-icon-slack">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M6 15a2.5 2.5 0 0 1-2.5-2.5A2.5 2.5 0 0 1 6 10h2.5v2.5A2.5 2.5 0 0 1 6 15zm0-8.5A2.5 2.5 0 0 1 3.5 4 2.5 2.5 0 0 1 6 1.5 2.5 2.5 0 0 1 8.5 4v2.5H6zm8.5 0A2.5 2.5 0 0 1 12 4a2.5 2.5 0 0 1 2.5-2.5A2.5 2.5 0 0 1 17 4v2.5h-2.5zm0 8.5a2.5 2.5 0 0 1 2.5 2.5 2.5 2.5 0 0 1-2.5 2.5A2.5 2.5 0 0 1 12 17.5V15h2.5zm-8.5 2.5A2.5 2.5 0 0 1 6 20a2.5 2.5 0 0 1-2.5-2.5 2.5 2.5 0 0 1 2.5-2.5H8.5V17.5zm8.5-8.5A2.5 2.5 0 0 1 17 6.5 2.5 2.5 0 0 1 19.5 9 2.5 2.5 0 0 1 17 11.5H14.5V9zm-2.5-2.5a2.5 2.5 0 0 1-2.5-2.5A2.5 2.5 0 0 1 12 1.5a2.5 2.5 0 0 1 2.5 2.5V6.5H12z"/>
                  </svg>
                </div>
                <div className="bridge-title-group">
                  <span className="bridge-title">
                    Slack Socket Mode
                    <span className={`bridge-status-pill ${config.enableSlackBridge ? 'active' : 'inactive'}`}>
                      {config.enableSlackBridge ? 'Active' : 'Disabled'}
                    </span>
                  </span>
                  <span className="bridge-subtitle">WebSocket Socket Mode API</span>
                </div>
              </div>
              <label className="settings-toggle-switch">
                <input
                  type="checkbox"
                  checked={config.enableSlackBridge}
                  onChange={(e) => updateField('enableSlackBridge', e.target.checked)}
                />
                <span className="settings-toggle-slider"></span>
              </label>
            </div>
            <div className="bridge-inputs">
              <ApiKeyInput
                className="bridge-input"
                placeholder="Slack Bot Token (xoxb-...)"
                value={config.slackBotToken}
                onChange={(value) => updateField('slackBotToken', value)}
              />
              <ApiKeyInput
                className="bridge-input"
                placeholder="Slack App Token (xapp-...)"
                value={config.slackAppToken}
                onChange={(value) => updateField('slackAppToken', value)}
              />
            </div>
          </div>

          {/* LINE Webhook Card */}
          <div className={`bridge-card ${config.enableLineBridge ? 'active' : ''}`}>
            <div className="bridge-card-header">
              <div className="bridge-info">
                <div className="bridge-icon-badge bridge-icon-line">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M19.365 9.863c.349 0 .63.285.63.631 0 .345-.281.63-.63.63h-2.286v1.442h2.286c.349 0 .63.285.63.63 0 .346-.281.63-.63.63h-2.917c-.349 0-.63-.284-.63-.63V8.581c0-.346.281-.63.63-.63h2.917c.349 0 .63.284.63.63 0 .346-.281.63-.63.63h-2.286V9.86h2.286zm-5.631 4.156c0 .248-.146.472-.371.57-.083.036-.172.054-.26.054-.154 0-.306-.056-.425-.165l-2.481-3.238v2.783c0 .346-.28.63-.63.63-.348 0-.63-.284-.63-.63V8.581c0-.248.146-.472.371-.57.225-.098.486-.051.664.12l2.48 3.237V8.581c0-.346.28-.63.63-.63.349 0 .63.284.63.63v5.438zm-6.843 0c0 .346-.28.63-.63.63-.349 0-.63-.284-.63-.63V8.581c0-.346.281-.63.63-.63.35 0 .63.284.63.63v5.438zm-2.434 0c0 .346-.28.63-.63.63h-2.417c-.349 0-.63-.284-.63-.63V8.581c0-.346.281-.63.63-.63.349 0 .63.284.63.63v4.808h1.787c.349 0 .63.285.63.63zM24 10.617C24 4.754 18.627 0 12 0S0 4.754 0 10.617c0 5.247 4.264 9.61 10.02 10.435.39.084.922.257 1.057.592.121.3.079.77.039 1.074l-.17.994c-.052.304-.247 1.191 1.044.649 1.292-.542 6.974-4.107 9.513-7.032C23.013 15.602 24 13.23 24 10.617z"/>
                  </svg>
                </div>
                <div className="bridge-title-group">
                  <span className="bridge-title">
                    LINE Messaging Webhook
                    <span className={`bridge-status-pill ${config.enableLineBridge ? 'active' : 'inactive'}`}>
                      {config.enableLineBridge ? 'Active' : 'Disabled'}
                    </span>
                  </span>
                  <span className="bridge-subtitle">HTTP Webhook Listener (Callback)</span>
                </div>
              </div>
              <label className="settings-toggle-switch">
                <input
                  type="checkbox"
                  checked={config.enableLineBridge}
                  onChange={(e) => updateField('enableLineBridge', e.target.checked)}
                />
                <span className="settings-toggle-slider"></span>
              </label>
            </div>
            <div className="bridge-inputs">
              <ApiKeyInput
                className="bridge-input"
                placeholder="LINE Channel Access Token"
                value={config.lineChannelAccessToken}
                onChange={(value) => updateField('lineChannelAccessToken', value)}
              />
              <ApiKeyInput
                className="bridge-input"
                placeholder="LINE Channel Secret"
                value={config.lineChannelSecret}
                onChange={(value) => updateField('lineChannelSecret', value)}
              />
              <div className="bridge-input-row">
                <input
                  type="text"
                  className="bridge-input"
                  placeholder="Webhook Host (default: 127.0.0.1)"
                  value={config.lineWebhookHost || ''}
                  onChange={(e) => updateField('lineWebhookHost', e.target.value)}
                />
                <input
                  type="number"
                  className="bridge-input"
                  placeholder="Port"
                  value={config.lineWebhookPort || 3000}
                  onChange={(e) => updateField('lineWebhookPort', parseInt(e.target.value) || 3000)}
                  style={{ width: '120px' }}
                />
              </div>
            </div>
          </div>

          {/* WhatsApp Cloud API Card */}
          <div className={`bridge-card ${config.enableWhatsappBridge ? 'active' : ''}`}>
            <div className="bridge-card-header">
              <div className="bridge-info">
                <div className="bridge-icon-badge bridge-icon-whatsapp">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M.057 24l1.687-6.163c-1.041-1.804-1.588-3.849-1.587-5.946.003-6.556 5.338-11.891 11.893-11.891 3.181.001 6.167 1.24 8.413 3.488 2.245 2.248 3.481 5.236 3.48 8.414-.003 6.557-5.338 11.892-11.893 11.892-1.99-.001-3.951-.5-5.688-1.448l-6.305 1.654zm6.597-3.807c1.676.995 3.276 1.591 5.392 1.592 5.448 0 9.886-4.434 9.889-9.885.002-5.462-4.415-9.89-9.881-9.892-5.452 0-9.887 4.434-9.889 9.884-.001 2.225.651 3.891 1.746 5.634l-.999 3.648 3.742-.981zm11.387-5.464c-.301-.15-1.785-.881-2.062-.982-.276-.101-.477-.15-.677.15-.199.301-.776.982-.952 1.182-.175.201-.351.226-.652.076-.301-.15-1.27-.468-2.42-1.494-.897-.8-1.502-1.788-1.678-2.09-.175-.301-.019-.464.131-.613.136-.135.301-.351.451-.526.15-.176.2-.301.3-.502.1-.201.05-.376-.025-.526-.075-.15-.677-1.631-.928-2.235-.244-.587-.492-.507-.676-.517-.175-.008-.376-.01-.576-.01-.201 0-.526.075-.802.376-.276.301-1.053 1.029-1.053 2.509 0 1.48 1.078 2.909 1.229 3.11.15.201 2.122 3.241 5.141 4.544.718.31 1.279.495 1.716.634.721.229 1.377.197 1.896.12.578-.086 1.785-.729 2.036-1.431.25-.702.25-1.304.175-1.431-.075-.127-.276-.201-.577-.352z"/>
                  </svg>
                </div>
                <div className="bridge-title-group">
                  <span className="bridge-title">
                    WhatsApp Cloud API
                    <span className={`bridge-status-pill ${config.enableWhatsappBridge ? 'active' : 'inactive'}`}>
                      {config.enableWhatsappBridge ? 'Active' : 'Disabled'}
                    </span>
                  </span>
                  <span className="bridge-subtitle">Meta Cloud API Webhook Listener</span>
                </div>
              </div>
              <label className="settings-toggle-switch">
                <input
                  type="checkbox"
                  checked={config.enableWhatsappBridge}
                  onChange={(e) => updateField('enableWhatsappBridge', e.target.checked)}
                />
                <span className="settings-toggle-slider"></span>
              </label>
            </div>
            <div className="bridge-inputs">
              <ApiKeyInput
                className="bridge-input"
                placeholder="WhatsApp Cloud Access Token"
                value={config.whatsappCloudAccessToken}
                onChange={(value) => updateField('whatsappCloudAccessToken', value)}
              />
              <input
                type="text"
                className="bridge-input"
                placeholder="WhatsApp Phone Number ID"
                value={config.whatsappPhoneNumberId}
                onChange={(e) => updateField('whatsappPhoneNumberId', e.target.value)}
              />
              <ApiKeyInput
                className="bridge-input"
                placeholder="WhatsApp Verify Token"
                value={config.whatsappVerifyToken}
                onChange={(value) => updateField('whatsappVerifyToken', value)}
              />
              <ApiKeyInput
                className="bridge-input"
                placeholder="WhatsApp App Secret (optional HMAC validation)"
                value={config.whatsappAppSecret}
                onChange={(value) => updateField('whatsappAppSecret', value)}
              />
              <div className="bridge-input-row">
                <input
                  type="text"
                  className="bridge-input"
                  placeholder="Webhook Host (default: 127.0.0.1)"
                  value={config.whatsappWebhookHost || ''}
                  onChange={(e) => updateField('whatsappWebhookHost', e.target.value)}
                />
                <input
                  type="number"
                  className="bridge-input"
                  placeholder="Port"
                  value={config.whatsappWebhookPort || 3001}
                  onChange={(e) => updateField('whatsappWebhookPort', parseInt(e.target.value) || 3001)}
                  style={{ width: '120px' }}
                />
              </div>
            </div>
          </div>
        </div>
      </section>

    </div>
  )
}
