import { Fragment, useState } from 'react'
import type { AgentActivityView, AgentActivity, AgentActivityGroup, TimelineItem } from '../utils/agentActivity'
import { groupActivities } from '../utils/agentActivity'
import { materialFileIcon, materialFolderIcon, getExtension } from '../utils/fileIcons'
import { InlineThoughtNote } from './InlineThoughtNote'
import { renderFormattedMessage } from '../utils/markdown'
import {
  Brain,
  Calculator,
  FileCode,
  Folder,
  Search,
  Terminal,
  Wrench,
  ChevronRight,
  Check,
  Loader2,
} from 'lucide-react'

interface Props {
  activityView: AgentActivityView
}

function getFilename(target: string): string {
  const clean = target.split(' #')[0].trim()
  const segments = clean.split('/')
  return segments[segments.length - 1]
}

function renderActivityIcon(kind: AgentActivity['kind'], target: string, action?: string) {
  if (kind === 'file') {
    const filename = getFilename(target)
    const ext = getExtension(filename)
    const icon = materialFileIcon(filename, ext)
    if (icon) return <img src={icon} alt="" draggable={false} className="agent-activity-icon-img" />
    return <FileCode size={14} className="agent-activity-kind-icon" />
  }
  if (kind === 'folder') {
    const foldername = getFilename(target)
    const icon = materialFolderIcon(foldername, false)
    if (icon) return <img src={icon} alt="" draggable={false} className="agent-activity-icon-img" />
    return <Folder size={14} className="agent-activity-kind-icon" />
  }
  if (kind === 'calc' || action === 'calculation') {
    return <Calculator size={14} className="agent-activity-kind-icon" />
  }
  if (kind === 'search') {
    return <Search size={14} className="agent-activity-kind-icon" />
  }
  if (kind === 'terminal') {
    return <Terminal size={14} className="agent-activity-kind-icon" />
  }
  return <Wrench size={14} className="agent-activity-kind-icon" />
}

function extractResultPreview(action: string | undefined, result: string | undefined): string | null {
  if (!result || !result.trim()) return null
  const firstLine = result.trim().split('\n')[0].trim()
  if (action === 'calculation') {
    const eqIdx = firstLine.indexOf('=')
    if (eqIdx !== -1) {
      const val = firstLine.slice(eqIdx + 1).trim()
      if (val.length > 0 && val.length < 30) return `→ ${val}`
    }
  }
  if (firstLine.length > 0 && firstLine.length <= 35 && !firstLine.startsWith('Error:') && !firstLine.startsWith('{')) {
    return `→ ${firstLine}`
  }
  return null
}

function renderStatusIndicator(state: AgentActivity['state']) {
  if (state === 'active') {
    return <Loader2 size={12} className="agent-activity-status-spin" />
  }
  if (state === 'retry') {
    return <span className="agent-activity-badge retry" title="Encountered an error but agent retried">Retried</span>
  }
  if (state === 'error') {
    return <span className="agent-activity-badge error" title="Action failed">Failed</span>
  }
  return <Check size={12} className="agent-activity-status-check" />
}

