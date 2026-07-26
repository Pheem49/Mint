import React, { useState, useEffect } from 'react'
import { DEFAULT_CONFIG } from '../SettingsWindow'
import { listLearnedSkills, addLearnedSkill, deleteLearnedSkill, LearnedSkill, detectSystemTools, DetectedTools } from '../../tauri'

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

function renderMcpSvgIcon(name: string, customSvgOrUrl?: string) {
  if (customSvgOrUrl && customSvgOrUrl.trim()) {
    const trimmed = customSvgOrUrl.trim()
    if (trimmed.startsWith('<svg') || trimmed.includes('<svg')) {
      return (
        <span 
          style={{ width: '22px', height: '22px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
          dangerouslySetInnerHTML={{ __html: trimmed }} 
        />
      )
    }
    if (trimmed.startsWith('data:image') || trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
      return <img src={trimmed} alt={name} style={{ width: '22px', height: '22px', objectFit: 'contain' }} />
    }
    const cleanCustom = trimmed.toLowerCase()
    if (cleanCustom === 'search') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#3b82f6" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
      )
    }
    if (cleanCustom === 'database') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#a855f7" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <ellipse cx="12" cy="5" rx="9" ry="3"/>
          <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
          <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
        </svg>
      )
    }
    if (cleanCustom === 'cloud') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#38bdf8" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z"/>
        </svg>
      )
    }
    if (cleanCustom === 'code' || cleanCustom === 'terminal') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#10b981" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="16 18 22 12 16 6" />
          <polyline points="8 6 2 12 8 18" />
        </svg>
      )
    }
    if (cleanCustom === 'api' || cleanCustom === 'bolt') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
        </svg>
      )
    }
  }

  const cleanName = name.toLowerCase().trim()
  if (cleanName === 'docker') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#0db7ed">
        <path d="M13 8.5h2v2h-2zm-3 0h2v2h-2zm-3 0h2v2H7zm-3 3h2v2H4zm3 0h2v2H7zm3 0h2v2h-2zm3 0h2v2h-2zm3 0h2v2h-2zm-12 3h2v2H4zm3 0h2v2H7zm3 0h2v2h-2zm3 0h2v2h-2zm3 0h2v2h-2zm-6.2 3.8c-.8.5-2.1.8-3.4.6-2.5-.3-4.5-2.2-4.8-4.7h-1c.4 3.4 3.1 6 6.5 6 3 0 5.6-2 6.4-4.8.6-.2 1.4-.4 2.2-.2.3.1.6.3.8.5.6.6 1.4.9 2.2.9h.4c.5-.7.8-1.5.8-2.4 0-.3 0-.6-.1-.9-.7.1-1.4 0-2.1-.3-.6-.3-1.1-.8-1.5-1.4l-.2-.3h-2.1c-.2.7-.6 1.3-1.2 1.7-.6.4-1.3.6-2 .4z"/>
      </svg>
    )
  }
  if (cleanName === 'git' || cleanName === 'gitkraken') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#f05032" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="18" cy="18" r="3" />
        <circle cx="6" cy="6" r="3" />
        <circle cx="6" cy="18" r="3" />
        <path d="M6 9v6" />
        <path d="M9 18h6" />
      </svg>
    )
  }
  if (cleanName === 'github') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#f0f6fc">
        <path fillRule="evenodd" clipRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"/>
      </svg>
    )
  }
  if (cleanName === 'node' || cleanName === 'nodejs') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#68a063">
        <path d="M12 2L2 7.5v9L12 22l10-5.5v-9L12 2zm0 2.3l7.7 4.2v7L12 19.7 4.3 15.5v-7L12 4.3zm-1 4.2v3.5l-3-1.7V8.5l3 1.7zm2 0l3-1.7v1.8l-3 1.7V8.5z"/>
      </svg>
    )
  }
  if (cleanName === 'spotify') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#1ed760">
        <path d="M12 2C6.477 2 2 6.477 2 12s4.477 10 10 10 10-4.477 10-10S17.523 2 12 2zm4.586 14.424c-.18.295-.563.387-.857.207-2.35-1.434-5.308-1.758-8.793-.963-.335.077-.67-.133-.746-.467-.077-.334.132-.67.467-.746 3.812-.871 7.102-.494 9.722 1.112.294.18.386.563.207.857zm1.233-2.743c-.226.367-.706.482-1.073.257-2.687-1.652-6.785-2.131-9.965-1.166-.413.126-.847-.106-.973-.519-.125-.413.106-.847.519-.973 3.632-1.102 8.147-.568 11.235 1.328.367.226.482.706.257 1.073zm.105-2.835C14.692 8.95 9.375 8.775 6.297 9.71c-.496.15-1.022-.132-1.173-.628-.15-.496.132-1.022.628-1.173 3.535-1.073 9.404-.871 12.984 1.254.446.265.592.844.327 1.29-.265.446-.844.592-1.29.327z"/>
      </svg>
    )
  }
  if (cleanName === 'discord') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#5865f2">
        <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994.021-.041.001-.09-.041-.106a13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.061 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.893.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.028zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
      </svg>
    )
  }
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#10b981" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2" width="20" height="8" rx="2" ry="2"/>
      <rect x="2" y="14" width="20" height="8" rx="2" ry="2"/>
      <line x1="6" y1="6" x2="6.01" y2="6"/>
      <line x1="6" y1="18" x2="6.01" y2="18"/>
    </svg>
  )
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

  const [expandedMcp, setExpandedMcp] = useState<string | null>(null)

  const handleUpdateMcpServerField = (serverName: string, field: string, value: any) => {
    const currentMcp = config.mcpServers?.[serverName] || { command: 'npx', args: [], env: {} }
    const updatedMcp = {
      ...(config.mcpServers || {}),
      [serverName]: {
        ...currentMcp,
        [field]: value
      }
    }
    updateField('mcpServers', updatedMcp)
  }

  // Local state to toggle expansion of configuration sections
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
    customIcon?: string
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
                      {renderMcpSvgIcon(p.key)}
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
