import React, { useState, useEffect } from 'react'
import { DEFAULT_CONFIG } from '@/components/SettingsWindow'
import { listLearnedSkills, addLearnedSkill, deleteLearnedSkill, LearnedSkill, detectSystemTools, DetectedTools, listMcpServerTools } from '@/tauri'
import McpToolAllowlist from '../McpToolAllowlist'
import {
  getOAuthStatuses,
  startOAuthFlow,
  revokeOAuth,
  subscribeOAuthChange,
  type OAuthStatus,
} from '../../utils/oauthManager'
import { renderMcpSvgIcon, BUILTIN_PLUGINS_LIST } from '../../constants/plugins'
import { isNativePluginEnabled, applyNativePluginToggle } from '../../utils/nativePlugins'

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
  mcpIcon?: string
  setMcpIcon?: (val: string) => void
  handleAddMcpServer: (allowAll?: boolean) => void
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
  mcpIcon = '',
  setMcpIcon,
  handleAddMcpServer,
  handleRemoveMcpServer,
  handleConnectPlugin
}: PluginsTabProps) {
  // Local state for learned skills
  const [skills, setSkills] = useState<LearnedSkill[]>([])
  const [newSkillName, setNewSkillName] = useState('')
  const [newSkillContent, setNewSkillContent] = useState('')
  const [skillsLoading, setSkillsLoading] = useState(false)
  const [skillsError, setSkillsError] = useState('')

  // Local state for auto-detected tools
  const [detectedTools, setDetectedTools] = useState<DetectedTools>({
    docker: false,
    git: false,
    gh: false,
    node: false
  })
  const [detecting, setDetecting] = useState(false)

  // OAuth statuses
  const [oauthStatuses, setOauthStatuses] = useState<OAuthStatus[]>([])
  const [authenticatingProvider, setAuthenticatingProvider] = useState<string | null>(null)
  const [expandedPlugin, setExpandedPlugin] = useState<string | null>(null)
  const [expandedMcp, setExpandedMcp] = useState<string | null>(null)
  const [addMcpAllowAll, setAddMcpAllowAll] = useState(false)

  useEffect(() => {
    fetchSkills()
    detectTools()
    fetchOAuthStatuses()
    const unsubscribe = subscribeOAuthChange(() => {
      fetchOAuthStatuses()
    })
    return () => {
      unsubscribe()
    }
  }, [])

  const fetchOAuthStatuses = async () => {
    try {
      const statuses = await getOAuthStatuses()
      setOauthStatuses(statuses)
    } catch (err) {
      console.error('Failed to fetch OAuth statuses:', err)
    }
  }

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
    if ((provider === 'google' || provider === 'gmail' || provider === 'google_calendar') && 
        !(config as any).gmailClientId?.trim() && !(config as any).googleCalendarClientId?.trim()) {
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

  const fetchSkills = async () => {
    setSkillsLoading(true)
    setSkillsError('')
    try {
      const activeWorkspace = window.localStorage.getItem('mint:last-workspace-path') || undefined
      const list = await listLearnedSkills(activeWorkspace)
      setSkills(list)
    } catch (err) {
      console.error('Failed to fetch learned skills:', err)
      setSkillsError('Failed to load learned skills')
    } finally {
      setSkillsLoading(false)
    }
  }

  const detectTools = async () => {
    setDetecting(true)
    try {
      const tools = await detectSystemTools()
      setDetectedTools(tools)
    } catch (err) {
      console.error('Failed to detect system tools:', err)
    } finally {
      setDetecting(false)
    }
  }

  const handleAddSkill = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!newSkillName.trim() || !newSkillContent.trim()) return
    try {
      await addLearnedSkill(newSkillName.trim(), newSkillContent.trim())
      setNewSkillName('')
      setNewSkillContent('')
      fetchSkills()
    } catch (err) {
      console.error('Failed to add learned skill:', err)
      alert('Error saving skill')
    }
  }

  const handleDeleteSkill = async (name: string) => {
    if (!confirm(`Are you sure you want to forget skill "${name}"?`)) return
    try {
      await deleteLearnedSkill(name)
      fetchSkills()
    } catch (err) {
      console.error('Failed to delete skill:', err)
      alert('Error deleting skill')
    }
  }

  const handleEnableTool = (name: string, command: string, args: string[]) => {
    const updated = { ...(config.mcpServers || {}) };
    updated[name] = { command, args, env: {} };
    updateField('mcpServers', updated);
  };

  const handleUpdateMcpServerField = (name: string, field: string, value: any) => {
    const updated = { ...(config.mcpServers || {}) };
    if (updated[name]) {
      updated[name] = { ...updated[name], [field]: value };
      updateField('mcpServers', updated);
    }
  };

  const toggleExpand = (key: string) => {
    setExpandedPlugin(prev => prev === key ? null : key)
  }

  const pluginsList = BUILTIN_PLUGINS_LIST

  const handleToggleMcpServer = (name: string, enabled: boolean, defaultCmd?: string, defaultArgs?: string[]) => {
    const updated = { ...(config.mcpServers || {}) }
    if (updated[name]) {
      updated[name] = { ...updated[name], disabled: !enabled }
    } else if (enabled && defaultCmd) {
      updated[name] = { command: defaultCmd, args: defaultArgs || [], env: {}, disabled: false }
    }
    updateField('mcpServers', updated)
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
  }> = [];

  Object.entries(config.mcpServers || {}).forEach(([name, srv]) => {
    let icon = '🔌';
    if (name === 'docker') icon = '🐳';
    if (name === 'git' || name === 'github') icon = '🐙';

    mcpListItems.push({
      name,
      command: srv.command,
      args: srv.args || [],
      icon,
      customIcon: srv.icon,
      isEnabled: (srv as any)?.disabled !== true,
      isConfigured: true,
      description: `Command: ${srv.command} ${(srv.args || []).join(' ')}`
    });
  });

  if (detectedTools.docker && !config.mcpServers?.docker) {
    mcpListItems.push({
      name: 'docker',
      command: 'npx',
      args: ['-y', '@proxeus/mcp-docker-server'],
      icon: '🐳',
      isEnabled: false,
      isConfigured: false,
      description: 'Docker MCP Server (Discovered)'
    });
  }
  if (detectedTools.git && !config.mcpServers?.git) {
    mcpListItems.push({
      name: 'git',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-git'],
      icon: '🐙',
      isEnabled: false,
      isConfigured: false,
      description: 'Git MCP Server (Discovered)'
    });
  }

  return (
    <div className="tab-pane active">

      {/* ── 1. Learned AI Skills ── */}
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Knowledge Base</p>
            <h2 className="section-title">Learned AI Skills</h2>
          </div>
        </div>

        {skills.length === 0 ? (
          <div className="empty-skills-notice">
            No learned skills found. Teach Mint a skill below or run <code>/learn</code> in chat.
          </div>
        ) : (
          <div className="skills-list">
            {skills.map((s) => (
              <div className="skill-card" key={s.name}>
                <div className="skill-card-body">
                  <div className="plugin-name-row">
                    <span className="plugin-tag accent">{s.name}</span>
                    <span className={`plugin-tag ${s.is_workspace ? 'ok' : 'info'}`}>
                      {s.is_workspace ? 'Workspace' : 'Global'}
                    </span>
                  </div>
                  <p className="skill-card-desc">{s.description || s.content}</p>
                </div>
                <button
                  className="btn-danger btn-sm"
                  onClick={() => handleDeleteSkill(s.name)}
                >
                  Forget
                </button>
              </div>
            ))}
          </div>
        )}

        <form className="add-skill-box" onSubmit={handleAddSkill}>
          <h3>Teach New Skill</h3>
          {skillsError && <p className="profile-message-error">{skillsError}</p>}

          <div className="setting-row stacked">
            <label>Skill Name (e.g. coding-guidelines)</label>
            <input
              type="text"
              placeholder="e.g. angular-standard"
              value={newSkillName}
              onChange={(e) => setNewSkillName(e.target.value)}
            />
          </div>

          <div className="setting-row stacked">
            <label>Skill Instructions / Content</label>
            <textarea
              placeholder="# Instructions&#10;Write only clean TypeScript. Use async/await."
              value={newSkillContent}
              onChange={(e) => setNewSkillContent(e.target.value)}
              rows={5}
            />
          </div>

          <button type="submit" className="btn-primary btn-full" disabled={skillsLoading}>
            {skillsLoading ? 'Learning...' : 'Teach Skill'}
          </button>
        </form>
      </section>

      {/* ── 2. External tools (MCP Servers) ── */}
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">External tools</p>
            <h2 className="section-title">MCP Servers</h2>
            <p className="section-desc" style={{ opacity: 0.7, fontSize: '0.85rem' }}>
              Add, edit, enable/disable a server and choose which of its tools the agent may
              call. The same manager opens full-screen with <code>/mcp</code> in chat.
            </p>
          </div>
        </div>

        <div className="mcp-list">
          {mcpListItems.length === 0 ? (
            <div className="mcp-empty-notice">
              No MCP servers configured or discovered.
            </div>
          ) : (
            mcpListItems.map((item) => {
              const isExpanded = expandedMcp === item.name;
              const srvConfig = config.mcpServers?.[item.name] || { command: item.command, args: item.args, env: {}, icon: item.customIcon };

              return (
                <div
                  className={`plugin-card-wrapper${isExpanded ? ' active-plugin-card' : ''}${item.isConfigured ? '' : ' is-discovered'}`}
                  key={item.name}
                >
                  <div className="plugin-card">
                    <div className="plugin-card-main">
                      <div className="plugin-icon">
                        {renderMcpSvgIcon(item.name, item.customIcon)}
                      </div>
                      <div className="plugin-info">
                        <div className="plugin-name plugin-name-row">
                          {item.name}
                          {!item.isConfigured && <span className="plugin-tag ok">Discovered</span>}
                        </div>
                        <div className="plugin-desc mono">
                          {item.description}
                        </div>
                      </div>
                    </div>
                    <div className="plugin-actions">
                      {item.isConfigured && (
                        <>
                          <button
                            type="button"
                            className={`btn-icon${isExpanded ? ' is-active' : ''}`}
                            onClick={() => setExpandedMcp(isExpanded ? null : item.name)}
                            title="Edit MCP server settings"
                          >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                            </svg>
                          </button>
                          <button
                            type="button"
                            className="btn-icon danger"
                            onClick={() => handleRemoveMcpServer(item.name)}
                            title="Delete MCP server"
                          >
                            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                              <polyline points="3 6 5 6 21 6"></polyline>
                              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                              <line x1="10" y1="11" x2="10" y2="17"></line>
                              <line x1="14" y1="11" x2="14" y2="17"></line>
                            </svg>
                          </button>
                        </>
                      )}
                      <label className="settings-toggle-switch">
                        <input
                          type="checkbox"
                          checked={item.isEnabled}
                          onChange={(e) => {
                            handleToggleMcpServer(item.name, e.target.checked, item.command, item.args);
                          }}
                        />
                        <span className="settings-toggle-slider" />
                      </label>
                    </div>
                  </div>

                  {item.isConfigured && isExpanded && (
                    <div className="plugin-config-panel">
                      <h4>Edit settings for {item.name}</h4>

                      <div className="form-grid two-col">
                        <div className="setting-row stacked">
                          <label>Command</label>
                          <input
                            type="text"
                            value={srvConfig.command || ''}
                            onChange={(e) => handleUpdateMcpServerField(item.name, 'command', e.target.value)}
                          />
                        </div>
                        <div className="setting-row stacked">
                          <label>Icon (Optional)</label>
                          <input
                            type="text"
                            placeholder="SVG code, URL, or preset: search/database/cloud/code/api"
                            value={srvConfig.icon || ''}
                            onChange={(e) => handleUpdateMcpServerField(item.name, 'icon', e.target.value)}
                          />
                        </div>
                      </div>

                      <div className="setting-row stacked">
                        <label>Arguments (Space separated)</label>
                        <input
                          type="text"
                          value={(srvConfig.args || []).join(' ')}
                          onChange={(e) => handleUpdateMcpServerField(item.name, 'args', e.target.value.split(/\s+/).filter(Boolean))}
                        />
                      </div>

                      <div className="setting-row stacked">
                        <label>Environment Variables (JSON format)</label>
                        <textarea
                          value={typeof srvConfig.env === 'object' ? JSON.stringify(srvConfig.env, null, 2) : (srvConfig.env || '')}
                          onChange={(e) => {
                            try {
                              const parsed = JSON.parse(e.target.value)
                              handleUpdateMcpServerField(item.name, 'env', parsed)
                            } catch {
                              // allow live typing
                            }
                          }}
                          rows={3}
                        />
                      </div>

                      <McpToolAllowlist
                        serverName={item.name}
                        config={config}
                        updateField={updateField}
                        listServerTools={listMcpServerTools}
                      />
                    </div>
                  )}
                </div>
              )
            })
          )}
        </div>

        <div className="add-mcp-box">
          <h3>Add MCP Server</h3>
          <div className="form-grid two-col">
            <div className="setting-row stacked">
              <label>Server Name</label>
              <input
                type="text"
                placeholder="e.g. google-search"
                value={mcpName}
                onChange={(e) => setMcpName(e.target.value)}
              />
            </div>
            <div className="setting-row stacked">
              <label>Command</label>
              <input
                type="text"
                placeholder="e.g. npx"
                value={mcpCmd}
                onChange={(e) => setMcpCmd(e.target.value)}
              />
            </div>
          </div>
          <div className="setting-row stacked">
            <label>Arguments</label>
            <input
              type="text"
              placeholder="e.g. -y @modelcontextprotocol/server-brave-search"
              value={mcpArgs}
              onChange={(e) => setMcpArgs(e.target.value)}
            />
          </div>
          <div className="setting-row stacked">
            <label>Icon (Optional)</label>
            <input
              type="text"
              placeholder="SVG code, URL, or preset: search/database/cloud/code/api"
              value={mcpIcon}
              onChange={(e) => setMcpIcon && setMcpIcon(e.target.value)}
            />
          </div>
          <div className="setting-row stacked">
            <label>Environment Variables (JSON)</label>
            <textarea
              placeholder='e.g. {"BRAVE_API_KEY": "your_key_here"}'
              value={mcpEnv}
              onChange={(e) => setMcpEnv(e.target.value)}
              rows={3}
            />
          </div>
          <label className="setting-row" style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8, cursor: 'pointer' }}>
            <input type="checkbox" checked={addMcpAllowAll} onChange={(e) => setAddMcpAllowAll(e.target.checked)} />
            <span>
              Allow the agent to call all of this server’s tools (*)
              <span style={{ display: 'block', opacity: 0.6, fontSize: '0.8rem' }}>
                Leave off to approve tools one by one afterwards.
              </span>
            </span>
          </label>
          <button
            className="btn-primary btn-full"
            onClick={() => {
              handleAddMcpServer(addMcpAllowAll)
              setAddMcpAllowAll(false)
            }}
          >
            Add MCP Server
          </button>
        </div>
      </section>

      {/* ── 3. Built-in Plugins ── */}
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Integrations</p>
            <h2 className="section-title">Plugins &amp; Integrations</h2>
          </div>
        </div>
        <div className="plugin-list">
          {pluginsList.map(p => {
            const isEnabled = isNativePluginEnabled(config, p)
            const isExpanded = expandedPlugin === p.key

            return (
              <div className={`plugin-card-wrapper${isEnabled ? ' active-plugin-card' : ''}`} key={p.key}>
                <div className="plugin-card">
                  <div className="plugin-card-main">
                    <div className="plugin-icon">
                      {renderMcpSvgIcon(p.key)}
                    </div>
                    <div className="plugin-info">
                      <div className="plugin-name plugin-name-row">
                        {p.name}
                        {p.isOAuth && (() => {
                          const oauthMatch = oauthStatuses.find(s => s.provider === p.oauthProvider)
                          const isConn = oauthMatch?.connected
                          return (
                            <span className={`plugin-status${isConn ? ' connected' : ''}`}>
                              {isConn ? `Connected ${oauthMatch.accountEmail ? `(${oauthMatch.accountEmail})` : ''}` : 'Not connected'}
                            </span>
                          )
                        })()}
                      </div>
                      <div className="plugin-desc">{p.desc}</div>
                    </div>
                  </div>
                  <div className="plugin-actions">
                    {p.isOAuth && (() => {
                      const oauthMatch = oauthStatuses.find(s => s.provider === p.oauthProvider)
                      const isConn = oauthMatch?.connected
                      return isConn ? (
                        <button
                          className="btn-danger btn-sm"
                          onClick={() => handleRevokeOAuth(p.oauthProvider!)}
                        >
                          Disconnect
                        </button>
                      ) : (
                        <button
                          className="btn-primary btn-sm"
                          onClick={() => handleConnectOAuth(p.oauthProvider!, p.key)}
                          disabled={authenticatingProvider === p.oauthProvider}
                        >
                          {authenticatingProvider === p.oauthProvider ? 'Signing In...' : 'Sign In'}
                        </button>
                      )
                    })()}
                    {p.hasCredentials && (
                      <button
                        className="btn-secondary btn-sm"
                        onClick={() => toggleExpand(p.key)}
                      >
                        {isExpanded ? 'Hide Config' : 'Configure'}
                      </button>
                    )}
                    {p.key === 'discord' && isEnabled && (
                      <button
                        className="btn-secondary btn-sm"
                        onClick={() => handleConnectPlugin('discord')}
                      >
                        Update RPC
                      </button>
                    )}
                    <label className="settings-toggle-switch">
                      <input
                        type="checkbox"
                        checked={isEnabled}
                        onChange={(e) => {
                          applyNativePluginToggle(config, p, e.target.checked, updateField)
                        }}
                      />
                      <span className="settings-toggle-slider" />
                    </label>
                  </div>
                </div>

                {p.hasCredentials && isExpanded && (
                  <div className="plugin-config-panel">
                    <h4>Credentials</h4>
                    <div className="form-grid compact">
                      {p.fields?.map(f => (
                        <div className="setting-row stacked" key={f.field}>
                          <label>{f.label}</label>
                          <input
                            type={f.type}
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
            );
          })}
        </div>
      </section>
    </div>
  )
}
