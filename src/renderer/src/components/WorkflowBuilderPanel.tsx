import React, { useState, useEffect, useCallback, useMemo } from 'react'
import {
  ReactFlow,
  MiniMap,
  Controls,
  Background,
  useNodesState,
  useEdgesState,
  addEdge,
  type Node,
  type Edge,
  Position,
  Handle
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import type { DashboardView } from './DashboardSidebar'

interface WorkflowBuilderPanelProps {
  view: DashboardView
  onShowToast?: (message: string) => void
}

interface WorkflowTrigger {
  type: string
  processName: string
}

interface WorkflowAction {
  type: string
  message: string
  target: string
}

interface Workflow {
  id: string
  name: string
  trigger: WorkflowTrigger
  action: WorkflowAction
}

/* Custom Node: Trigger Node */
interface TriggerNodeData {
  processName: string
  onChangeProcessName: (value: string) => void
}

function TriggerNode({ data }: { data: TriggerNodeData }) {
  return (
    <div className="custom-node trigger-node">
      <div className="node-header">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="12" y1="8" x2="12" y2="12"></line>
          <line x1="12" y1="16" x2="12.01" y2="16"></line>
        </svg>
        <span className="node-title">Trigger: Process Running</span>
      </div>
      <div className="node-body">
        <label>Process Name</label>
        <input
          type="text"
          className="node-input"
          value={data.processName || ''}
          onChange={(e) => data.onChangeProcessName(e.target.value)}
          placeholder="e.g., code, zoom, chrome, steam, figma"
        />
        <span className="field-hint">
          💡 Enter the process executable name <strong>without the .exe</strong> (e.g., <code>code</code> for VS Code, <code>zoom</code>, <code>figma</code>, <code>spotify</code>).
        </span>
      </div>
      <Handle type="source" position={Position.Right} id="trigger-out" />
    </div>
  )
}

/* Custom Node: Action Node */
interface ActionNodeData {
  type: string
  message: string
  target: string
  onChangeType: (value: string) => void
  onChangeMessage: (value: string) => void
  onChangeTarget: (value: string) => void
}

function ActionNode({ data }: { data: ActionNodeData }) {
  const getTargetLabel = (type: string) => {
    switch (type) {
      case 'open_app': return 'App Name'
      case 'open_url': return 'Web URL'
      case 'run_command': return 'Shell Command'
      default: return 'Target'
    }
  }

  const getTargetPlaceholder = (type: string) => {
    switch (type) {
      case 'open_app': return 'e.g., spotify, slack, discord'
      case 'open_url': return 'e.g., https://github.com'
      case 'run_command': return 'e.g., npm run dev'
      default: return ''
    }
  }

  const getTargetHint = (type: string) => {
    switch (type) {
      case 'open_app': return <span>💡 Enter the application name, e.g., <code>spotify</code> or <code>discord</code>.</span>
      case 'open_url': return <span>💡 Enter the target website URL, e.g., <code>https://google.com</code>.</span>
      case 'run_command': return <span>💡 Enter a custom terminal command, e.g., <code>git pull</code>.</span>
      default: return null
    }
  }

  return (
    <div className="custom-node action-node">
      <Handle type="target" position={Position.Left} id="action-in" />
      <div className="node-header">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline>
        </svg>
        <span className="node-title">Action: Proactive Assistant Action</span>
      </div>
      <div className="node-body">
        <label>Action Type</label>
        <select
          className="node-select"
          value={data.type || 'system_info'}
          onChange={(e) => data.onChangeType(e.target.value)}
        >
          <option value="system_info">System Info</option>
          <option value="open_app">Open Application</option>
          <option value="open_url">Open URL / Website</option>
        </select>

        <label>Interactive Prompt</label>
        <input
          type="text"
          className="node-input"
          value={data.message || ''}
          onChange={(e) => data.onChangeMessage(e.target.value)}
          placeholder="e.g., Zoom is running. Do you want me to check system resources?"
        />
        <span className="field-hint">
          💡 The text the assistant will show as a pop-up on your desktop. We recommend ending with a question.
        </span>

        {data.type !== 'system_info' && (
          <>
            <label style={{ marginTop: '8px', display: 'block' }}>{getTargetLabel(data.type)}</label>
            <input
              type="text"
              className="node-input"
              value={data.target || ''}
              onChange={(e) => data.onChangeTarget(e.target.value)}
              placeholder={getTargetPlaceholder(data.type)}
            />
            <span className="field-hint">
              {getTargetHint(data.type)}
            </span>
          </>
        )}
      </div>
    </div>
  )
}

export default function WorkflowBuilderPanel({ view, onShowToast }: WorkflowBuilderPanelProps) {
  const [workflows, setWorkflows] = useState<Workflow[]>([])
  const [selectedWorkflowId, setSelectedWorkflowId] = useState<string | null>(null)
  const [showHelp, setShowHelp] = useState(true)
  const [accentColor, setAccentColor] = useState('#10b981')

  useEffect(() => {
    const style = getComputedStyle(document.documentElement)
    const accent = style.getPropertyValue('--accent').trim()
    if (accent) {
      setAccentColor(accent)
    }
  }, [view])
  
  const [nodes, setNodes, onNodesChange] = useNodesState([])
  const [edges, setEdges, onEdgesChange] = useEdgesState([])

  const nodeTypes = useMemo(() => ({
    triggerNode: TriggerNode,
    actionNode: ActionNode
  }), [])

  // Load workflows on mount
  const loadWorkflows = async () => {
    try {
      if (window.settingsApi) {
        const res = await window.settingsApi.reloadCustomWorkflows()
        if (res && res.workflows) {
          setWorkflows(res.workflows)
          if (res.workflows.length > 0 && !selectedWorkflowId) {
            setSelectedWorkflowId(res.workflows[0].id)
          }
        }
      }
    } catch (e) {
      console.error("Failed to load workflows:", e)
      onShowToast?.("Failed to load workflows ⚠️")
    }
  }

  useEffect(() => {
    if (view === 'workflows') {
      loadWorkflows()
    }
  }, [view])

  const selectedWorkflow = useMemo(() => {
    return workflows.find(w => w.id === selectedWorkflowId) || null
  }, [workflows, selectedWorkflowId])

  // Update backend workflows file
  const handleSave = async () => {
    try {
      if (window.settingsApi) {
        const res = await window.settingsApi.saveCustomWorkflows(workflows)
        if (res && res.success) {
          onShowToast?.("Workflows saved successfully! 💾")
          // Reload rules in backend monitor
          await window.settingsApi.reloadCustomWorkflows()
        } else {
          onShowToast?.("Failed to save workflows ⚠️")
        }
      }
    } catch (e) {
      console.error("Failed to save workflows:", e)
      onShowToast?.("Error saving workflows ⚠️")
    }
  }

  const handleReloadRules = async () => {
    try {
      if (window.settingsApi) {
        const res = await window.settingsApi.reloadCustomWorkflows()
        if (res && res.success) {
          onShowToast?.("Rules reloaded & active! 🔄")
        } else {
          onShowToast?.("Failed to reload rules ⚠️")
        }
      }
    } catch (e) {
      console.error("Failed to reload rules:", e)
      onShowToast?.("Error reloaded rules ⚠️")
    }
  }

  // Handle updates to node data
  const updateSelectedWorkflow = useCallback((updater: (w: Workflow) => Workflow) => {
    if (!selectedWorkflowId) return
    setWorkflows(current => current.map(w => {
      if (w.id === selectedWorkflowId) {
        return updater(w)
      }
      return w
    }))
  }, [selectedWorkflowId])

  // Map selected workflow to node/edge graph
  useEffect(() => {
    if (!selectedWorkflow) {
      setNodes([])
      setEdges([])
      return
    }

    const tNode: Node = {
      id: 'node-trigger',
      type: 'triggerNode',
      position: { x: 60, y: 140 },
      data: {
        processName: selectedWorkflow.trigger.processName,
        onChangeProcessName: (val: string) => {
          updateSelectedWorkflow(w => ({
            ...w,
            trigger: { ...w.trigger, processName: val }
          }))
        }
      }
    }

    const aNode: Node = {
      id: 'node-action',
      type: 'actionNode',
      position: { x: 420, y: 100 },
      data: {
        type: selectedWorkflow.action.type,
        message: selectedWorkflow.action.message,
        target: selectedWorkflow.action.target,
        onChangeType: (val: string) => {
          updateSelectedWorkflow(w => ({
            ...w,
            action: { ...w.action, type: val }
          }))
        },
        onChangeMessage: (val: string) => {
          updateSelectedWorkflow(w => ({
            ...w,
            action: { ...w.action, message: val }
          }))
        },
        onChangeTarget: (val: string) => {
          updateSelectedWorkflow(w => ({
            ...w,
            action: { ...w.action, target: val }
          }))
        }
      }
    }

    const connectionEdge: Edge = {
      id: 'edge-main',
      source: 'node-trigger',
      target: 'node-action',
      sourceHandle: 'trigger-out',
      targetHandle: 'action-in',
      animated: true
    }

    setNodes([tNode, aNode])
    setEdges([connectionEdge])
  }, [selectedWorkflow, updateSelectedWorkflow])

  const handleAddNewWorkflow = () => {
    const nextId = `wf-${Date.now()}`
    const newWf: Workflow = {
      id: nextId,
      name: `Launch music when coding (Example)`,
      trigger: {
        type: 'process_running',
        processName: 'code'
      },
      action: {
        type: 'open_app',
        message: 'You started coding. Do you want me to launch Spotify? 🎵',
        target: 'spotify'
      }
    }

    setWorkflows(current => [...current, newWf])
    setSelectedWorkflowId(nextId)
    onShowToast?.("New workflow created ➕")
  }

  const handleDeleteWorkflow = (id: string, e: React.MouseEvent) => {
    e.stopPropagation()
    if (confirm("Are you sure you want to delete this workflow?")) {
      setWorkflows(current => current.filter(w => w.id !== id))
      if (selectedWorkflowId === id) {
        const remaining = workflows.filter(w => w.id !== id)
        setSelectedWorkflowId(remaining.length > 0 ? remaining[0].id : null)
      }
      onShowToast?.("Workflow deleted 🗑️")
    }
  }

  return (
    <section className={`workflows-builder-container ${view === 'workflows' ? 'is-visible' : ''}`} aria-hidden={view !== 'workflows'}>
      {/* Sidebar - Master List */}
      <div className="workflows-sidebar">
        <div className="workflows-sidebar-header">
          <h3>Workflow (Beta)</h3>
          <button className="btn-workflows-add" onClick={handleAddNewWorkflow}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add Workflow
          </button>
        </div>
        <div className="workflows-list">
          {workflows.map(wf => (
            <div
              key={wf.id}
              className={`workflow-list-item ${selectedWorkflowId === wf.id ? 'active' : ''}`}
              onClick={() => setSelectedWorkflowId(wf.id)}
            >
              <div className="workflow-item-info">
                <span className="workflow-item-name">{wf.name || 'Unnamed Workflow'}</span>
                <span className="workflow-item-desc">
                  {wf.trigger.processName ? `Trigger: ${wf.trigger.processName}` : 'No trigger set'}
                </span>
              </div>
              <button
                className="workflow-delete-btn"
                title="Delete workflow"
                onClick={(e) => handleDeleteWorkflow(wf.id, e)}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="3 6 5 6 21 6"></polyline>
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                </svg>
              </button>
            </div>
          ))}
        </div>
      </div>

      {/* Editor Canvas */}
      <div className="workflows-canvas-panel">
        {selectedWorkflow ? (
          <>
            <header className="workflows-canvas-header">
              <div className="workflows-canvas-title">
                <input
                  type="text"
                  className="workflow-name-input"
                  value={selectedWorkflow.name || ''}
                  onChange={(e) => {
                    const name = e.target.value
                    updateSelectedWorkflow(w => ({ ...w, name }))
                  }}
                  placeholder="Workflow Name"
                />
              </div>
              <div className="workflows-canvas-actions">
                <button className="btn-help-toggle" onClick={() => setShowHelp(!showHelp)}>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="12" cy="12" r="10"></circle>
                    <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"></path>
                    <line x1="12" y1="17" x2="12.01" y2="17"></line>
                  </svg>
                  {showHelp ? "Close Guide" : "Quick Guide 💡"}
                </button>
                <button className="btn-reload-rules" onClick={handleReloadRules}>
                  Reload Rules
                </button>
                <button className="btn-save-workflows" onClick={handleSave}>
                  Save Workflows
                </button>
              </div>
            </header>

            <div className="workflows-canvas-wrapper">
              <div style={{ flex: 1, position: 'relative' }}>
                <ReactFlow
                  nodes={nodes}
                  edges={edges}
                  onNodesChange={onNodesChange}
                  onEdgesChange={onEdgesChange}
                  nodeTypes={nodeTypes}
                  fitView
                  fitViewOptions={{ padding: 0.3 }}
                  minZoom={0.5}
                  maxZoom={1.5}
                  nodesDraggable={true}
                  elementsSelectable={true}
                >
                  <Background color="#334155" gap={16} size={1} />
                  <Controls showInteractive={false} />
                  <MiniMap
                    style={{
                      background: 'rgba(15, 23, 42, 0.9)',
                      border: '1px solid rgba(148, 163, 184, 0.2)',
                      borderRadius: '8px'
                    }}
                    maskColor="rgba(30, 41, 59, 0.4)"
                    nodeColor={(n) => {
                      if (n.type === 'triggerNode') return accentColor
                      if (n.type === 'actionNode') return '#8b5cf6'
                      return '#334155'
                    }}
                  />
                </ReactFlow>
              </div>

              {showHelp && (
                <div className="workflow-help-panel">
                  <div className="workflow-help-header">
                    <h3>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <circle cx="12" cy="12" r="10"></circle>
                        <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"></path>
                        <line x1="12" y1="17" x2="12.01" y2="17"></line>
                      </svg>
                      Workflow Quick Guide
                    </h3>
                    <button className="workflow-help-close-btn" onClick={() => setShowHelp(false)} title="Close">
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <line x1="18" y1="6" x2="6" y2="18"></line>
                        <line x1="6" y1="6" x2="18" y2="18"></line>
                      </svg>
                    </button>
                  </div>
                  <div className="workflow-help-body">
                    <div className="help-section">
                      <h4>What is a Workflow?</h4>
                      <p>
                        Workflows let you automate tasks. When a monitored application starts (like Zoom or VS Code), your assistant will pop up on your desktop proposing a proactive action like launching another app, opening a website, or checking system specs.
                      </p>
                    </div>
                    <div className="help-section">
                      <h4>How to set up:</h4>
                      <ol className="help-steps">
                        <li>Click <strong>+ Add Workflow</strong> in the left sidebar.</li>
                        <li>In the green Trigger node 🟢:
                          <p style={{ margin: '4px 0 0 0', fontSize: '0.75rem', color: '#cbd5e1' }}>
                            Enter the executable name, e.g., <code>code</code>, <code>zoom</code>, <code>chrome</code> (do NOT include paths or .exe).
                          </p>
                        </li>
                        <li>In the purple Action node 🟣:
                          <p style={{ margin: '4px 0 0 0', fontSize: '0.75rem', color: '#cbd5e1' }}>
                            Choose your Action Type ➡️ Write the popup question in Interactive Prompt ➡️ Specify the target app name or URL.
                          </p>
                        </li>
                        <li>Click <strong>Save Workflows</strong> at the top-right to save.</li>
                        <li>Click <strong>Reload Rules</strong> to apply the rules immediately!</li>
                      </ol>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="workflows-empty-canvas">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M4 12h16"></path>
              <path d="M12 4v16"></path>
              <rect x="2" y="9" width="4" height="6" rx="1"></rect>
              <rect x="18" y="9" width="4" height="6" rx="1"></rect>
              <rect x="10" y="2" width="4" height="4" rx="1"></rect>
              <rect x="10" y="18" width="4" height="4" rx="1"></rect>
            </svg>
            <h4>No Workflow Selected</h4>
            <p>Select a workflow from the list or add a new one to begin editing visually.</p>
          </div>
        )}
      </div>
    </section>
  )
}
