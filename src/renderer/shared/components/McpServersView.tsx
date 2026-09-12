import React, { useState, useEffect } from 'react'
import '../css/management-views.css'
import { renderMcpSvgIcon, renderMcpHubSvgIcon } from '../constants/plugins'
import McpToolAllowlist from './McpToolAllowlist'
import McpRegistryPicker from './McpRegistryPicker'
import type { McpRegistryEntry } from '../constants/mcpRegistry'
import { testMcpConnection } from '@/tauri'
import { Globe, Terminal, Zap, Loader2, Check, AlertCircle, ShieldAlert } from 'lucide-react'

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
  handleAddMcpServer: (allowAll?: boolean) => void
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
  const [showCatalogModal, setShowCatalogModal] = useState(false)
  const [addAllowAll, setAddAllowAll] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [reauthStatus, setReauthStatus] = useState<Record<string, 'idle' | 'running' | 'success' | 'error'>>({})

  // Remote MCP states
  const [serverType, setServerType] = useState<'url' | 'command'>('url')
  const [mcpUrl, setMcpUrl] = useState('')
  const [authType, setAuthType] = useState<'none' | 'bearer' | 'custom'>('none')
  const [bearerToken, setBearerToken] = useState('')
  const [customHeaders, setCustomHeaders] = useState('')
  const [testStatus, setTestStatus] = useState<'idle' | 'testing' | 'success' | 'error'>('idle')
  const [testMessage, setTestMessage] = useState('')
  const [securityAcknowledged, setSecurityAcknowledged] = useState(false)

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
    isRemote: boolean
    url?: string
    description?: string
  }> = []

  Object.entries(config.mcpServers || {}).forEach(([name, srv]: [string, any]) => {
    let icon = 'plug'
    if (name === 'docker') icon = 'docker'
    if (name === 'git' || name === 'github') icon = 'git'
    if (srv.url) icon = 'globe'

    const isRemote = Boolean(srv.url)
    const desc = isRemote
      ? `URL: ${srv.url}`
      : `Command: ${srv.command} ${(srv.args || []).join(' ')}`

    mcpListItems.push({
      name,
      command: srv.command || '',
      args: srv.args || [],
      icon,
      customIcon: srv.icon,
      isEnabled: srv?.disabled !== true,
      isConfigured: true,
      isRemote,
      url: srv.url,
      description: desc,
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
      isRemote: false,
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
      isRemote: false,
      description: 'Git MCP Server (Auto Discovered)',
    })
  }

  const filteredMcpItems = mcpListItems.filter((item) =>
    item.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    item.command.toLowerCase().includes(searchQuery.toLowerCase()) ||
    (item.url || '').toLowerCase().includes(searchQuery.toLowerCase()) ||
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

  const handleTestConnection = async () => {
    if (!mcpUrl.trim()) {
      setTestStatus('error')
      setTestMessage('Please enter a server URL first.')
      return
    }
    let headers: Record<string, string> | undefined
    if (authType === 'bearer') {
      if (bearerToken.trim()) {
        const token = bearerToken.trim().startsWith('Bearer ') ? bearerToken.trim() : `Bearer ${bearerToken.trim()}`
        headers = { Authorization: token }
      }
    } else if (authType === 'custom') {
      if (customHeaders.trim()) {
        try {
          headers = JSON.parse(customHeaders.trim())
        } catch {
          setTestStatus('error')
          setTestMessage('Invalid JSON in Custom Headers field.')
          return
        }
      }
    }
    setTestStatus('testing')
    setTestMessage('Connecting to MCP server...')
    try {
      const res = await testMcpConnection(mcpUrl.trim(), headers)
      if (res.ok) {
        setTestStatus('success')
        const name = res.server_info?.name || 'Remote Server'
        const count = res.tools_count ?? 0
        setTestMessage(`Connected: ${name} (${count} tools discovered)`)
      } else {
        setTestStatus('error')
        setTestMessage(`Connection failed: ${res.error || 'Server unreachable'}`)
      }
    } catch (err: any) {
      setTestStatus('error')
      setTestMessage(`Connection failed: ${err?.message || String(err)}`)
    }
  }

  const onSubmitAddServer = (e: React.FormEvent) => {
    e.preventDefault()
    if (serverType === 'url') {
      if (!mcpName.trim() || !mcpUrl.trim()) {
        alert('Please provide a server name and URL.')
        return
      }
      if (!securityAcknowledged) {
        alert('Please acknowledge the security risks of connecting to a custom remote MCP server.')
        return
      }
      let headers: Record<string, string> | undefined
      if (authType === 'bearer' && bearerToken.trim()) {
        const token = bearerToken.trim().startsWith('Bearer ') ? bearerToken.trim() : `Bearer ${bearerToken.trim()}`
        headers = { Authorization: token }
      } else if (authType === 'custom' && customHeaders.trim()) {
        try {
          headers = JSON.parse(customHeaders.trim())
        } catch {
          alert('Invalid JSON in Custom Headers field.')
          return
        }
      }
      const name = mcpName.trim()
      const updated = { ...(config.mcpServers || {}) }
      updated[name] = {
        url: mcpUrl.trim(),
        headers,
        transport: 'sse',
        disabled: false,
        icon: mcpIcon?.trim() || undefined,
      }
      updateField('mcpServers', updated)
      if (addAllowAll) {
        const currentAllowed = (config as any).allowedMcpTools || {}
        updateField('allowedMcpTools', { ...currentAllowed, [name]: ['*'] })
      }
      setMcpName('')
      setMcpUrl('')
      setBearerToken('')
      setCustomHeaders('')
      setTestStatus('idle')
      setTestMessage('')
      setSecurityAcknowledged(false)
      setAddAllowAll(false)
      setShowAddModal(false)
    } else {
      handleAddMcpServer(addAllowAll)
      setAddAllowAll(false)
      setShowAddModal(false)
    }
  }

  const applyRegistryPick = (
    entry: McpRegistryEntry,
    argValues: string[],
    envSeed: Record<string, string>,
  ) => {
    setServerType('command')
    setMcpName(entry.key)
    setMcpCmd(entry.command)
    setMcpArgs([...(entry.args || []), ...argValues].join(' '))
    setMcpEnv(Object.keys(envSeed).length ? JSON.stringify(envSeed, null, 2) : '')
    if (setMcpIcon) setMcpIcon(entry.icon || '')
    // Hand off to the manual form pre-filled, so the user reviews before adding.
    setShowCatalogModal(false)
    setShowAddModal(true)
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

        <div style={{ display: 'flex', gap: 8 }}>
          <button
            type="button"
            className="management-action-btn"
            onClick={() => setShowCatalogModal(true)}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            MCP catalog
          </button>
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
                  {item.isRemote && (
                    <span className="management-badge remote" style={{ fontSize: '0.72rem', padding: '2px 7px', display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
                      <Globe size={11} /> Remote
                    </span>
                  )}
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
                    {srvConfig.url ? (
                      <div style={{ display: 'grid', gap: '12px' }}>
                        <div className="management-form-group">
                          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                            <label className="management-label">Server URL</label>
                            <span className="management-badge remote" style={{ fontSize: '0.7rem' }}>SSE Transport</span>
                          </div>
                          <input
                            type="text"
                            className="management-input-field"
                            value={srvConfig.url || ''}
                            onChange={(e) => handleUpdateMcpServerField(item.name, 'url', e.target.value)}
                          />
                        </div>
                        <div className="management-form-group">
                          <label className="management-label">Custom Headers (JSON)</label>
                          <textarea
                            className="management-textarea-field"
                            value={typeof srvConfig.headers === 'object' ? JSON.stringify(srvConfig.headers, null, 2) : srvConfig.headers || ''}
                            onChange={(e) => {
                              try {
                                const parsed = JSON.parse(e.target.value)
                                handleUpdateMcpServerField(item.name, 'headers', parsed)
                              } catch {
                                // allow live editing
                              }
                            }}
                            rows={3}
                            placeholder='{"Authorization": "Bearer ..."}'
                          />
                        </div>
                        <div className="management-form-group">
                          <label className="management-label">Icon (preset / URL / SVG)</label>
                          <input
                            type="text"
                            className="management-input-field"
                            placeholder="e.g. globe, search, database"
                            value={srvConfig.icon || ''}
                            onChange={(e) => handleUpdateMcpServerField(item.name, 'icon', e.target.value)}
                          />
                        </div>
                      </div>
                    ) : (
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
                    )}

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
                {/* Segmented Switcher */}
                <div className="mcp-server-type-toggle">
                  <button
                    type="button"
                    className={`mcp-toggle-btn ${serverType === 'url' ? 'active' : ''}`}
                    onClick={() => {
                      setServerType('url')
                      setTestStatus('idle')
                      setTestMessage('')
                    }}
                  >
                    <Globe size={15} /> Remote Server (URL / SSE)
                  </button>
                  <button
                    type="button"
                    className={`mcp-toggle-btn ${serverType === 'command' ? 'active' : ''}`}
                    onClick={() => {
                      setServerType('command')
                      setTestStatus('idle')
                      setTestMessage('')
                    }}
                  >
                    <Terminal size={15} /> Local Command (stdio)
                  </button>
                </div>

                {serverType === 'url' ? (
                  <>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
                      <div className="management-form-group">
                        <label className="management-label">Server Name</label>
                        <input
                          type="text"
                          className="management-input-field"
                          placeholder="e.g. cloud-mcp"
                          value={mcpName}
                          onChange={(e) => setMcpName(e.target.value)}
                          required
                        />
                      </div>
                      <div className="management-form-group">
                        <label className="management-label">Authentication</label>
                        <select
                          className="management-input-field"
                          value={authType}
                          onChange={(e) => {
                            setAuthType(e.target.value as any)
                            setTestStatus('idle')
                            setTestMessage('')
                          }}
                          style={{ cursor: 'pointer' }}
                        >
                          <option value="none">None (Public)</option>
                          <option value="bearer">Bearer Token</option>
                          <option value="custom">Custom Headers (JSON)</option>
                        </select>
                      </div>
                    </div>

                    <div className="management-form-group">
                      <label className="management-label">Server URL</label>
                      <input
                        type="url"
                        className="management-input-field"
                        placeholder="https://example.com/sse or http://localhost:8000/sse"
                        value={mcpUrl}
                        onChange={(e) => {
                          setMcpUrl(e.target.value)
                          setTestStatus('idle')
                          setTestMessage('')
                        }}
                        required
                      />
                    </div>

                    {authType === 'bearer' && (
                      <div className="management-form-group">
                        <label className="management-label">Bearer Token</label>
                        <input
                          type="password"
                          className="management-input-field"
                          placeholder="Paste API token or JWT..."
                          value={bearerToken}
                          onChange={(e) => {
                            setBearerToken(e.target.value)
                            setTestStatus('idle')
                            setTestMessage('')
                          }}
                        />
                      </div>
                    )}

                    {authType === 'custom' && (
                      <div className="management-form-group">
                        <label className="management-label">Custom HTTP Headers (JSON)</label>
                        <textarea
                          className="management-textarea-field"
                          placeholder='{"Authorization": "Bearer ...", "X-Custom-Auth": "secret"}'
                          value={customHeaders}
                          onChange={(e) => {
                            setCustomHeaders(e.target.value)
                            setTestStatus('idle')
                            setTestMessage('')
                          }}
                          rows={3}
                        />
                      </div>
                    )}

                    <div className="management-form-group">
                      <label className="management-label">Icon (Optional)</label>
                      <input
                        type="text"
                        className="management-input-field"
                        placeholder="e.g. globe, cloud, database, search"
                        value={mcpIcon}
                        onChange={(e) => setMcpIcon && setMcpIcon(e.target.value)}
                      />
                    </div>

                    {/* Test Connection Bar */}
                    <div className="mcp-test-row">
                      <button
                        type="button"
                        className="mcp-test-btn"
                        onClick={handleTestConnection}
                        disabled={testStatus === 'testing' || !mcpUrl.trim()}
                      >
                        {testStatus === 'testing' ? (
                          <>
                            <Loader2 size={13} className="mcp-spin" /> Testing...
                          </>
                        ) : (
                          <>
                            <Zap size={13} /> Test Connection
                          </>
                        )}
                      </button>
                      {testMessage && (
                        <span className={`mcp-test-status ${testStatus}`}>
                          {testStatus === 'success' && <Check size={13} />}
                          {testStatus === 'error' && <AlertCircle size={13} />}
                          {testMessage}
                        </span>
                      )}
                    </div>

                    {/* Security Notice / Warning Box */}
                    <div className="mcp-security-notice">
                      <div className="mcp-security-header">
                        <ShieldAlert size={16} className="mcp-security-icon" />
                        <span>Custom MCP servers introduce security risks</span>
                      </div>
                      <div className="mcp-security-body">
                        <label className="mcp-security-ack">
                          <input
                            type="checkbox"
                            className="mint-custom-checkbox"
                            checked={securityAcknowledged}
                            onChange={(e) => setSecurityAcknowledged(e.target.checked)}
                          />
                          <span className="mcp-security-ack-text">
                            <strong>I understand the risks and wish to proceed</strong>
                            <span className="mcp-security-subtext">
                              External endpoints can execute tools and receive prompt context. Malicious servers could attempt to access sensitive information or trigger unintended actions. Only connect to endpoints you trust.
                            </span>
                          </span>
                        </label>
                      </div>
                    </div>
                  </>
                ) : (
                  <>
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
                  </>
                )}

                <label className="mcp-allow-all-row">
                  <input
                    type="checkbox"
                    className="mint-custom-checkbox"
                    checked={addAllowAll}
                    onChange={(e) => setAddAllowAll(e.target.checked)}
                  />
                  <span className="mcp-allow-all-text">
                    <span className="mcp-allow-all-title">Allow the agent to call all of this server’s tools (*)</span>
                    <span className="mcp-allow-all-desc">
                      Leave off to approve tools one by one afterwards.
                    </span>
                  </span>
                </label>
              </div>

              <div className="management-modal-footer">
                <button type="button" className="management-action-btn" onClick={() => setShowAddModal(false)}>
                  Cancel
                </button>
                <button
                  type="submit"
                  className="management-primary-btn"
                  disabled={serverType === 'url' && !securityAcknowledged}
                >
                  Add Server
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* MCP Catalog Modal */}
      {showCatalogModal && (
        <div className="management-modal-overlay" onClick={() => setShowCatalogModal(false)}>
          <div className="management-modal mcp-catalog-modal" onClick={(e) => e.stopPropagation()}>
            <div className="management-modal-header">
              <h2 className="management-modal-title">MCP Catalog</h2>
              <button type="button" className="management-modal-close" onClick={() => setShowCatalogModal(false)}>
                ✕
              </button>
            </div>
            <div className="management-modal-body">
              <McpRegistryPicker
                configuredNames={Object.keys(config.mcpServers || {})}
                onPick={applyRegistryPick}
                showManualHint={false}
              />
            </div>
            <div className="management-modal-footer">
              <span />
              <button
                type="button"
                className="management-action-btn"
                onClick={() => {
                  setShowCatalogModal(false)
                  setShowAddModal(true)
                }}
              >
                Add manually instead
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
})

export default McpServersView
