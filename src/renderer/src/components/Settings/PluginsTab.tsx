import React, { useState, useEffect } from 'react'
import { DEFAULT_CONFIG } from '../SettingsWindow'
import { listLearnedSkills, addLearnedSkill, deleteLearnedSkill, LearnedSkill, detectSystemTools, DetectedTools } from '../../tauri'
import {
  getOAuthStatuses,
  startOAuthFlow,
  revokeOAuth,
  subscribeOAuthChange,
  type OAuthStatus,
} from '../../../shared/utils/oauthManager'
import { renderMcpSvgIcon, BUILTIN_PLUGINS_LIST } from '../../../shared/constants/plugins'

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
      isEnabled: true,
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
    <div className="tab-pane active" style={{ display: 'flex', flexDirection: 'column', gap: '24px' }}>
      
      {/* ── 1. Learned AI Skills ── */}
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Knowledge Base</p>
            <h2 className="section-title">Learned AI Skills</h2>
          </div>
          <p className="section-description">
            Skills are special instructions or guides taught to Mint. The AI reads active skills before every prompt to align with your guidelines.
          </p>
        </div>

        {skills.length === 0 ? (
          <div className="empty-skills-notice" style={{
            padding: '24px',
            border: '1px dashed var(--border)',
            borderRadius: '12px',
            textAlign: 'center',
            color: 'var(--text-muted)',
            background: 'rgba(0,0,0,0.06)',
            fontSize: '0.9rem',
            marginBottom: '20px'
          }}>
            No learned skills found. Teach Mint a skill below or run <code>/learn</code> in chat.
          </div>
        ) : (
          <div className="skills-list" style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginBottom: '20px' }}>
            {skills.map((s) => (
              <div className="skill-card" key={s.name} style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '16px 20px',
                border: '1px solid var(--border)',
                borderRadius: '12px',
                background: 'var(--surface-bg)',
                gap: '16px'
              }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span className="badge" style={{
                      background: 'var(--accent)',
                      color: '#fff',
                      fontSize: '0.75rem',
                      fontWeight: '600',
                      padding: '2px 8px',
                      borderRadius: '4px'
                    }}>{s.name}</span>
                    <span className="location-badge" style={{
                      background: s.is_workspace ? 'rgba(16, 185, 129, 0.15)' : 'rgba(59, 130, 246, 0.15)',
                      color: s.is_workspace ? '#10b981' : '#3b82f6',
                      fontSize: '0.7rem',
                      fontWeight: '600',
                      padding: '2px 6px',
                      borderRadius: '4px'
                    }}>
                      {s.is_workspace ? 'Workspace' : 'Global'}
                    </span>
                  </div>
                  <p style={{
                    fontSize: '0.8rem',
                    color: 'var(--text-muted)',
                    margin: '6px 0 0 0',
                    display: '-webkit-box',
                    WebkitLineClamp: 2,
                    WebkitBoxOrient: 'vertical',
                    overflow: 'hidden'
                  }}>
                    {s.description || s.content}
                  </p>
                </div>
                <button 
                  className="btn btn-danger" 
                  onClick={() => handleDeleteSkill(s.name)}
                  style={{ padding: '6px 12px', fontSize: '0.8rem', height: '34px', flexShrink: 0 }}
                >
                  Forget
                </button>
              </div>
            ))}
          </div>
        )}

        <form className="add-skill-box" onSubmit={handleAddSkill} style={{
          padding: '20px',
          border: '1px solid var(--border)',
          borderRadius: '12px',
          background: 'var(--surface-bg)'
        }}>
          <h3 style={{ fontSize: '1rem', fontWeight: '600', color: 'var(--text-main)', marginBottom: '14px' }}>Teach New Skill</h3>
          {skillsError && <div style={{ color: '#ef4444', fontSize: '0.85rem', marginBottom: '12px', fontWeight: '500' }}>{skillsError}</div>}
          
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '12px' }}>
            <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Skill Name (e.g. coding-guidelines)</label>
            <input
              type="text"
              placeholder="e.g. angular-standard"
              value={newSkillName}
              onChange={(e) => setNewSkillName(e.target.value)}
              style={{ padding: '10px 14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.9rem' }}
            />
          </div>
          
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '14px' }}>
            <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Skill Instructions / Content</label>
            <textarea
              placeholder="# Instructions&#10;Write only clean TypeScript. Use async/await."
              value={newSkillContent}
              onChange={(e) => setNewSkillContent(e.target.value)}
              style={{ padding: '10px 14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.9rem', width: '100%', height: '100px', resize: 'vertical' }}
            />
          </div>

          <button 
            type="submit" 
            className="btn-primary" 
            disabled={skillsLoading}
            style={{ width: '100%', padding: '10px 16px', borderRadius: '8px', border: 'none', background: 'var(--accent)', color: '#fff', fontWeight: '600', cursor: 'pointer' }}
          >
            {skillsLoading ? 'Learning...' : 'Teach Skill'}
          </button>
        </form>
      </section>

      {/* ── 2. External tools (MCP Servers) ── */}
      <section className="setting-section" style={{ borderTop: '1px solid var(--border)', paddingTop: '24px' }}>
        <div className="section-heading">
          <div>
            <p className="section-kicker">External tools</p>
            <h2 className="section-title">MCP Servers</h2>
          </div>
          <p className="section-description">Connect Mint to tools like search, GitHub, or filesystem servers.</p>
        </div>

        <div className="mcp-list">
          {mcpListItems.length === 0 ? (
            <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-muted)' }}>
              No MCP servers configured or discovered.
            </div>
          ) : (
            mcpListItems.map((item) => {
              const isExpanded = expandedMcp === item.name;
              const srvConfig = config.mcpServers?.[item.name] || { command: item.command, args: item.args, env: {}, icon: item.customIcon };

              return (
                <div 
                  className={`plugin-card-wrapper ${isExpanded ? 'active-plugin-card' : ''}`}
                  key={item.name} 
                  style={{
                    border: '1px solid var(--border)',
                    borderRadius: '12px',
                    background: item.isConfigured ? 'var(--surface-bg)' : 'rgba(16, 185, 129, 0.03)',
                    opacity: item.isConfigured ? 1 : 0.65,
                    marginBottom: '10px',
                    transition: 'all 0.2s ease',
                    overflow: 'hidden'
                  }}
                >
                  <div className="plugin-card" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '16px 20px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                      <div className="plugin-icon" style={{ fontSize: '1.5rem', width: '40px', height: '40px', display: 'grid', placeItems: 'center', background: 'var(--surface-strong)', borderRadius: '10px' }}>
                        {renderMcpSvgIcon(item.name, item.customIcon)}
                      </div>
                      <div className="plugin-info">
                        <div className="plugin-name" style={{ fontWeight: '600', color: 'var(--text-main)' }}>
                          {item.name} {!item.isConfigured && <span style={{ fontSize: '0.75rem', padding: '2px 6px', background: 'rgba(16, 185, 129, 0.15)', color: '#10b981', borderRadius: '4px', marginLeft: '6px' }}>Discovered</span>}
                        </div>
                        <div className="plugin-desc" style={{ fontFamily: 'monospace', fontSize: '0.8rem', color: 'var(--text-muted)' }}>
                          {item.description}
                        </div>
                      </div>
                    </div>
                    <div className="plugin-actions" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      {item.isConfigured && (
                        <>
                          <button 
                            className="btn-icon-edit" 
                            onClick={() => setExpandedMcp(isExpanded ? null : item.name)}
                            style={{ 
                              background: isExpanded ? 'var(--accent)' : 'transparent',
                              border: 'none',
                              color: isExpanded ? '#fff' : 'var(--text-soft)',
                              cursor: 'pointer',
                              padding: '6px',
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                              borderRadius: '6px',
                              transition: 'all 0.2s'
                            }}
                            title="Edit MCP Server Settings"
                          >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                            </svg>
                          </button>
                          <button 
                            className="btn-icon-danger" 
                            onClick={() => handleRemoveMcpServer(item.name)} 
                            style={{ 
                              background: 'transparent',
                              border: 'none',
                              color: '#ef4444',
                              cursor: 'pointer',
                              padding: '6px',
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                              borderRadius: '6px',
                              transition: 'background 0.2s'
                            }}
                            title="Delete MCP Server"
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
                            if (e.target.checked) {
                              handleEnableTool(item.name, item.command, item.args);
                            } else {
                              handleRemoveMcpServer(item.name);
                            }
                          }}
                        />
                        <span className="settings-toggle-slider" />
                      </label>
                    </div>
                  </div>

                  {item.isConfigured && isExpanded && (
                    <div className="plugin-config-panel" style={{
                      padding: '16px 20px',
                      borderTop: '1px solid var(--border)',
                      background: 'rgba(0, 0, 0, 0.18)',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '12px'
                    }}>
                      <h4 style={{ fontSize: '0.85rem', fontWeight: '600', color: 'var(--accent)', textTransform: 'uppercase', letterSpacing: '0.5px' }}>
                        Edit Settings for {item.name}
                      </h4>

                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                          <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Command</label>
                          <input
                            type="text"
                            value={srvConfig.command || ''}
                            onChange={(e) => handleUpdateMcpServerField(item.name, 'command', e.target.value)}
                            style={{ padding: '8px 12px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.85rem' }}
                          />
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                          <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Icon (Optional)</label>
                          <input
                            type="text"
                            placeholder="SVG code, URL, or preset: search/database/cloud/code/api"
                            value={srvConfig.icon || ''}
                            onChange={(e) => handleUpdateMcpServerField(item.name, 'icon', e.target.value)}
                            style={{ padding: '8px 12px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.85rem' }}
                          />
                        </div>
                      </div>

                      <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                        <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Arguments (Space separated)</label>
                        <input
                          type="text"
                          value={(srvConfig.args || []).join(' ')}
                          onChange={(e) => handleUpdateMcpServerField(item.name, 'args', e.target.value.split(/\s+/).filter(Boolean))}
                          style={{ padding: '8px 12px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.85rem', width: '100%' }}
                        />
                      </div>

                      <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                        <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Environment Variables (JSON format)</label>
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
                          style={{ padding: '8px 12px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.85rem', fontFamily: 'monospace', height: '65px', resize: 'vertical' }}
                        />
                      </div>
                    </div>
                  )}
                </div>
              )
            })
          )}
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
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '12px' }}>
            <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Icon (Optional)</label>
            <input
              type="text"
              placeholder="SVG code, URL, or preset: search/database/cloud/code/api"
              value={mcpIcon}
              onChange={(e) => setMcpIcon && setMcpIcon(e.target.value)}
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

      {/* ── 3. Built-in Plugins ── */}
      <section className="setting-section" style={{ borderTop: '1px solid var(--border)', paddingTop: '24px' }}>
        <div className="section-heading">
          <div>
            <p className="section-kicker">Integrations</p>
            <h2 className="section-title">Built-in Plugins</h2>
          </div>
          <p className="section-description">Enable and configure credentials for native Mint plugins.</p>
        </div>
        <div className="plugin-list" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          {pluginsList.map(p => {
            const isEnabled = (config as any)[p.enabledField]
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
                      {renderMcpSvgIcon(p.key)}
                    </div>
                    <div className="plugin-info">
                      <div className="plugin-name" style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                        <span style={{ fontWeight: '600', color: 'var(--text-main)' }}>{p.name}</span>
                        {p.isOAuth && (() => {
                          const oauthMatch = oauthStatuses.find(s => s.provider === p.oauthProvider)
                          const isConn = oauthMatch?.connected
                          return (
                            <span style={{
                              fontSize: '0.72rem',
                              padding: '2px 8px',
                              borderRadius: '6px',
                              background: isConn ? 'rgba(16, 185, 129, 0.15)' : 'rgba(255, 255, 255, 0.05)',
                              color: isConn ? '#10b981' : 'var(--text-muted)',
                              border: `1px solid ${isConn ? 'rgba(16, 185, 129, 0.3)' : 'var(--border)'}`,
                              fontWeight: '500',
                              whiteSpace: 'nowrap',
                              display: 'inline-flex',
                              alignItems: 'center',
                              gap: '5px'
                            }}>
                              <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: isConn ? '#10b981' : 'var(--text-muted)' }} />
                              {isConn ? `Connected ${oauthMatch.accountEmail ? `(${oauthMatch.accountEmail})` : ''}` : 'Not Connected'}
                            </span>
                          )
                        })()}
                      </div>
                      <div className="plugin-desc" style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginTop: '2px' }}>{p.desc}</div>
                    </div>
                  </div>
                  <div className="plugin-actions" style={{ display: 'flex', alignItems: 'center', gap: '10px', flexShrink: 0 }}>
                    {p.isOAuth && (() => {
                      const oauthMatch = oauthStatuses.find(s => s.provider === p.oauthProvider)
                      const isConn = oauthMatch?.connected
                      return isConn ? (
                        <button
                          className="btn-danger"
                          onClick={() => handleRevokeOAuth(p.oauthProvider!)}
                          style={{ padding: '6px 14px', fontSize: '0.8rem', height: '34px', whiteSpace: 'nowrap', borderRadius: '8px' }}
                        >
                          Disconnect
                        </button>
                      ) : (
                        <button
                          className="btn-primary"
                          onClick={() => handleConnectOAuth(p.oauthProvider!, p.key)}
                          disabled={authenticatingProvider === p.oauthProvider}
                          style={{ 
                            padding: '6px 14px', 
                            fontSize: '0.8rem', 
                            height: '34px', 
                            whiteSpace: 'nowrap', 
                            borderRadius: '8px',
                            display: 'inline-flex',
                            alignItems: 'center',
                            gap: '6px',
                            background: '#10b981',
                            color: '#ffffff',
                            border: 'none',
                            fontWeight: '600',
                            cursor: 'pointer'
                          }}
                        >
                          {authenticatingProvider === p.oauthProvider ? (
                            'Signing In...'
                          ) : (
                            <>
                              Sign In
                              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
                                <circle cx="12" cy="12" r="10"/>
                                <line x1="2" y1="12" x2="22" y2="12"/>
                                <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
                              </svg>
                            </>
                          )}
                        </button>
                      )
                    })()}
                    {p.hasCredentials && (
                      <button
                        className="btn-secondary"
                        onClick={() => toggleExpand(p.key)}
                        style={{ padding: '6px 14px', fontSize: '0.8rem', height: '34px', whiteSpace: 'nowrap', borderRadius: '8px' }}
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
                          updateField(p.enabledField as any, e.target.checked)
                        }}
                      />
                      <span className="settings-toggle-slider" />
                    </label>
                  </div>
                </div>

                {p.hasCredentials && isExpanded && (
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
                            onChange={(e) => updateField(f.field as any, e.target.value)}
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
            );
          })}
        </div>
      </section>
    </div>
  )
}
