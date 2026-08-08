import React, { useState, useEffect } from 'react'
import '../css/management-views.css'
import { renderMcpSvgIcon, renderMcpHubSvgIcon } from '../constants/plugins'

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
}) {
  const [detectedTools, setDetectedTools] = useState({ docker: false, git: false, gh: false, node: false })
  const [expandedMcp, setExpandedMcp] = useState<string | null>(null)
  const [showAddModal, setShowAddModal] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')

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
            Model Context Protocol (MCP) Hub
          </h1>
          <p className="management-subtitle">
            Connect Mint Agent to external tool servers (GitHub, Brave Search, Filesystem, SQLite, Docker, etc.).
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

      {/* MCP Server List */}
      {filteredMcpItems.length === 0 ? (
        <div style={{ padding: '48px 24px', textAlign: 'center', background: 'rgba(255,255,255,0.02)', border: '1px dashed rgba(255,255,255,0.08)', borderRadius: '14px', color: '#94a3b8' }}>
          <div style={{ fontSize: '2rem', marginBottom: '8px' }}>🔌</div>
          <div style={{ fontWeight: 600, color: '#e2e8f0', marginBottom: '4px' }}>No MCP Servers Configured</div>
          <div style={{ fontSize: '0.85rem' }}>Click "Add MCP Server" to connect a new Model Context Protocol tool.</div>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {filteredMcpItems.map((item) => {
            const isExpanded = expandedMcp === item.name
            const srvConfig = config.mcpServers?.[item.name] || { command: item.command, args: item.args, env: {}, icon: item.customIcon }

            return (
              <div
                key={item.name}
                style={{
                  border: '1px solid rgba(255, 255, 255, 0.08)',
                  borderRadius: '12px',
                  background: 'rgba(255, 255, 255, 0.03)',
                  overflow: 'hidden',
                  transition: 'all 0.15s ease',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '16px 20px', flexWrap: 'wrap', gap: '12px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '14px', flex: 1, minWidth: 0 }}>
                    <div style={{ width: '42px', height: '42px', borderRadius: '10px', background: 'rgba(255, 255, 255, 0.06)', display: 'grid', placeItems: 'center', flexShrink: 0 }}>
                      {renderMcpSvgIcon(item.name, item.customIcon)}
                    </div>

                    <div style={{ minWidth: 0 }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <span style={{ fontWeight: 700, fontSize: '0.95rem', color: '#f8fafc' }}>
                          {item.name}
                        </span>
                        {!item.isConfigured && (
                          <span style={{ fontSize: '0.7rem', padding: '2px 6px', background: 'rgba(16, 185, 129, 0.15)', color: '#10b981', borderRadius: '4px', fontWeight: 600 }}>
                            Discovered
                          </span>
                        )}
                        {item.isEnabled && (
                          <span style={{ fontSize: '0.7rem', padding: '2px 6px', background: 'rgba(59, 130, 246, 0.15)', color: '#3b82f6', borderRadius: '4px', fontWeight: 600 }}>
                            Active
                          </span>
                        )}
                      </div>
                      <div style={{ fontFamily: 'monospace', fontSize: '0.78rem', color: '#94a3b8', marginTop: '3px', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                        {item.description}
                      </div>
                    </div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginLeft: 'auto' }}>
                    {item.isConfigured && (
                      <>
                        <button
                          type="button"
                          className="management-action-btn"
                          onClick={() => setExpandedMcp(isExpanded ? null : item.name)}
                        >
                          Configure
                        </button>
                        <button
                          type="button"
                          className="management-action-btn danger"
                          onClick={() => handleRemoveMcpServer(item.name)}
                          title="Remove MCP Server"
                        >
                          Remove
                        </button>
                      </>
                    )}

                    <label className="settings-toggle-switch" title={item.isEnabled ? 'Disable server' : 'Enable server'}>
                      <input
                        type="checkbox"
                        checked={item.isEnabled}
                        onChange={(e) => {
                          handleToggleMcpServer(item.name, e.target.checked, item.command, item.args)
                        }}
                      />
                      <span className="settings-toggle-slider" />
                    </label>
                  </div>
                </div>

                {item.isConfigured && isExpanded && (
                  <div
                    style={{
                      padding: '20px',
                      borderTop: '1px solid rgba(255, 255, 255, 0.08)',
                      background: 'rgba(0, 0, 0, 0.2)',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '14px',
                    }}
                  >
                    <h4 style={{ fontSize: '0.85rem', fontWeight: 600, color: '#3b82f6', textTransform: 'uppercase', letterSpacing: '0.5px', margin: 0 }}>
                      Edit Server Config ({item.name})
                    </h4>

                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
                      <div>
                        <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Command</label>
                        <input
                          type="text"
                          value={srvConfig.command || ''}
                          onChange={(e) => handleUpdateMcpServerField(item.name, 'command', e.target.value)}
                          style={{
                            width: '100%',
                            padding: '8px 12px',
                            borderRadius: '8px',
                            border: '1px solid rgba(255,255,255,0.1)',
                            background: 'rgba(255,255,255,0.04)',
                            color: '#f8fafc',
                            fontSize: '0.85rem',
                            boxSizing: 'border-box',
                          }}
                        />
                      </div>
                      <div>
                        <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Icon (Preset / URL / SVG)</label>
                        <input
                          type="text"
                          placeholder="e.g. search, database, code"
                          value={srvConfig.icon || ''}
                          onChange={(e) => handleUpdateMcpServerField(item.name, 'icon', e.target.value)}
                          style={{
                            width: '100%',
                            padding: '8px 12px',
                            borderRadius: '8px',
                            border: '1px solid rgba(255,255,255,0.1)',
                            background: 'rgba(255,255,255,0.04)',
                            color: '#f8fafc',
                            fontSize: '0.85rem',
                            boxSizing: 'border-box',
                          }}
                        />
                      </div>
                    </div>

                    <div>
                      <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Arguments (Space-separated)</label>
                      <input
                        type="text"
                        value={(srvConfig.args || []).join(' ')}
                        onChange={(e) => handleUpdateMcpServerField(item.name, 'args', e.target.value.split(/\s+/).filter(Boolean))}
                        style={{
                          width: '100%',
                          padding: '8px 12px',
                          borderRadius: '8px',
                          border: '1px solid rgba(255,255,255,0.1)',
                          background: 'rgba(255,255,255,0.04)',
                          color: '#f8fafc',
                          fontSize: '0.85rem',
                          boxSizing: 'border-box',
                        }}
                      />
                    </div>

                    <div>
                      <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Environment Variables (JSON)</label>
                      <textarea
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
                        style={{
                          width: '100%',
                          padding: '8px 12px',
                          borderRadius: '8px',
                          border: '1px solid rgba(255,255,255,0.1)',
                          background: 'rgba(255,255,255,0.04)',
                          color: '#f8fafc',
                          fontSize: '0.85rem',
                          fontFamily: 'monospace',
                          boxSizing: 'border-box',
                          resize: 'vertical',
                        }}
                      />
                    </div>
                  </div>
                )}
              </div>
            )
          })}
        </div>
      )}

      {/* Add MCP Server Modal */}
      {showAddModal && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.7)',
            backdropFilter: 'blur(4px)',
            display: 'grid',
            placeItems: 'center',
            zIndex: 1000,
            padding: '20px',
          }}
          onClick={() => setShowAddModal(false)}
        >
          <div
            style={{
              width: '100%',
              maxWidth: '540px',
              background: '#18181b',
              border: '1px solid rgba(255,255,255,0.12)',
              borderRadius: '14px',
              padding: '24px',
              boxShadow: '0 20px 40px rgba(0,0,0,0.6)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '16px' }}>
              <h2 style={{ fontSize: '1.1rem', fontWeight: 700, color: '#f8fafc', margin: 0 }}>
                Add New MCP Server
              </h2>
              <button
                type="button"
                onClick={() => setShowAddModal(false)}
                style={{ background: 'none', border: 'none', color: '#94a3b8', fontSize: '1.2rem', cursor: 'pointer' }}
              >
                ✕
              </button>
            </div>

            <form onSubmit={onSubmitAddServer} style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
                <div>
                  <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Server Name</label>
                  <input
                    type="text"
                    placeholder="e.g. google-search"
                    value={mcpName}
                    onChange={(e) => setMcpName(e.target.value)}
                    required
                    style={{
                      width: '100%',
                      padding: '10px 14px',
                      borderRadius: '8px',
                      border: '1px solid rgba(255,255,255,0.1)',
                      background: 'rgba(255,255,255,0.04)',
                      color: '#f8fafc',
                      fontSize: '0.88rem',
                      boxSizing: 'border-box',
                    }}
                  />
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Command</label>
                  <input
                    type="text"
                    placeholder="e.g. npx"
                    value={mcpCmd}
                    onChange={(e) => setMcpCmd(e.target.value)}
                    required
                    style={{
                      width: '100%',
                      padding: '10px 14px',
                      borderRadius: '8px',
                      border: '1px solid rgba(255,255,255,0.1)',
                      background: 'rgba(255,255,255,0.04)',
                      color: '#f8fafc',
                      fontSize: '0.88rem',
                      boxSizing: 'border-box',
                    }}
                  />
                </div>
              </div>

              <div>
                <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Arguments</label>
                <input
                  type="text"
                  placeholder="e.g. -y @modelcontextprotocol/server-brave-search"
                  value={mcpArgs}
                  onChange={(e) => setMcpArgs(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '10px 14px',
                    borderRadius: '8px',
                    border: '1px solid rgba(255,255,255,0.1)',
                    background: 'rgba(255,255,255,0.04)',
                    color: '#f8fafc',
                    fontSize: '0.88rem',
                    boxSizing: 'border-box',
                  }}
                />
              </div>

              <div>
                <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Icon (Optional)</label>
                <input
                  type="text"
                  placeholder="e.g. search, database, cloud, code"
                  value={mcpIcon}
                  onChange={(e) => setMcpIcon && setMcpIcon(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '10px 14px',
                    borderRadius: '8px',
                    border: '1px solid rgba(255,255,255,0.1)',
                    background: 'rgba(255,255,255,0.04)',
                    color: '#f8fafc',
                    fontSize: '0.88rem',
                    boxSizing: 'border-box',
                  }}
                />
              </div>

              <div>
                <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Environment Variables (JSON)</label>
                <textarea
                  placeholder='e.g. {"BRAVE_API_KEY": "your_key_here"}'
                  value={mcpEnv}
                  onChange={(e) => setMcpEnv(e.target.value)}
                  rows={3}
                  style={{
                    width: '100%',
                    padding: '10px 14px',
                    borderRadius: '8px',
                    border: '1px solid rgba(255,255,255,0.1)',
                    background: 'rgba(255,255,255,0.04)',
                    color: '#f8fafc',
                    fontSize: '0.88rem',
                    fontFamily: 'monospace',
                    boxSizing: 'border-box',
                    resize: 'vertical',
                  }}
                />
              </div>

              <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', marginTop: '10px' }}>
                <button
                  type="button"
                  onClick={() => setShowAddModal(false)}
                  style={{
                    padding: '8px 16px',
                    borderRadius: '8px',
                    border: '1px solid rgba(255,255,255,0.1)',
                    background: 'transparent',
                    color: '#cbd5e1',
                    fontSize: '0.85rem',
                    cursor: 'pointer',
                  }}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  style={{
                    padding: '8px 18px',
                    borderRadius: '8px',
                    border: 'none',
                    background: '#3b82f6',
                    color: '#ffffff',
                    fontWeight: 600,
                    fontSize: '0.85rem',
                    cursor: 'pointer',
                  }}
                >
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
