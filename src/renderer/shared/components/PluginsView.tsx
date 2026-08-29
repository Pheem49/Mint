import React, { useState, useEffect } from 'react'
import '../css/management-views.css'
import { renderMcpSvgIcon, BUILTIN_PLUGINS_LIST, renderPluginsSvgIcon, type BuiltinPluginDefinition } from '../constants/plugins'
import {
  getOAuthStatuses,
  startOAuthFlow,
  revokeOAuth,
  subscribeOAuthChange,
  type OAuthStatus,
} from '../utils/oauthManager'

export interface PluginsViewProps {
  config: any
  updateField: (field: string, value: any) => void
  handleConnectPlugin: (plugin: string) => void
}

export const PluginsView: React.FC<PluginsViewProps> = React.memo(function PluginsView({
  config,
  updateField,
  handleConnectPlugin,
}) {
  const [oauthStatuses, setOauthStatuses] = useState<OAuthStatus[]>([])
  const [authenticatingProvider, setAuthenticatingProvider] = useState<string | null>(null)
  const [detailPlugin, setDetailPlugin] = useState<BuiltinPluginDefinition | null>(null)
  const [searchQuery, setSearchQuery] = useState('')

  const fetchOAuthStatuses = async () => {
    try {
      const statuses = await getOAuthStatuses()
      setOauthStatuses(statuses)
    } catch (err) {
      console.error('Failed to fetch OAuth statuses:', err)
    }
  }

  useEffect(() => {
    fetchOAuthStatuses()
    const unsubscribe = subscribeOAuthChange(() => {
      fetchOAuthStatuses()
    })
    return () => {
      unsubscribe()
    }
  }, [])

  const pluginsList = BUILTIN_PLUGINS_LIST

  const openForConfig = (pluginKey: string) => {
    const plugin = pluginsList.find((item) => item.key === pluginKey)
    if (plugin) setDetailPlugin(plugin)
  }

  const handleConnectOAuth = async (provider: string, pluginKey?: string) => {
    if (provider === 'spotify' && !(config as any).spotifyClientId?.trim()) {
      openForConfig(pluginKey || 'spotify')
      alert('Please enter your Spotify Client ID below before signing in (get it from developer.spotify.com/dashboard)')
      return
    }
    if (provider === 'github' && !(config as any).githubClientId?.trim() && !(config as any).githubToken?.trim()) {
      openForConfig(pluginKey || 'github')
      alert('Please enter your GitHub Client ID (or Personal Access Token) below before signing in.')
      return
    }
    if (provider === 'vercel' && !(config as any).vercelClientId?.trim() && !(config as any).vercelToken?.trim()) {
      openForConfig(pluginKey || 'vercel')
      alert('Please enter your Vercel Client ID (or Access Token) below before signing in.')
      return
    }
    if (
      (provider === 'google' || provider === 'gmail' || provider === 'google_calendar') &&
      !(config as any).gmailClientId?.trim() &&
      !(config as any).googleCalendarClientId?.trim()
    ) {
      openForConfig(pluginKey || 'gmail')
      alert('Please enter your Google Client ID below before signing in.')
      return
    }

    setAuthenticatingProvider(provider)
    try {
      await startOAuthFlow(provider)
    } catch (err) {
      console.error(`OAuth login error for ${provider}:`, err)
    } finally {
      setAuthenticatingProvider(null)
      fetchOAuthStatuses()
    }
  }

  const handleRevokeOAuth = async (provider: string) => {
    try {
      await revokeOAuth(provider)
      await fetchOAuthStatuses()
    } catch (err) {
      console.error(`OAuth revoke error for ${provider}:`, err)
    }
  }

  const filteredPlugins = pluginsList.filter(
    (p) =>
      p.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.desc.toLowerCase().includes(searchQuery.toLowerCase())
  )

  const installedPlugins = pluginsList.filter((p) => Boolean((config as any)[p.enabledField]))

  return (
    <div className="management-container">
      {/* Top Header */}
      <div className="management-header">
        <div className="management-title-group">
          <h1 className="management-title">
            <span className="management-title-icon" style={{ display: 'inline-flex', alignItems: 'center' }}>
              {renderPluginsSvgIcon(22, 'var(--accent)')}
            </span>
            Plugins
          </h1>
          <p className="management-subtitle">
            Connect external services and accounts.
          </p>
        </div>
      </div>

      {/* Search Input */}
      <div className="management-control-bar">
        <div className="management-search-wrapper" style={{ maxWidth: '400px' }}>
          <input
            type="text"
            className="management-search-input"
            placeholder="Search plugins..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          <svg
            className="management-search-icon"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
        </div>
      </div>

      {/* Installed */}
      {installedPlugins.length > 0 && (
        <div className="management-installed-section">
          <h2 className="management-section-title">Installed</h2>
          <div className="management-installed-row">
            {installedPlugins.map((p) => (
              <button
                key={p.key}
                type="button"
                className="management-plugin-avatar"
                title={p.name}
                onClick={() => setDetailPlugin(p)}
              >
                {renderMcpSvgIcon(p.key)}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Plugin Grid */}
      <h2 className="management-section-title">Recommended</h2>
      <div className="mgmt-row-stack">
        {filteredPlugins.map((p) => {
          const isEnabled = Boolean((config as any)[p.enabledField])
          const oauthMatch = p.isOAuth ? oauthStatuses.find((s) => s.provider === p.oauthProvider) : undefined
          const isConn = Boolean(oauthMatch?.connected)

          return (
            <div
              key={p.key}
              className="management-plugin-row"
              onClick={() => setDetailPlugin(p)}
            >
              <div className="management-card-icon" style={{ background: 'rgba(255, 255, 255, 0.06)', borderColor: 'rgba(255, 255, 255, 0.08)' }}>
                {renderMcpSvgIcon(p.key)}
              </div>
              <div className="management-plugin-info">
                <div className="management-plugin-name">
                  {p.name}
                  {p.isOAuth && <span className={`management-dot ${isConn ? 'connected' : ''}`} title={isConn ? 'Connected' : 'Not connected'} />}
                </div>
                <div className="management-plugin-desc">{p.desc}</div>
              </div>

              {isEnabled ? (
                <button
                  type="button"
                  className="management-plugin-icon-btn"
                  title="View details"
                  onClick={(e) => {
                    e.stopPropagation()
                    setDetailPlugin(p)
                  }}
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                    <circle cx="5" cy="12" r="2" />
                    <circle cx="12" cy="12" r="2" />
                    <circle cx="19" cy="12" r="2" />
                  </svg>
                </button>
              ) : (
                <button
                  type="button"
                  className="management-plugin-icon-btn"
                  title="Enable"
                  onClick={(e) => {
                    e.stopPropagation()
                    updateField(p.enabledField as any, true)
                    setDetailPlugin(p)
                  }}
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round">
                    <line x1="12" y1="5" x2="12" y2="19" />
                    <line x1="5" y1="12" x2="19" y2="12" />
                  </svg>
                </button>
              )}
            </div>
          )
        })}
      </div>

      {/* Plugin Detail */}
      {detailPlugin && (() => {
        const p = detailPlugin
        const isEnabled = Boolean((config as any)[p.enabledField])
        const oauthMatch = p.isOAuth ? oauthStatuses.find((s) => s.provider === p.oauthProvider) : undefined
        const isConn = Boolean(oauthMatch?.connected)

        return (
          <div className="management-modal-overlay" onClick={() => setDetailPlugin(null)}>
            <div className="management-modal" onClick={(e) => e.stopPropagation()}>
              <div className="management-modal-header">
                <div className="management-card-title-group">
                  <div className="management-card-icon" style={{ width: 44, height: 44, background: 'rgba(255, 255, 255, 0.06)', borderColor: 'rgba(255, 255, 255, 0.08)' }}>
                    {renderMcpSvgIcon(p.key)}
                  </div>
                  <h2 className="management-modal-title">{p.name}</h2>
                </div>
                <button type="button" className="management-modal-close" onClick={() => setDetailPlugin(null)}>
                  ✕
                </button>
              </div>

              <div className="management-modal-body">
                <p style={{ color: 'var(--text-soft, #d1d1d4)', lineHeight: 1.55 }}>{p.desc}</p>

                <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginTop: '16px', flexWrap: 'wrap' }}>
                  <label className="settings-toggle-switch" title={isEnabled ? 'Disable plugin' : 'Enable plugin'}>
                    <input
                      type="checkbox"
                      checked={isEnabled}
                      onChange={(e) => updateField(p.enabledField as any, e.target.checked)}
                    />
                    <span className="settings-toggle-slider" />
                  </label>
                  <span style={{ fontSize: '0.85rem', color: 'var(--text-muted, #94a3b8)' }}>
                    {isEnabled ? 'Enabled' : 'Disabled'}
                  </span>

                  {p.isOAuth &&
                    (isConn ? (
                      <button
                        type="button"
                        className="management-action-btn danger"
                        style={{ marginLeft: 'auto' }}
                        onClick={() => handleRevokeOAuth(p.oauthProvider!)}
                      >
                        Disconnect{oauthMatch?.accountEmail ? ` (${oauthMatch.accountEmail})` : ''}
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="management-primary-btn"
                        style={{ marginLeft: 'auto' }}
                        onClick={() => handleConnectOAuth(p.oauthProvider!, p.key)}
                        disabled={authenticatingProvider === p.oauthProvider}
                      >
                        {authenticatingProvider === p.oauthProvider ? 'Signing In...' : 'Sign In'}
                      </button>
                    ))}
                </div>

                {p.hasCredentials && p.fields && (
                  <div style={{ marginTop: '20px', paddingTop: '16px', borderTop: '1px solid var(--border, rgba(255, 255, 255, 0.08))' }}>
                    <h4 style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--accent, #10b981)', textTransform: 'uppercase', letterSpacing: '0.5px', margin: '0 0 12px' }}>
                      Credentials
                    </h4>
                    <div style={{ display: 'grid', gap: '12px' }}>
                      {p.fields.map((f) => (
                        <div key={f.field} className="management-form-group">
                          <label className="management-label">{f.label}</label>
                          <input
                            type={f.type}
                            className="management-input-field"
                            placeholder={f.placeholder}
                            value={(config as any)[f.field] || ''}
                            onChange={(e) => updateField(f.field as any, e.target.value)}
                          />
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        )
      })()}
    </div>
  )
})

export default PluginsView
