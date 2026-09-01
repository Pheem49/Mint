import React, { useState, useEffect } from 'react'
import '../css/management-views.css'
import { renderMcpSvgIcon, renderMcpHubSvgIcon } from '../constants/plugins'
import McpToolAllowlist from './McpToolAllowlist'

export interface McpServersViewProps {
  config: any
  updateField: (field: string, value: any) => void
  mcpName: string
  setMcpName: (val: string) => void
  mcpCmd: string
  setMcpCmd: (val: string) => void
  mcpArgs: string
  setMcpArgs: (val: string) => void
  mcpEnv: string
  setMcpEnv: (val: string) => void
  mcpIcon?: string
  setMcpIcon?: (val: string) => void
  handleAddMcpServer: () => void
  handleRemoveMcpServer: (name: string) => void
  detectTools?: () => Promise<{ docker: boolean; git: boolean; gh: boolean; node: boolean }>
  onReauth?: (name: string) => Promise<boolean>
  listServerTools?: (name: string) => Promise<string[]>
}

export const McpServersView: React.FC<McpServersViewProps> = React.memo(function McpServersView({
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
  mcpIcon = '',
  setMcpIcon,
  handleAddMcpServer,
  handleRemoveMcpServer,
  detectTools,
  onReauth,
  listServerTools,
}) {
  const [detectedTools, setDetectedTools] = useState({ docker: false, git: false, gh: false, node: false })
  const [detailMcpName, setDetailMcpName] = useState<string | null>(null)
  const [showAddModal, setShowAddModal] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [reauthStatus, setReauthStatus] = useState<Record<string, 'idle' | 'running' | 'success' | 'error'>>({})

  useEffect(() => {
    if (detectTools) {
      detectTools().then((t) => t && setDetectedTools(t)).catch(() => {})
    }
  }, [detectTools])

  const handleToggleMcpServer = (name: string, enabled: boolean, defaultCmd?: string, defaultArgs?: string[]) => {
    const updated = { ...(config.mcpServers || {}) }
    if (updated[name]) {
      updated[name] = { ...updated[name], disabled: !enabled }
    } else if (enabled && defaultCmd) {
      updated[name] = { command: defaultCmd, args: defaultArgs || [], env: {}, disabled: false }
    }
    updateField('mcpServers', updated)
  }

  const handleUpdateMcpServerField = (name: string, field: string, value: any) => {
    const updated = { ...(config.mcpServers || {}) }
    if (updated[name]) {
      updated[name] = { ...updated[name], [field]: value }
      updateField('mcpServers', updated)
    }
  }

  const mcpListItems: Array<{
    name: string
    command: string
    args: string[]
    icon: string
    customIcon?: string
    isEnabled: boolean
    isConfigured: boolean
    description?: string
  }> = []

  Object.entries(config.mcpServers || {}).forEach(([name, srv]: [string, any]) => {
    let icon = '🔌'
    if (name === 'docker') icon = '🐳'
    if (name === 'git' || name === 'github') icon = '🐙'

    mcpListItems.push({
      name,
      command: srv.command,
      args: srv.args || [],
      icon,
      customIcon: srv.icon,
      isEnabled: srv?.disabled !== true,
      isConfigured: true,
      description: `Command: ${srv.command} ${(srv.args || []).join(' ')}`,
    })
  })

  if (detectedTools.docker && !config.mcpServers?.docker) {
    mcpListItems.push({
      name: 'docker',
      command: 'npx',
      args: ['-y', '@proxeus/mcp-docker-server'],
      icon: '🐳',
      isEnabled: false,
      isConfigured: false,
      description: 'Docker MCP Server (Auto Discovered)',
    })
  }

  if (detectedTools.git && !config.mcpServers?.git) {
    mcpListItems.push({
      name: 'git',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-git'],
      icon: '🐙',
      isEnabled: false,
      isConfigured: false,
      description: 'Git MCP Server (Auto Discovered)',
    })
  }

  const filteredMcpItems = mcpListItems.filter((item) =>
    item.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    item.command.toLowerCase().includes(searchQuery.toLowerCase()) ||
    (item.description || '').toLowerCase().includes(searchQuery.toLowerCase())
  )

  const installedMcpItems = mcpListItems.filter((item) => item.isEnabled)

  const handleReauth = async (name: string) => {
    if (!onReauth) return
    setReauthStatus((prev) => ({ ...prev, [name]: 'running' }))
    try {
      const success = await onReauth(name)
      setReauthStatus((prev) => ({ ...prev, [name]: success ? 'success' : 'error' }))
    } catch {
      setReauthStatus((prev) => ({ ...prev, [name]: 'error' }))
    } finally {
      setTimeout(() => {
        setReauthStatus((prev) => ({ ...prev, [name]: 'idle' }))
      }, 2500)
    }
  }

  const onSubmitAddServer = (e: React.FormEvent) => {
    e.preventDefault()
    handleAddMcpServer()
    setShowAddModal(false)
  }

  return (
    <div className="management-container">
      {/* Top Header */}
      <div className="management-header">
        <div className="management-title-group">
          <h1 className="management-title">
            <span className="management-title-icon" style={{ display: 'inline-flex', alignItems: 'center' }}>
              {renderMcpHubSvgIcon(22, 'var(--accent)')}
            </span>
            MCP Servers
          </h1>
          <p className="management-subtitle">
            Connect external tool servers.
          </p>
        </div>

        <button
          type="button"
          className="management-primary-btn"
          onClick={() => setShowAddModal(true)}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
          Add MCP Server
        </button>
      </div>

      {/* Search Input */}
      <div className="management-control-bar">
        <div className="management-search-wrapper" style={{ maxWidth: '400px' }}>
          <input
            type="text"
            className="management-search-input"
            placeholder="Search MCP servers..."
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
      {installedMcpItems.length > 0 && (
        <div className="management-installed-section">
          <h2 className="management-section-title">Installed</h2>
          <div className="management-installed-row">
            {installedMcpItems.map((item) => (
              <button
                key={item.name}
                type="button"
                className="management-plugin-avatar"
                title={item.name}
                onClick={() => setDetailMcpName(item.name)}
              >
                {renderMcpSvgIcon(item.name, item.customIcon)}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* MCP Server List */}
      <h2 className="management-section-title">All Servers</h2>
      {filteredMcpItems.length === 0 ? (
        <div className="mgmt-empty">
          <p>{searchQuery ? 'No MCP servers match your search.' : 'No MCP servers yet.'}</p>
          {!searchQuery && <p>Add one above, or run <code>mint mcp add</code> in a terminal.</p>}
        </div>
      ) : (
        <div className="mgmt-row-stack">
          {filteredMcpItems.map((item) => (
            <div
              key={item.name}
              className="management-plugin-row"
              onClick={() => setDetailMcpName(item.name)}
            >
              <div className="management-card-icon" style={{ background: 'rgba(255, 255, 255, 0.06)', borderColor: 'rgba(255, 255, 255, 0.08)' }}>
                {renderMcpSvgIcon(item.name, item.customIcon)}
              </div>
              <div className="management-plugin-info">
                <div className="management-plugin-name">
                  {item.name}
                  <span className={`management-dot ${item.isEnabled ? 'connected' : ''}`} title={item.isEnabled ? 'Active' : 'Inactive'} />
                </div>
                <div className="management-plugin-desc" style={{ fontFamily: 'monospace' }}>
                  {item.description}
                </div>
              </div>

              {item.isEnabled ? (
                <button
                  type="button"
                  className="management-plugin-icon-btn"
                  title="View details"
                  onClick={(e) => {
                    e.stopPropagation()
                    setDetailMcpName(item.name)
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
                    handleToggleMcpServer(item.name, true, item.command, item.args)
                    setDetailMcpName(item.name)
                  }}
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round">
                    <line x1="12" y1="5" x2="12" y2="19" />
                    <line x1="5" y1="12" x2="19" y2="12" />
                  </svg>
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {/* MCP Server Detail */}
      {detailMcpName && (() => {
        const item = mcpListItems.find((i) => i.name === detailMcpName)
        if (!item) return null
        const srvConfig = config.mcpServers?.[item.name] || { command: item.command, args: item.args, env: {}, icon: item.customIcon }

        return (
          <div className="management-modal-overlay" onClick={() => setDetailMcpName(null)}>
            <div className="management-modal" onClick={(e) => e.stopPropagation()}>
              <div className="management-modal-header">
                <div className="management-card-title-group">
                  <div className="management-card-icon" style={{ width: 44, height: 44, background: 'rgba(255, 255, 255, 0.06)', borderColor: 'rgba(255, 255, 255, 0.08)' }}>
                    {renderMcpSvgIcon(item.name, item.customIcon)}
                  </div>
                  <h2 className="management-modal-title">{item.name}</h2>
                </div>
                <button type="button" className="management-modal-close" onClick={() => setDetailMcpName(null)}>
                  ✕
                </button>
              </div>

              <div className="management-modal-body">
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
                  <label className="settings-toggle-switch" title={item.isEnabled ? 'Disable server' : 'Enable server'}>
                    <input
                      type="checkbox"
                      checked={item.isEnabled}
                      onChange={(e) => handleToggleMcpServer(item.name, e.target.checked, item.command, item.args)}
                    />
                    <span className="settings-toggle-slider" />
                  </label>
                  <span style={{ fontSize: '0.85rem', color: 'var(--text-muted, #94a3b8)' }}>
                    {item.isEnabled ? 'Enabled' : 'Disabled'}
                  </span>
                  {!item.isConfigured && (
                    <span className="management-tag workspace" style={{ marginLeft: 'auto' }}>
                      Discovered
                    </span>
                  )}
                </div>

                {item.isConfigured && (
                  <div style={{ marginTop: '20px', paddingTop: '16px', borderTop: '1px solid var(--border, rgba(255, 255, 255, 0.08))' }}>
                    <h4 style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--accent, #3b82f6)', textTransform: 'uppercase', letterSpacing: '0.5px', margin: '0 0 12px' }}>
                      Server Config
                    </h4>
                    <div style={{ display: 'grid', gap: '12px' }}>
                      <div className="management-form-group">
                        <label className="management-label">Command</label>
                        <input
                          type="text"
                          className="management-input-field"
                          value={srvConfig.command || ''}
                          onChange={(e) => handleUpdateMcpServerField(item.name, 'command', e.target.value)}
                        />
                      </div>
                      <div className="management-form-group">
                        <label className="management-label">Arguments (space-separated)</label>
                        <input
                          type="text"
                          className="management-input-field"
                          value={(srvConfig.args || []).join(' ')}
                          onChange={(e) => handleUpdateMcpServerField(item.name, 'args', e.target.value.split(/\s+/).filter(Boolean))}
                        />
                      </div>
                      <div className="management-form-group">
                        <label className="management-label">Icon (preset / URL / SVG)</label>
                        <input
                          type="text"
                          className="management-input-field"
                          placeholder="e.g. search, database, code"
                          value={srvConfig.icon || ''}
                          onChange={(e) => handleUpdateMcpServerField(item.name, 'icon', e.target.value)}
                        />
                      </div>
                      <div className="management-form-group">
                        <label className="management-label">Environment Variables (JSON)</label>
                        <textarea
                          className="management-textarea-field"
                          value={typeof srvConfig.env === 'object' ? JSON.stringify(srvConfig.env, null, 2) : srvConfig.env || ''}
                          onChange={(e) => {
                            try {
                              const parsed = JSON.parse(e.target.value)
                              handleUpdateMcpServerField(item.name, 'env', parsed)
                            } catch {
                              // allow live editing
                            }
                          }}
                          rows={3}
                        />
                      </div>
                    </div>

                    <McpToolAllowlist
                      serverName={item.name}
                      config={config}
                      updateField={updateField}
                      listServerTools={listServerTools}
                    />
                  </div>
                )}
              </div>

              <div className="management-modal-footer">
                {item.isConfigured && onReauth ? (
                  <span style={{ fontSize: '0.8rem', color: 'var(--text-muted, #94a3b8)' }}>
                    {reauthStatus[item.name] === 'success' && 'Done ✓'}
                    {reauthStatus[item.name] === 'error' && 'Re-authentication failed'}
                  </span>
                ) : (
                  <span />
                )}
                <div style={{ display: 'flex', gap: '8px' }}>
                  {item.isConfigured && onReauth && (
                    <button
                      type="button"
                      className="management-action-btn"
                      disabled={reauthStatus[item.name] === 'running'}
                      onClick={() => handleReauth(item.name)}
                    >
                      {reauthStatus[item.name] === 'running' ? 'Re-authenticating...' : 'Re-authenticate'}
                    </button>
                  )}
                  {item.isConfigured && (
                    <button
                      type="button"
                      className="management-action-btn danger"
                      onClick={() => {
                        handleRemoveMcpServer(item.name)
                        setDetailMcpName(null)
                      }}
                    >
                      Remove
                    </button>
                  )}
                </div>
              </div>
            </div>
          </div>
        )
      })()}

      {/* Add MCP Server Modal */}
      {showAddModal && (
        <div className="management-modal-overlay" onClick={() => setShowAddModal(false)}>
          <div className="management-modal" onClick={(e) => e.stopPropagation()}>
            <div className="management-modal-header">
              <h2 className="management-modal-title">Add New MCP Server</h2>
              <button type="button" className="management-modal-close" onClick={() => setShowAddModal(false)}>
                ✕
              </button>
            </div>

            <form onSubmit={onSubmitAddServer}>
              <div className="management-modal-body">
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
                  <div className="management-form-group">
                    <label className="management-label">Server Name</label>
                    <input
                      type="text"
                      className="management-input-field"
                      placeholder="e.g. google-search"
                      value={mcpName}
                      onChange={(e) => setMcpName(e.target.value)}
                      required
                    />
                  </div>
                  <div className="management-form-group">
                    <label className="management-label">Command</label>
                    <input
                      type="text"
                      className="management-input-field"
                      placeholder="e.g. npx"
                      value={mcpCmd}
                      onChange={(e) => setMcpCmd(e.target.value)}
                      required
                    />
                  </div>
                </div>

                <div className="management-form-group">
                  <label className="management-label">Arguments</label>
                  <input
                    type="text"
                    className="management-input-field"
                    placeholder="e.g. -y @modelcontextprotocol/server-brave-search"
                    value={mcpArgs}
                    onChange={(e) => setMcpArgs(e.target.value)}
                  />
                </div>

                <div className="management-form-group">
                  <label className="management-label">Icon (Optional)</label>
                  <input
                    type="text"
                    className="management-input-field"
                    placeholder="e.g. search, database, cloud, code"
                    value={mcpIcon}
                    onChange={(e) => setMcpIcon && setMcpIcon(e.target.value)}
                  />
                </div>

                <div className="management-form-group">
                  <label className="management-label">Environment Variables (JSON)</label>
                  <textarea
                    className="management-textarea-field"
                    placeholder='e.g. {"BRAVE_API_KEY": "your_key_here"}'
                    value={mcpEnv}
                    onChange={(e) => setMcpEnv(e.target.value)}
                    rows={3}
                  />
                </div>
              </div>

              <div className="management-modal-footer">
                <button type="button" className="management-action-btn" onClick={() => setShowAddModal(false)}>
                  Cancel
                </button>
                <button type="submit" className="management-primary-btn">
                  Add Server
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  )
})

export default McpServersView
