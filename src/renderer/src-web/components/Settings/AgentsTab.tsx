import React, { useState } from 'react'
import { 
  DEFAULT_CONFIG,
  GEMINI_MODELS,
  OPENAI_MODELS,
  OPENROUTER_MODELS,
  DEEPSEEK_MODELS,
  ANTHROPIC_MODELS,
  HF_MODELS,
  LOCAL_MODELS
} from '../SettingsWindow'

interface Agent {
  id: string
  name: string
  provider: string
  model: string
  apiKey?: string
  systemInstruction: string
  enabled: boolean
}

interface AgentsTabProps {
  config: typeof DEFAULT_CONFIG & { agents?: Agent[] }
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
    let defaultModel = ''
    if (newProvider === 'gemini') defaultModel = config.geminiModel || GEMINI_MODELS[0]
    else if (newProvider === 'openai') defaultModel = config.openaiModel || OPENAI_MODELS[0]
    else if (newProvider === 'anthropic') defaultModel = config.anthropicModel || ANTHROPIC_MODELS[0]
    else if (newProvider === 'openrouter') defaultModel = config.openrouterModel || OPENROUTER_MODELS[0]
    else if (newProvider === 'deepseek') defaultModel = config.deepseekModel || DEEPSEEK_MODELS[0]
    else if (newProvider === 'huggingface') defaultModel = config.hfModel || HF_MODELS[0]
    else if (newProvider === 'local_openai') defaultModel = config.localModelName || LOCAL_MODELS[0]
    else if (newProvider === 'ollama') defaultModel = config.ollamaModel || dynamicOllamaModels[0] || 'llama3:latest'
    setModel(defaultModel)
  }

  const handleEdit = (agent: Agent) => {
    setEditingAgent(agent)
    setName(agent.name)
    setProvider(agent.provider)
    setModel(agent.model)
    setApiKey(agent.apiKey || '')
    setSystemInstruction(agent.systemInstruction)
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
          ? { ...a, name, provider, model, apiKey: apiKey || undefined, systemInstruction }
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

  // Filter providers that have configured API keys, plus local ones
  const availableProviders = [
    { value: 'gemini', label: 'Google Gemini', hasKey: !!config.apiKey },
    { value: 'openai', label: 'OpenAI', hasKey: !!config.openaiApiKey },
    { value: 'anthropic', label: 'Anthropic Claude', hasKey: !!config.anthropicApiKey },
    { value: 'openrouter', label: 'OpenRouter', hasKey: !!config.openrouterApiKey },
    { value: 'deepseek', label: 'DeepSeek', hasKey: !!config.deepseekApiKey },
    { value: 'ollama', label: 'Ollama (Local)', hasKey: true },
    { value: 'huggingface', label: 'Hugging Face', hasKey: !!config.hfApiKey },
    { value: 'local_openai', label: 'Local (LM Studio / OpenAI Compatible)', hasKey: true },
  ].filter(p => p.hasKey);

  // Find out what models list corresponds to the selected provider
  let modelList: string[] = []
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
            <p className="section-description">
              Configure multiple distinct AI agents with specific roles. You can adjust the models, system instructions, and credentials for each agent.
            </p>
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
    </div>
  )
}