export function AgentActivityTable({ activityView }: Props) {
  const groups: AgentActivityGroup[] = activityView.groups || groupActivities(activityView.items)
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>({})
  const [expandedIndices, setExpandedIndices] = useState<Record<string, boolean>>({})

  const toggleGroup = (groupId: string) => {
    setExpandedGroups(prev => ({
      ...prev,
      [groupId]: !prev[groupId],
    }))
  }

  const toggleExpandItem = (key: string) => {
    setExpandedIndices(prev => ({
      ...prev,
      [key]: !prev[key],
    }))
  }

  const renderSingleItem = (activity: AgentActivity, itemKey: string) => {
    const isItemExpanded = Boolean(expandedIndices[itemKey])
    const output = activity.result?.trim()
    const inlineResult = extractResultPreview(activity.action, output)

    return (
      <Fragment key={itemKey}>
        <div
          className="agent-activity-item"
          data-kind={activity.kind}
          data-state={activity.state}
          style={{ cursor: output ? 'pointer' : 'default' }}
          onClick={() => output && toggleExpandItem(itemKey)}
        >
          <span className="agent-activity-icon-wrap" aria-hidden="true">
            {renderActivityIcon(activity.kind, activity.target, activity.action)}
          </span>
          <span className="agent-activity-label">
            <span className="agent-activity-tool-name">{activity.label || activity.action}</span>
          </span>
          <span
            className="agent-activity-text"
            title={activity.target}
            style={isItemExpanded ? { whiteSpace: 'normal', wordBreak: 'break-all', overflow: 'visible', textOverflow: 'clip' } : undefined}
          >
            <span className="agent-activity-target-text">{activity.target}</span>
            {inlineResult && <span className="agent-activity-inline-result">{inlineResult}</span>}
          </span>
          <span className="agent-activity-status-col">
            {renderStatusIndicator(activity.state)}
            {output && (
              <span
                className={`agent-activity-chevron${isItemExpanded ? ' is-open' : ''}`}
                aria-hidden="true"
              >
                <ChevronRight size={12} strokeWidth={2.2} />
              </span>
            )}
          </span>
        </div>
        {isItemExpanded && output && (
          <div className="agent-activity-output">
            <pre>{output}</pre>
          </div>
        )}
      </Fragment>
    )
  }

  const renderGroup = (group: AgentActivityGroup) => {
    const isGroupExpanded = Boolean(expandedGroups[group.id])

    return (
      <div key={group.id} className="agent-activity-group">
        <div
          className={`agent-activity-group-header${isGroupExpanded ? ' is-expanded' : ''}`}
          data-state={group.state}
          onClick={() => toggleGroup(group.id)}
        >
          <span className="agent-activity-icon-wrap" aria-hidden="true">
            {renderActivityIcon(group.kind, group.items[0]?.target, group.action)}
          </span>
          <span className="agent-activity-group-title">
            {group.title}
          </span>
          <span className="agent-activity-group-badges">
            <span className="agent-activity-count-badge">
              {group.count}
            </span>
            {renderStatusIndicator(group.state)}
            <span
              className={`agent-activity-chevron${isGroupExpanded ? ' is-open' : ''}`}
              aria-hidden="true"
            >
              <ChevronRight size={13} strokeWidth={2.2} />
            </span>
          </span>
        </div>

        {isGroupExpanded && (
          <div className="agent-activity-group-subitems">
            {group.items.map((activity, itemIdx) => {
              const itemKey = `${group.id}-${itemIdx}-${activity.target}`
              const isItemExpanded = Boolean(expandedIndices[itemKey])
              const output = activity.result?.trim()
              const inlineResult = extractResultPreview(activity.action, output)

              return (
                <Fragment key={itemKey}>
                  <div
                    className="agent-activity-subitem"
                    data-state={activity.state}
                    style={{ cursor: output ? 'pointer' : 'default' }}
                    onClick={() => output && toggleExpandItem(itemKey)}
                  >
                    <span className="agent-activity-tree-bullet" aria-hidden="true">
                      {itemIdx === group.items.length - 1 ? '└─' : '├─'}
                    </span>
                    <span
                      className="agent-activity-subitem-target"
                      title={activity.target}
                    >
                      {activity.target}
                    </span>
                    {inlineResult && (
                      <span className="agent-activity-inline-result">{inlineResult}</span>
                    )}
                    <span className="agent-activity-subitem-status">
                      {renderStatusIndicator(activity.state)}
                      {output && (
                        <span
                          className={`agent-activity-chevron${isItemExpanded ? ' is-open' : ''}`}
                          aria-hidden="true"
                        >
                          <ChevronRight size={11} strokeWidth={2.2} />
                        </span>
                      )}
                    </span>
                  </div>
                  {isItemExpanded && output && (
                    <div className="agent-activity-output agent-activity-subitem-output">
                      <pre>{output}</pre>
                    </div>
                  )}
                </Fragment>
              )
            })}
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="agent-activity-list">
      {activityView.timeline && activityView.timeline.length > 0
        ? activityView.timeline.map((item, idx) => {
            if (item.kind === 'thought') {
              return <InlineThoughtNote key={item.id || `thought-${idx}`} thought={item.thought} />
            }
            if (item.kind === 'extendedThinking') {
              const itemKey = item.id || `ext-thought-${idx}`
              const isThoughtExpanded = Boolean(expandedIndices[itemKey])
              return (
                <div key={itemKey} className="agent-activity-thought-step">
                  <div
                    className={`agent-activity-thought-header${isThoughtExpanded ? ' is-expanded' : ''}`}
                    onClick={() => toggleExpandItem(itemKey)}
                  >
                    <span className="agent-activity-icon-wrap" aria-hidden="true">
                      <Brain size={13} className="agent-activity-kind-icon" />
                    </span>
                    <span className="agent-activity-thought-title">
                      Thought
                    </span>
                    <span
                      className={`agent-activity-chevron${isThoughtExpanded ? ' is-open' : ''}`}
                      aria-hidden="true"
                    >
                      <ChevronRight size={12} strokeWidth={2.2} />
                    </span>
                  </div>
                  {isThoughtExpanded && (
                    <div className="agent-activity-thought-body">
                      {renderFormattedMessage(item.thought) ?? item.thought}
                    </div>
                  )}
                </div>
              )
            }
            if (item.kind === 'activity') {
              return renderSingleItem(item.activity, item.id || `act-${idx}`)
            }
            if (item.kind === 'group') {
              return renderGroup(item.group)
            }
            return null
          })
        : groups.map((group, groupIdx) => {
            if (group.count <= 1) {
              const activity = group.items[0]
              const itemKey = `${groupIdx}-0-${activity.label}-${activity.target}`
              return renderSingleItem(activity, itemKey)
            }
            return renderGroup(group)
          })}
    </div>
  )
}
