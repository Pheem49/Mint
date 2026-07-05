import React, { useState, useEffect } from 'react'
import { DEFAULT_CONFIG } from '../../components/SettingsWindow'
import { listLearnedSkills, addLearnedSkill, deleteLearnedSkill, LearnedSkill, detectSystemTools, DetectedTools } from '../../../src/tauri'

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

  // Load skills and detect tools on mount
  useEffect(() => {
    fetchSkills()
    detectTools()
  }, [])

  const detectTools = async () => {
    setDetecting(true)
    try {
      const tools = await detectSystemTools()
      setDetectedTools(tools)
    } catch (e) {
      console.error("Failed to detect tools:", e)
    } finally {
      setDetecting(false)
    }
  }

  const handleEnableTool = (name: string, command: string, args: string[]) => {
    const updatedMcp = {
      ...(config.mcpServers || {}),
      [name]: {
        command,
        args,
        env: {}
      }
    }
    updateField('mcpServers', updatedMcp)
  }

  const fetchSkills = async () => {
    try {
      const activeWorkspace = window.localStorage.getItem('mint:last-workspace-path') || undefined
      const list = await listLearnedSkills(activeWorkspace)
      setSkills(list)
    } catch (e) {
      console.error("Failed to load skills:", e)
    }
  }

  const handleAddSkill = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!newSkillName.trim() || !newSkillContent.trim()) {
      setSkillsError('Please fill in both name and content.')
      return
    }

    const cleanName = newSkillName.trim().replace(/\s+/g, '-').toLowerCase()
    setSkillsLoading(true)
    setSkillsError('')

    try {
      await addLearnedSkill(cleanName, newSkillContent)
      setNewSkillName('')
      setNewSkillContent('')
      await fetchSkills()
    } catch (err: any) {
      setSkillsError(err.message || String(err))
    } finally {
      setSkillsLoading(false)
    }
  }

  const handleDeleteSkill = async (name: string) => {
    if (confirm(`Forget skill "${name}"?`)) {
      try {
        await deleteLearnedSkill(name)
        await fetchSkills()
      } catch (e) {
        console.error("Failed to delete skill:", e)
      }
    }
  }

  const [expandedPlugin, setExpandedPlugin] = useState<string | null>(null)

  const toggleExpand = (pluginKey: string) => {
    setExpandedPlugin(expandedPlugin === pluginKey ? null : pluginKey)
  }

  const pluginsList: Array<{
    key: string
    name: string
    desc: string
    icon: string
    enabledField: 'pluginSpotifyEnabled' | 'pluginDiscordEnabled'
    hasCredentials: boolean
    fields?: Array<{ label: string; field: any; type: string; placeholder: string }>
  }> = [
    {
      key: 'spotify',
      name: 'Spotify',
      desc: 'Control playback with AI. Requires playerctl locally.',
      icon: '🎵',
      enabledField: 'pluginSpotifyEnabled' as const,
      hasCredentials: false
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

  const mcpListItems: Array<{
    name: string
    command: string
    args: string[]
    icon: string
    isEnabled: boolean
    isConfigured: boolean
    description: string
  }> = []

  Object.entries(config.mcpServers || {}).forEach(([name, srv]: [string, any]) => {
    let icon = '⚙';
    if (name === 'docker') icon = '🐳';
    else if (name === 'git' || name === 'gitkraken') icon = '🐙';
    else if (name === 'github') icon = '🐱';
    else if (name === 'node') icon = '🟢';

    mcpListItems.push({
      name,
      command: srv.command,
      args: srv.args || [],
      icon,
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
  if (detectedTools.gh && !config.mcpServers?.github) {
    mcpListItems.push({
      name: 'github',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-github'],
      icon: '🐱',
      isEnabled: false,
      isConfigured: false,
      description: 'GitHub MCP Server (Discovered)'
    });
  }
  if (detectedTools.node && !config.mcpServers?.node) {
    mcpListItems.push({
      name: 'node',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-node'],
      icon: '🟢',
      isEnabled: false,
      isConfigured: false,
      description: 'NodeJS Runtime MCP Server (Discovered)'
    });
  }

  return (
    <div className="tab-pane active" style={{ display: 'flex', flexDirection: 'column', gap: '24px' }}>
      {/* ── Learned AI Skills ── */}
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Knowledge Base</p>
            <h2 className="section-title">Learned AI Skills</h2>
          </div>
          <p className="section-description">
            Skills are special instructions or guides taught to Mint (equivalent to <code>mint learn</code> in CLI). The AI reads active skills before every prompt to align with your guidelines.
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
            No learned skills found. Teach Mint a skill below or run <code>/learn &lt;path&gt;</code> in chat!
          </div>
        ) : (
          <div className="skills-list" style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginBottom: '20px' }}>
            {skills.map((s) => (
              <div className="skill-card" key={s.id} style={{
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
                    {s.location && (
                      <span className="location-badge" style={{
                        background: s.location === 'workspace' 
                          ? 'rgba(16, 185, 129, 0.15)' 
                          : s.location === 'global' 
                            ? 'rgba(59, 130, 246, 0.15)' 
                            : 'rgba(139, 92, 246, 0.15)',
                        color: s.location === 'workspace' 
                          ? '#10b981' 
                          : s.location === 'global' 
                            ? '#3b82f6' 
                            : '#8b5cf6',
                        fontSize: '0.7rem',
                        fontWeight: '600',
                        padding: '2px 6px',
                        borderRadius: '4px',
                        textTransform: 'capitalize'
                      }}>
                        {s.location === 'database' ? 'Taught' : s.location}
                      </span>
                    )}
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-muted)', textOverflow: 'ellipsis', overflow: 'hidden', whiteSpace: 'nowrap' }}>
                      Source: {s.sourcePath}
                    </span>
                  </div>
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
              placeholder="# Instructions&#10;Write only clean TypeScript. Use async/await. Avoid let where const is possible."
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

      {/* ── External tools (MCP Servers) ── */}
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
            mcpListItems.map((item) => (
              <div 
                className="plugin-card" 
                key={item.name} 
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '16px 20px',
                  border: '1px solid var(--border)',
                  borderRadius: '12px',
                  background: item.isConfigured ? 'var(--surface-bg)' : 'rgba(16, 185, 129, 0.03)',
                  opacity: item.isConfigured ? 1 : 0.65,
                  marginBottom: '10px',
                  transition: 'all 0.2s'
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                  <div className="plugin-icon" style={{ fontSize: '1.5rem', width: '40px', height: '40px', display: 'grid', placeItems: 'center', background: 'var(--surface-strong)', borderRadius: '10px' }}>
                    {item.icon}
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
                <div className="plugin-actions" style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                  {item.isConfigured && (
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
            ))
          )}
        </div>

        {/* Save settings alert */}
        {((detectedTools.docker && !config.mcpServers?.docker) || 
          (detectedTools.git && !config.mcpServers?.git) || 
          (detectedTools.gh && !config.mcpServers?.github) || 
          (detectedTools.node && !config.mcpServers?.node)) && (
          <div style={{ marginTop: '12px', padding: '12px 16px', background: 'rgba(255,193,7,0.1)', border: '1px solid rgba(255,193,7,0.3)', borderRadius: '8px', fontSize: '0.8rem', color: '#ffb300', fontWeight: '500', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span>⚠️ After toggling a plugin, please click the "Save Settings" button at the bottom of the window to persist changes.</span>
          </div>
        )}

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

      {/* ── Built-in Plugins ── */}
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
                    {p.hasCredentials && (
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
                            // Don't auto-collapse config on disable so user can still edit
                          }
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
            );
          })}
        </div>
      </section>
    </div>
  )
}

