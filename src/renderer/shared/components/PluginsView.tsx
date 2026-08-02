import React, { useState, useEffect } from 'react'
import '../css/management-views.css'
import { renderMcpSvgIcon, BUILTIN_PLUGINS_LIST, renderPluginsSvgIcon } from '../constants/plugins'
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
  const [expandedPlugin, setExpandedPlugin] = useState<string | null>(null)
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

  const handleConnectOAuth = async (provider: string, pluginKey?: string) => {
    if (provider === 'spotify' && !(config as any).spotifyClientId?.trim()) {
      setExpandedPlugin(pluginKey || 'spotify')
      alert('Please click "Configure" below and enter your Spotify Client ID before signing in (Get it from developer.spotify.com/dashboard)')
      return
    }
    if (provider === 'github' && !(config as any).githubClientId?.trim() && !(config as any).githubToken?.trim()) {
      setExpandedPlugin(pluginKey || 'github')
      alert('Please click "Configure" below and enter your GitHub Client ID (or Personal Access Token) before signing in.')
      return
    }
    if (provider === 'vercel' && !(config as any).vercelClientId?.trim() && !(config as any).vercelToken?.trim()) {
      setExpandedPlugin(pluginKey || 'vercel')
      alert('Please click "Configure" below and enter your Vercel Client ID (or Access Token) before signing in.')
      return
    }
    if (
      (provider === 'google' || provider === 'gmail' || provider === 'google_calendar') &&
      !(config as any).gmailClientId?.trim() &&
      !(config as any).googleCalendarClientId?.trim()
    ) {
      setExpandedPlugin(pluginKey || 'gmail')
      alert('Please click "Configure" below and enter your Google Client ID before signing in.')
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

  const toggleExpand = (key: string) => {
    setExpandedPlugin((prev) => (prev === key ? null : key))
  }

  const pluginsList = BUILTIN_PLUGINS_LIST

  const filteredPlugins = pluginsList.filter(
    (p) =>
      p.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.desc.toLowerCase().includes(searchQuery.toLowerCase())
  )

  return (
    <div className="management-container">
      {/* Top Header */}
      <div className="management-header">
        <div className="management-title-group">
          <h1 className="management-title">
            <span className="management-title-icon" style={{ display: 'inline-flex', alignItems: 'center' }}>
              {renderPluginsSvgIcon(22, 'var(--accent)')}
            </span>
            Plugins & Integrations
          </h1>
          <p className="management-subtitle">
            Configure native credentials, OAuth logins, and external service connectors for Mint Agent.
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

      {/* Plugin Grid */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
        {filteredPlugins.map((p) => {
          const isEnabled = Boolean((config as any)[p.enabledField])
          const isExpanded = expandedPlugin === p.key

          return (
            <div
              key={p.key}
              style={{
                border: '1px solid rgba(255, 255, 255, 0.08)',
                borderRadius: '12px',
                background: 'rgba(255, 255, 255, 0.03)',
                overflow: 'hidden',
                transition: 'all 0.15s ease',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justify: 'space-between',
                  padding: '16px 20px',
                  background: isEnabled ? 'rgba(16, 185, 129, 0.04)' : 'transparent',
                  flexWrap: 'wrap',
                  gap: '12px',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '14px', flex: 1, minWidth: 0 }}>
                  <div style={{ width: '42px', height: '42px', borderRadius: '10px', background: 'rgba(255, 255, 255, 0.06)', display: 'grid', placeItems: 'center', flexShrink: 0 }}>
                    {renderMcpSvgIcon(p.key)}
                  </div>
                  <div style={{ minWidth: 0 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                      <span style={{ fontWeight: 700, fontSize: '0.95rem', color: '#f8fafc' }}>
                        {p.name}
                      </span>
                      {p.isOAuth && (() => {
                        const oauthMatch = oauthStatuses.find((s) => s.provider === p.oauthProvider)
                        const isConn = oauthMatch?.connected
                        return (
                          <span
                            style={{
                              fontSize: '0.72rem',
                              padding: '2px 8px',
                              borderRadius: '6px',
                              background: isConn ? 'rgba(16, 185, 129, 0.15)' : 'rgba(255, 255, 255, 0.05)',
                              color: isConn ? '#10b981' : '#94a3b8',
                              border: `1px solid ${isConn ? 'rgba(16, 185, 129, 0.3)' : 'rgba(255, 255, 255, 0.1)'}`,
                              fontWeight: 600,
                              whiteSpace: 'nowrap',
                              display: 'inline-flex',
                              alignItems: 'center',
                              gap: '5px',
                            }}
                          >
                            <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: isConn ? '#10b981' : '#94a3b8' }} />
                            {isConn ? `Connected ${oauthMatch.accountEmail ? `(${oauthMatch.accountEmail})` : ''}` : 'Not Connected'}
                          </span>
                        )
                      })()}
                    </div>
                    <div style={{ fontSize: '0.82rem', color: '#94a3b8', marginTop: '3px' }}>
                      {p.desc}
                    </div>
                  </div>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexShrink: 0, marginLeft: 'auto' }}>
                  {p.isOAuth && (() => {
                    const oauthMatch = oauthStatuses.find((s) => s.provider === p.oauthProvider)
                    const isConn = oauthMatch?.connected
                    return isConn ? (
                      <button
                        type="button"
                        onClick={() => handleRevokeOAuth(p.oauthProvider!)}
                        style={{
                          padding: '6px 12px',
                          fontSize: '0.8rem',
                          borderRadius: '8px',
                          border: '1px solid rgba(239, 68, 68, 0.3)',
                          background: 'rgba(239, 68, 68, 0.1)',
                          color: '#ef4444',
                          fontWeight: 600,
                          cursor: 'pointer',
                        }}
                      >
                        Disconnect
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => handleConnectOAuth(p.oauthProvider!, p.key)}
                        disabled={authenticatingProvider === p.oauthProvider}
                        style={{
                          padding: '6px 14px',
                          fontSize: '0.8rem',
                          borderRadius: '8px',
                          border: 'none',
                          background: '#10b981',
                          color: '#ffffff',
                          fontWeight: 600,
                          cursor: 'pointer',
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: '6px',
                        }}
                      >
                        {authenticatingProvider === p.oauthProvider ? 'Signing In...' : 'Sign In'}
                      </button>
                    )
                  })()}

                  {p.hasCredentials && (
                    <button
                      type="button"
                      className="management-action-btn"
                      onClick={() => toggleExpand(p.key)}
                    >
                      Configure
                    </button>
                  )}

                  <label className="settings-toggle-switch" title={isEnabled ? 'Disable plugin' : 'Enable plugin'}>
                    <input
                      type="checkbox"
                      checked={isEnabled}
                      onChange={(e) => {
                        updateField(p.enabledField as any, e.target.checked)
                      }}
                    />
                    <span className="settings-toggle-slider" />
                  </label>
                </div>
              </div>

              {p.hasCredentials && isExpanded && (
                <div
                  style={{
                    padding: '20px',
                    borderTop: '1px solid rgba(255, 255, 255, 0.08)',
                    background: 'rgba(0, 0, 0, 0.2)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '12px',
                  }}
                >
                  <h4 style={{ fontSize: '0.85rem', fontWeight: 600, color: '#10b981', textTransform: 'uppercase', letterSpacing: '0.5px', margin: 0 }}>
                    Credentials Configuration
                  </h4>
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr', gap: '12px' }}>
                    {p.fields?.map((f) => (
                      <div key={f.field}>
                        <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>{f.label}</label>
                        <input
                          type={f.type}
                          placeholder={f.placeholder}
                          value={(config as any)[f.field] || ''}
                          onChange={(e) => updateField(f.field as any, e.target.value)}
                          style={{
                            width: '100%',
                            padding: '10px 14px',
                            borderRadius: '8px',
                            border: '1px solid rgba(255, 255, 255, 0.1)',
                            background: 'rgba(255, 255, 255, 0.04)',
                            color: '#f8fafc',
                            fontSize: '0.88rem',
                            boxSizing: 'border-box',
                          }}
                        />
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
})

export default PluginsView
