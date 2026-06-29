import React, { useState } from 'react'
import { DEFAULT_CONFIG } from '../../components/SettingsWindow'

interface PluginsTabProps {
  config: typeof DEFAULT_CONFIG
  updateField: (field: keyof typeof DEFAULT_CONFIG, value: any) => void
  mcpName: string
  setMcpName: (val: string) => void
  mcpCmd: string
  setMcpCmd: (val: string) => void
  mcpArgs: string
  setMcpArgs: (val: string) => void
  mcpEnv: string
  setMcpEnv: (val: string) => void
  handleAddMcpServer: () => void
  handleRemoveMcpServer: (name: string) => void
  handleConnectPlugin: (plugin: string) => void
}

export default function PluginsTab({
  config,
  updateField,
  mcpName,
  setMcpName,
  mcpCmd,
  setMcpCmd,
  mcpArgs,
  setMcpArgs,
  mcpEnv,
  setMcpEnv,
  handleAddMcpServer,
  handleRemoveMcpServer,
  handleConnectPlugin
}: PluginsTabProps) {
  const [expandedPlugin, setExpandedPlugin] = useState<string | null>(null)

  const toggleExpand = (pluginKey: string) => {
    setExpandedPlugin(expandedPlugin === pluginKey ? null : pluginKey)
  }

  const pluginsList = [
    {
      key: 'spotify',
      name: 'Spotify',
      desc: 'Control playback with AI. Requires playerctl locally.',
      icon: '🎵',
      enabledField: 'pluginSpotifyEnabled' as const,
      hasCredentials: false
    },
    {
      key: 'calendar',
      name: 'Google Calendar',
      desc: 'Read and schedule events via Google Calendar API.',
      icon: '📅',
      enabledField: 'pluginCalendarEnabled' as const,
      hasCredentials: true,
      fields: [
        { label: 'Google Client ID', field: 'googleCalendarClientId' as const, type: 'text', placeholder: 'Enter Google Client ID...' },
        { label: 'Google Client Secret', field: 'googleCalendarClientSecret' as const, type: 'password', placeholder: 'Enter Google Client Secret...' },
        { label: 'Google Refresh Token', field: 'googleCalendarRefreshToken' as const, type: 'password', placeholder: 'Enter Google Refresh Token...' }
      ]
    },
    {
      key: 'gmail',
      name: 'Gmail',
      desc: 'Read email and create drafts safely via Gmail API.',
      icon: '✉',
      enabledField: 'pluginGmailEnabled' as const,
      hasCredentials: true,
      fields: [
        { label: 'Gmail Client ID', field: 'gmailClientId' as const, type: 'text', placeholder: 'Enter Gmail Client ID...' },
        { label: 'Gmail Client Secret', field: 'gmailClientSecret' as const, type: 'password', placeholder: 'Enter Gmail Client Secret...' },
        { label: 'Gmail Refresh Token', field: 'gmailRefreshToken' as const, type: 'password', placeholder: 'Enter Gmail Refresh Token...' }
      ]
    },
    {
      key: 'notion',
      name: 'Notion',
      desc: 'Create notes, pages, and read databases via Notion API.',
      icon: '📝',
      enabledField: 'pluginNotionEnabled' as const,
      hasCredentials: true,
      fields: [
        { label: 'Notion API Key', field: 'notionApiKey' as const, type: 'password', placeholder: 'secret_...' },
        { label: 'Notion Database ID', field: 'notionDatabaseId' as const, type: 'text', placeholder: 'Enter Notion Database ID...' }
      ]
    },
    {
      key: 'discord',
      name: 'Discord RPC',
      desc: 'Show "Using Mint Assistant" status in your local Discord client.',
      icon: '💬',
      enabledField: 'pluginDiscordEnabled' as const,
      hasCredentials: false
    }
  ]

  return (
    <div className="tab-pane active">
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Integrations</p>
            <h2 className="section-title">Built-in Plugins</h2>
          </div>
          <p className="section-description">Enable and configure credentials for native Mint plugins.</p>
        </div>
        <div className="plugin-list" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          {pluginsList.map(p => {
            const isEnabled = config[p.enabledField]
            const isExpanded = expandedPlugin === p.key

            return (
              <div className={`plugin-card-wrapper ${isEnabled ? 'active-plugin-card' : ''}`} key={p.key} style={{
                border: '1px solid var(--border)',
                borderRadius: '12px',
                background: 'var(--surface-bg)',
                transition: 'all 0.2s ease',
                overflow: 'hidden'
              }}>
                <div className="plugin-card" style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '16px 20px',
                  background: isEnabled ? 'color-mix(in srgb, var(--accent) 3%, var(--surface-bg))' : 'transparent'
                }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                    <div className="plugin-icon" style={{ fontSize: '1.5rem', width: '40px', height: '40px', display: 'grid', placeItems: 'center', background: 'var(--surface-strong)', borderRadius: '10px' }}>
                      {p.icon}
                    </div>
                    <div className="plugin-info">
                      <div className="plugin-name" style={{ fontWeight: '600', color: 'var(--text-main)' }}>{p.name}</div>
                      <div className="plugin-desc" style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>{p.desc}</div>
                    </div>
                  </div>
                  <div className="plugin-actions" style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    {p.hasCredentials && isEnabled && (
                      <button
                        className="btn-secondary"
                        onClick={() => toggleExpand(p.key)}
                        style={{ padding: '6px 12px', fontSize: '0.8rem', height: '34px' }}
                      >
                        {isExpanded ? 'Hide Config' : 'Configure'}
                      </button>
                    )}
                    {p.key === 'discord' && isEnabled && (
                      <button
                        className="btn-secondary"
                        onClick={() => handleConnectPlugin('discord')}
                        style={{ padding: '6px 12px', fontSize: '0.8rem', height: '34px' }}
                      >
                        Update RPC
                      </button>
                    )}
                    <label className="settings-toggle-switch">
                      <input
                        type="checkbox"
                        checked={isEnabled}
                        onChange={(e) => {
                          updateField(p.enabledField, e.target.checked)
                          if (!e.target.checked && expandedPlugin === p.key) {
                            setExpandedPlugin(null)
                          }
                        }}
                      />
                      <span className="settings-toggle-slider" />
                    </label>
                  </div>
                </div>

                {p.hasCredentials && isEnabled && isExpanded && (
                  <div className="plugin-config-panel" style={{
                    padding: '20px',
                    borderTop: '1px solid var(--border)',
                    background: 'rgba(0,0,0,0.12)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '12px'
                  }}>
                    <h4 style={{ fontSize: '0.9rem', fontWeight: '600', color: 'var(--text-soft)', marginBottom: '4px' }}>Credentials Configuration</h4>
                    <div className="form-grid compact" style={{ display: 'grid', gridTemplateColumns: '1fr', gap: '12px' }}>
                      {p.fields?.map(f => (
                        <div className="setting-row" key={f.field} style={{ display: 'flex', flexDirection: 'column', gap: '6px', border: 'none', padding: 0 }}>
                          <label style={{ fontSize: '0.8rem', fontWeight: '500', color: 'var(--text-muted)' }}>{f.label}</label>
                          <input
                            type={f.type}
                            placeholder={f.placeholder}
                            value={(config as any)[f.field] || ''}
                            onChange={(e) => updateField(f.field, e.target.value)}
                            style={{
                              width: '100%',
                              padding: '10px 14px',
                              borderRadius: '8px',
                              border: '1px solid var(--border)',
                              background: 'var(--input-bg)',
                              color: 'var(--text-main)',
                              fontSize: '0.9rem'
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
      </section>

      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">External tools</p>
            <h2 className="section-title">MCP Servers</h2>
          </div>
          <p className="section-description">Connect Mint to tools like search, GitHub, or filesystem servers.</p>
        </div>

        <div className="mcp-list">
          {Object.entries(config.mcpServers || {}).map(([name, srv]: [string, any]) => (
            <div className="plugin-card" key={name} style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '16px 20px',
              border: '1px solid var(--border)',
              borderRadius: '12px',
              background: 'var(--surface-bg)',
              marginBottom: '10px'
            }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                <div className="plugin-icon" style={{ fontSize: '1.5rem', width: '40px', height: '40px', display: 'grid', placeItems: 'center', background: 'var(--surface-strong)', borderRadius: '10px' }}>⚙</div>
                <div className="plugin-info">
                  <div className="plugin-name" style={{ fontWeight: '600', color: 'var(--text-main)' }}>{name}</div>
                  <div className="plugin-desc" style={{ fontFamily: 'monospace', fontSize: '0.8rem', color: 'var(--text-muted)' }}>
                    Command: {srv.command} {srv.args?.join(' ')}
                  </div>
                </div>
              </div>
              <div className="plugin-actions">
                <button className="btn btn-danger" onClick={() => handleRemoveMcpServer(name)} style={{ padding: '6px 12px', fontSize: '0.8rem', height: '34px' }}>Remove</button>
              </div>
            </div>
          ))}
        </div>

        <div className="add-mcp-box" style={{
          marginTop: '20px',
          padding: '20px',
          border: '1px solid var(--border)',
          borderRadius: '12px',
          background: 'var(--surface-bg)'
        }}>
          <h3 style={{ fontSize: '1rem', fontWeight: '600', color: 'var(--text-main)', marginBottom: '14px' }}>Add MCP Server</h3>
          <div className="form-grid" style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px', marginBottom: '12px' }}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Server Name</label>
              <input
                type="text"
                placeholder="e.g. google-search"
                value={mcpName}
                onChange={(e) => setMcpName(e.target.value)}
                style={{ padding: '10px 14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.9rem' }}
              />
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Command</label>
              <input
                type="text"
                placeholder="e.g. npx"
                value={mcpCmd}
                onChange={(e) => setMcpCmd(e.target.value)}
                style={{ padding: '10px 14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.9rem' }}
              />
            </div>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '12px' }}>
            <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Arguments</label>
            <input
              type="text"
              placeholder="e.g. -y @modelcontextprotocol/server-brave-search"
              value={mcpArgs}
              onChange={(e) => setMcpArgs(e.target.value)}
              style={{ padding: '10px 14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.9rem', width: '100%' }}
            />
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '14px' }}>
            <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Environment Variables (JSON)</label>
            <textarea
              placeholder='e.g. {"BRAVE_API_KEY": "your_key_here"}'
              value={mcpEnv}
              onChange={(e) => setMcpEnv(e.target.value)}
              style={{ padding: '10px 14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.9rem', width: '100%', height: '70px', resize: 'vertical' }}
            />
          </div>
          <button className="btn-primary" onClick={handleAddMcpServer} style={{ width: '100%', padding: '10px 16px', borderRadius: '8px', border: 'none', background: 'var(--accent)', color: '#fff', fontWeight: '600', cursor: 'pointer' }}>Add MCP Server</button>
        </div>
      </section>
    </div>
  )
}
