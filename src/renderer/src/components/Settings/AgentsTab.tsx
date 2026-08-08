import React, { useEffect, useState } from 'react'
import {
  DEFAULT_CONFIG,
  GEMINI_MODELS,
  OPENAI_MODELS,
  OPENROUTER_MODELS,
  DEEPSEEK_MODELS,
  ANTHROPIC_MODELS,
  HF_MODELS,
  LOCAL_MODELS,
  CustomProviderConfig,
} from '../SettingsWindow'
import { listSubagents, saveSubagent, deleteSubagent, SubagentDefinition, SubagentDraft } from '../../tauri'

export interface Agent {
  id: string
  name: string
  provider: string
  model: string
  apiKey?: string
  systemInstruction?: string
  systemPrompt?: string
  enabled?: boolean
  avatar?: string
}

interface AgentsTabProps {
  config: typeof DEFAULT_CONFIG & { agents?: Agent[], customProviders?: CustomProviderConfig[] }
  updateField: (field: any, value: any) => void
  dynamicOllamaModels?: string[]
}

export default function AgentsTab({ config, updateField, dynamicOllamaModels = [] }: AgentsTabProps) {
  const agents = config.agents || []
  const [editingAgent, setEditingAgent] = useState<Agent | null>(null)
  const [name, setName] = useState('')
  const [provider, setProvider] = useState('gemini')
  const [model, setModel] = useState('gemini-2.5-flash')
  const [apiKey, setApiKey] = useState('')
  const [systemInstruction, setSystemInstruction] = useState('')
  const [isEditing, setIsEditing] = useState(false)

  const handleProviderChange = (newProvider: string) => {
    setProvider(newProvider)
    if (newProvider === 'gemini') setModel(config.geminiModel || GEMINI_MODELS[0])
    else if (newProvider === 'openai') setModel(config.openaiModel || OPENAI_MODELS[0])
    else if (newProvider === 'openrouter') setModel(config.openrouterModel || OPENROUTER_MODELS[0])
    else if (newProvider === 'deepseek') setModel(config.deepseekModel || DEEPSEEK_MODELS[0])
    else if (newProvider === 'anthropic') setModel(config.anthropicModel || ANTHROPIC_MODELS[0])
    else if (newProvider === 'huggingface') setModel(config.hfModel || HF_MODELS[0])
    else if (newProvider === 'local_openai') setModel(config.localModelName || LOCAL_MODELS[0])
    else if (newProvider === 'ollama') setModel(config.ollamaModel || dynamicOllamaModels[0] || '')
    else if (newProvider.startsWith('custom:')) {
      const cpId = newProvider.replace(/^custom:/, '')
      const cp = (config.customProviders ?? []).find(p => p.id === cpId)
      setModel(cp?.models[0]?.modelId ?? '')
    }
  }

  const handleEdit = (agent: Agent) => {
    setEditingAgent(agent)
    setName(agent.name)
    setProvider(agent.provider)
    setModel(agent.model)
    setApiKey(agent.apiKey || '')
    setSystemInstruction(agent.systemInstruction || agent.systemPrompt || '')
    setIsEditing(true)
  }

  const handleSaveAgent = () => {
    if (!name.trim()) {
      alert('Please enter an agent name.')
      return
    }

    let updatedAgents: Agent[] = []
    if (editingAgent) {
      // Update existing agent
      updatedAgents = agents.map(a =>
        a.id === editingAgent.id
          ? {
              ...a,
              name,
              provider,
              model,
              apiKey: apiKey || undefined,
              systemInstruction,
              systemPrompt: systemInstruction,
              enabled: a.enabled ?? true,
            }
          : a
      )
    } else {
      // Add new agent
      const newAgent: Agent = {
        id: `agent-${Date.now()}`,
        name,
        provider,
        model,
        apiKey: apiKey || undefined,
        systemInstruction,
        systemPrompt: systemInstruction,
        enabled: true
      }
      updatedAgents = [...agents, newAgent]
    }

    updateField('agents', updatedAgents)
    closeForm()
  }

  const handleDeleteAgent = (id: string) => {
    if (confirm('Are you sure you want to delete this agent?')) {
      const updatedAgents = agents.filter(a => a.id !== id)
      updateField('agents', updatedAgents)
    }
  }

  const handleToggleAgent = (id: string, enabled: boolean) => {
    const updatedAgents = agents.map(a =>
      a.id === id ? { ...a, enabled } : a
    )
    updateField('agents', updatedAgents)
  }

  const handleDisableAll = () => {
    const updatedAgents = agents.map(a => ({ ...a, enabled: false }))
    updateField('agents', updatedAgents)
  }

  const closeForm = () => {
    setEditingAgent(null)
    setName('')
    setProvider('gemini')
    setModel('gemini-2.5-flash')
    setApiKey('')
    setSystemInstruction('')
    setIsEditing(false)
  }

  const handleAddClick = () => {
    setEditingAgent(null)
    setName('')
    setProvider('gemini')
    setModel('gemini-2.5-flash')
    setApiKey('')
    setSystemInstruction('')
    setIsEditing(true)
  }

  // Available providers for agent routing
  const availableProviders = [
    { value: 'gemini', label: 'Google Gemini', hasKey: true },
    { value: 'openai', label: 'OpenAI', hasKey: !!config.openaiApiKey },
    { value: 'anthropic', label: 'Anthropic Claude', hasKey: !!config.anthropicApiKey },
    { value: 'openrouter', label: 'OpenRouter', hasKey: !!config.openrouterApiKey },
    { value: 'deepseek', label: 'DeepSeek', hasKey: !!config.deepseekApiKey },
    { value: 'huggingface', label: 'Hugging Face', hasKey: !!config.hfApiKey },
    { value: 'local_openai', label: 'Local OpenAI', hasKey: !!config.localApiBaseUrl },
    { value: 'ollama', label: 'Ollama (Local)', hasKey: !!config.ollamaHost },
    // Custom providers — available whenever they have a base URL configured
    ...(config.customProviders ?? [])
      .filter(cp => cp.baseUrl.trim().length > 0)
      .map(cp => ({
        value: `custom:${cp.id}`,
        label: `${cp.displayName || cp.id} (Custom)`,
        hasKey: true,
      })),
  ].filter(p => p.hasKey);

  // Find out what models list corresponds to the selected provider
  let modelList: readonly string[] = []
  let defaultGeneralModel = ''
  if (provider === 'gemini') {
    modelList = GEMINI_MODELS
    defaultGeneralModel = config.geminiModel
  } else if (provider === 'openai') {
    modelList = OPENAI_MODELS
    defaultGeneralModel = config.openaiModel
  } else if (provider === 'openrouter') {
    modelList = OPENROUTER_MODELS
    defaultGeneralModel = config.openrouterModel
  } else if (provider === 'deepseek') {
    modelList = DEEPSEEK_MODELS
    defaultGeneralModel = config.deepseekModel
  } else if (provider === 'anthropic') {
    modelList = ANTHROPIC_MODELS
    defaultGeneralModel = config.anthropicModel
  } else if (provider === 'huggingface') {
    modelList = HF_MODELS
    defaultGeneralModel = config.hfModel
  } else if (provider === 'local_openai') {
    modelList = LOCAL_MODELS
    defaultGeneralModel = config.localModelName
  } else if (provider === 'ollama') {
    modelList = dynamicOllamaModels
    defaultGeneralModel = config.ollamaModel
  } else if (provider.startsWith('custom:')) {
    const cpId = provider.replace(/^custom:/, '')
    const cp = (config.customProviders ?? []).find(p => p.id === cpId)
    modelList = cp?.models.map(m => m.modelId) ?? []
    defaultGeneralModel = modelList[0] ?? ''
  }

  // Sort modelList so defaultGeneralModel is first (at the top)
  const sortedModelList = defaultGeneralModel 
    ? [defaultGeneralModel, ...modelList.filter(m => m !== defaultGeneralModel)]
    : modelList;

  const isCustomModel = !sortedModelList.includes(model)

  return (
    <div className="tab-pane active animate-fade-in" style={{ display: 'flex', flexDirection: 'column', gap: '24px' }}>
      <section className="setting-section">
        <div className="section-heading" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div>
            <p className="section-kicker">Multi-Agent System</p>
            <h2 className="section-title">Agent Collaboration Settings</h2>
          </div>
          {!isEditing && (
            <div style={{ display: 'flex', gap: '10px', height: 'fit-content', flexShrink: 0 }}>
              <button className="btn btn-secondary" onClick={handleDisableAll} style={{ whiteSpace: 'nowrap' }}>
                Disable All
              </button>
              <button className="btn btn-primary" onClick={handleAddClick} style={{ whiteSpace: 'nowrap' }}>
                + Add Custom Agent
              </button>
            </div>
          )}
        </div>

        <div className="toggle-row" style={{ margin: '16px 0 24px 0' }}>
          <div>
            <label>Enable Multi-Agent Collaboration</label>
            <p className="hint">Allow multiple specialized agents to collaborate sequentially (Planner → Coder → Reviewer).</p>
          </div>
          <label className="settings-toggle-switch">
            <input 
              type="checkbox" 
              checked={config.enableAgentCollaboration} 
              onChange={(e) => updateField('enableAgentCollaboration', e.target.checked)} 
            />
            <span className="settings-toggle-slider"></span>
          </label>
        </div>

        {isEditing ? (
          <div className="form-grid single" style={{ background: 'var(--panel-soft)', padding: '20px', borderRadius: '8px', border: '1px solid var(--panel-raised)', marginTop: '10px' }}>
            <h3 style={{ margin: '0 0 16px 0', fontSize: '18px', color: 'var(--accent)' }}>
              {editingAgent ? `Edit Agent: ${editingAgent.name}` : 'Create New Agent'}
            </h3>
            
            <div className="setting-row">
              <label>Agent Name</label>
              <input 
                type="text" 
                value={name} 
                onChange={(e) => setName(e.target.value)} 
                placeholder="e.g. Code Reviewer" 
              />
            </div>

            <div className="setting-row">
              <label>AI Provider</label>
              <select value={provider} onChange={(e) => handleProviderChange(e.target.value)}>
                {availableProviders.map(p => (
                  <option key={p.value} value={p.value}>{p.label}</option>
                ))}
              </select>
            </div>

            <div className="setting-row">
              <label>Model Name</label>
              <select 
                value={isCustomModel ? 'custom' : model} 
                onChange={(e) => {
                  const val = e.target.value
                  if (val === 'custom') {
                    setModel('')
                  } else {
                    setModel(val)
                  }
                }}
              >
                {sortedModelList.map(m => (
                  <option key={m} value={m}>{m}</option>
                ))}
                <option value="custom">Custom...</option>
              </select>
            </div>

            {isCustomModel && (
              <div className="setting-row">
                <label>Custom Model Name</label>
                <input 
                  type="text" 
                  value={model} 
                  onChange={(e) => setModel(e.target.value)} 
                  placeholder="Enter custom model name..." 
                />
              </div>
            )}

            <div className="setting-row">
              <label>Custom API Key (Optional)</label>
              <input 
                type="password" 
                value={apiKey} 
                onChange={(e) => setApiKey(e.target.value)} 
                placeholder="Leave blank to use global provider API key" 
              />
            </div>

            <div className="setting-row">
              <label>System Instructions / Role</label>
              <textarea 
                value={systemInstruction} 
                onChange={(e) => setSystemInstruction(e.target.value)} 
                placeholder="Describe this agent's goal, instructions, and personality..." 
                style={{ minHeight: '120px', resize: 'vertical' }}
              />
            </div>

            <div style={{ display: 'flex', gap: '10px', marginTop: '16px', justifyContent: 'flex-end' }}>
              <button className="btn btn-secondary" onClick={closeForm}>Cancel</button>
              <button className="btn btn-primary" onClick={handleSaveAgent}>Save Agent</button>
            </div>
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', marginTop: '16px' }}>
            {agents.map((agent) => (
              <div 
                key={agent.id} 
                style={{ 
                  display: 'flex', 
                  flexDirection: 'column', 
                  padding: '16px', 
                  background: 'var(--panel-bg)', 
                  border: '1px solid var(--panel-raised)', 
                  borderRadius: '8px',
                  gap: '8px'
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    <strong style={{ fontSize: '18px' }}>{agent.name}</strong>
                    <span 
                      style={{ 
                        fontSize: '11px', 
                        padding: '2px 6px', 
                        borderRadius: '4px', 
                        background: 'var(--panel-raised)',
                        color: 'var(--accent)'
                      }}
                    >
                      {agent.provider} ({agent.model})
                    </span>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
                    <label className="settings-toggle-switch">
                      <input 
                        type="checkbox" 
                        checked={agent.enabled} 
                        onChange={(e) => handleToggleAgent(agent.id, e.target.checked)} 
                      />
                      <span className="settings-toggle-slider"></span>
                    </label>
                    <button className="btn btn-secondary" style={{ padding: '4px 8px', fontSize: '12px' }} onClick={() => handleEdit(agent)}>Edit</button>
                    {!['planner', 'coder', 'reviewer'].includes(agent.id) && (
                      <button className="btn btn-danger" style={{ padding: '4px 8px', fontSize: '12px' }} onClick={() => handleDeleteAgent(agent.id)}>Delete</button>
                    )}
                  </div>
                </div>
                <p style={{ margin: '4px 0 0 0', fontSize: '14px', color: 'var(--text-soft)', fontStyle: 'italic' }}>
                  "{agent.systemInstruction}"
                </p>
              </div>
            ))}
            {agents.length === 0 && (
              <div style={{ textAlign: 'center', padding: '32px', color: 'var(--text-soft)', fontStyle: 'italic' }}>
                No agents configured. Click "+ Add Custom Agent" to get started.
              </div>
            )}
          </div>
        )}
      </section>

      <SubagentsSection />
    </div>
  )
}

// The global directory is the uniquely-named `mint-agents` folder segment; anything else
// is workspace-scoped `.agents/subagents/`. Checking for that one segment name (rather than
// the two-segment `.agents/subagents` path) avoids depending on the OS path separator.
const isWorkspaceScoped = (sourcePath: string) => !sourcePath.includes('mint-agents')

const emptySubagentForm = {
  name: '',
  description: '',
  tools: '',
  model: '',
  provider: '',
  systemPrompt: '',
  scope: 'workspace' as 'workspace' | 'global'
}

function SubagentsSection() {
  const [subagents, setSubagents] = useState<SubagentDefinition[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [isEditing, setIsEditing] = useState(false)
  const [editingSourcePath, setEditingSourcePath] = useState<string | null>(null)
  const [form, setForm] = useState(emptySubagentForm)

  const workspacePath = typeof window !== 'undefined'
    ? window.localStorage.getItem('mint:last-workspace-path') || undefined
    : undefined

  const refresh = async () => {
    setLoading(true)
    setError('')
    try {
      setSubagents(await listSubagents(workspacePath))
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load subagents.')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    refresh()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const closeForm = () => {
    setIsEditing(false)
    setEditingSourcePath(null)
    setForm(emptySubagentForm)
  }

  const handleAddClick = () => {
    setEditingSourcePath(null)
    setForm({ ...emptySubagentForm, scope: workspacePath ? 'workspace' : 'global' })
    setIsEditing(true)
  }

  const handleEdit = (subagent: SubagentDefinition) => {
    setEditingSourcePath(subagent.sourcePath)
    setForm({
      name: subagent.name,
      description: subagent.description,
      tools: subagent.tools?.join(', ') ?? '',
      model: subagent.model ?? '',
      provider: subagent.provider ?? '',
      systemPrompt: subagent.systemPrompt,
      scope: isWorkspaceScoped(subagent.sourcePath) ? 'workspace' : 'global'
    })
    setIsEditing(true)
  }

  const handleSave = async () => {
    if (!form.name.trim()) {
      alert('Please enter a subagent name.')
      return
    }
    if (!form.systemPrompt.trim()) {
      alert('Please describe what this subagent should do (its system prompt).')
      return
    }
    const draft: SubagentDraft = {
      name: form.name.trim(),
      description: form.description.trim(),
      tools: form.tools.trim()
        ? form.tools.split(',').map(t => t.trim()).filter(Boolean)
        : null,
      model: form.model.trim() || null,
      provider: form.provider.trim() || null,
      systemPrompt: form.systemPrompt,
      scope: form.scope,
      previousSourcePath: editingSourcePath
    }
    try {
      await saveSubagent(draft, workspacePath)
      closeForm()
      await refresh()
    } catch (e) {
      alert(e instanceof Error ? e.message : 'Failed to save subagent.')
    }
  }

  const handleDelete = async (subagent: SubagentDefinition) => {
    if (!confirm(`Delete subagent "${subagent.name}"?`)) return
    try {
      await deleteSubagent(subagent.sourcePath)
      await refresh()
    } catch (e) {
      alert(e instanceof Error ? e.message : 'Failed to delete subagent.')
    }
  }

  return (
    <section className="setting-section">
      <div className="section-heading" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <p className="section-kicker">dispatch_subagent</p>
          <h2 className="section-title">Subagents</h2>
        </div>
        {!isEditing && (
          <button className="btn btn-primary" onClick={handleAddClick} style={{ whiteSpace: 'nowrap' }}>
            + Add Subagent
          </button>
        )}
      </div>
      <p className="hint" style={{ margin: '4px 0 16px 0' }}>
        Focused workers the agent can delegate a sub-task to mid-conversation. Each runs its own isolated
        conversation — only its final summary comes back, so exploring, reading files, or running tools doesn't
        bloat the main context.
      </p>

      {isEditing ? (
        <div className="form-grid single" style={{ background: 'var(--panel-soft)', padding: '20px', borderRadius: '8px', border: '1px solid var(--panel-raised)' }}>
          <h3 style={{ margin: '0 0 16px 0', fontSize: '18px', color: 'var(--accent)' }}>
            {editingSourcePath ? `Edit Subagent: ${form.name}` : 'Create New Subagent'}
          </h3>

          <div className="setting-row">
            <label>Name</label>
            <input
              type="text"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="e.g. code-reviewer"
            />
          </div>

          <div className="setting-row">
            <label>Description</label>
            <input
              type="text"
              value={form.description}
              onChange={(e) => setForm({ ...form, description: e.target.value })}
              placeholder="One line: when should the agent reach for this subagent?"
            />
          </div>

          <div className="setting-row">
            <label>System Prompt</label>
            <textarea
              value={form.systemPrompt}
              onChange={(e) => setForm({ ...form, systemPrompt: e.target.value })}
              placeholder="You are a focused code-review subagent. Read the diff and report bugs concisely..."
              style={{ minHeight: '120px', resize: 'vertical' }}
            />
          </div>

          <div className="setting-row">
            <label>Allowed Tools (optional)</label>
            <input
              type="text"
              value={form.tools}
              onChange={(e) => setForm({ ...form, tools: e.target.value })}
              placeholder="e.g. read_file, search_code, git_diff (leave blank for the full toolset)"
            />
          </div>

          <div className="setting-row">
            <label>Model Override (optional)</label>
            <input
              type="text"
              value={form.model}
              onChange={(e) => setForm({ ...form, model: e.target.value })}
              placeholder="Leave blank to use the active model"
            />
          </div>

          <div className="setting-row">
            <label>Provider Override (optional)</label>
            <input
              type="text"
              value={form.provider}
              onChange={(e) => setForm({ ...form, provider: e.target.value })}
              placeholder="e.g. gemini, openai, anthropic — leave blank to use the active provider"
            />
          </div>

          <div className="setting-row">
            <label>Scope</label>
            <select value={form.scope} onChange={(e) => setForm({ ...form, scope: e.target.value as 'workspace' | 'global' })}>
              <option value="workspace" disabled={!workspacePath}>
                This workspace only ({'.agents/subagents/'}){!workspacePath ? ' — no workspace open' : ''}
              </option>
              <option value="global">All workspaces (global)</option>
            </select>
          </div>

          <div style={{ display: 'flex', gap: '10px', marginTop: '16px', justifyContent: 'flex-end' }}>
            <button className="btn btn-secondary" onClick={closeForm}>Cancel</button>
            <button className="btn btn-primary" onClick={handleSave}>Save Subagent</button>
          </div>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          {error && <div style={{ color: 'var(--danger, #ef4444)' }}>{error}</div>}
          {loading ? (
            <div style={{ textAlign: 'center', padding: '32px', color: 'var(--text-soft)' }}>Loading...</div>
          ) : (
            <>
              {subagents.map((subagent) => (
                <div
                  key={subagent.sourcePath}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    padding: '16px',
                    background: 'var(--panel-bg)',
                    border: '1px solid var(--panel-raised)',
                    borderRadius: '8px',
                    gap: '8px'
                  }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                      <strong style={{ fontSize: '18px' }}>{subagent.name}</strong>
                      <span
                        style={{
                          fontSize: '11px',
                          padding: '2px 6px',
                          borderRadius: '4px',
                          background: isWorkspaceScoped(subagent.sourcePath) ? 'rgba(16, 185, 129, 0.15)' : 'rgba(59, 130, 246, 0.15)',
                          color: isWorkspaceScoped(subagent.sourcePath) ? '#10b981' : '#3b82f6',
                        }}
                      >
                        {isWorkspaceScoped(subagent.sourcePath) ? 'Workspace' : 'Global'}
                      </span>
                      {(subagent.provider || subagent.model) && (
                        <span style={{ fontSize: '11px', padding: '2px 6px', borderRadius: '4px', background: 'var(--panel-raised)', color: 'var(--accent)' }}>
                          {[subagent.provider, subagent.model].filter(Boolean).join(' / ')}
                        </span>
                      )}
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                      <button className="btn btn-secondary" style={{ padding: '4px 8px', fontSize: '12px' }} onClick={() => handleEdit(subagent)}>Edit</button>
                      <button className="btn btn-danger" style={{ padding: '4px 8px', fontSize: '12px' }} onClick={() => handleDelete(subagent)}>Delete</button>
                    </div>
                  </div>
                  {subagent.description && (
                    <p style={{ margin: '4px 0 0 0', fontSize: '14px', color: 'var(--text-soft)', fontStyle: 'italic' }}>
                      "{subagent.description}"
                    </p>
                  )}
                  {subagent.tools && (
                    <p style={{ margin: 0, fontSize: '12px', color: 'var(--text-soft)' }}>
                      Tools: {subagent.tools.join(', ')}
                    </p>
                  )}
                </div>
              ))}
              {subagents.length === 0 && (
                <div style={{ textAlign: 'center', padding: '32px', color: 'var(--text-soft)', fontStyle: 'italic' }}>
                  No subagents yet. Click "+ Add Subagent" to create one.
                </div>
              )}
            </>
          )}
        </div>
      )}
    </section>
  )
}
