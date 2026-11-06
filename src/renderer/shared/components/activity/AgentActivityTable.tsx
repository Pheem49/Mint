import type { ArtifactFile } from '../../utils/artifacts'
import { Fragment, useState } from 'react'
import type { AgentActivityView, AgentActivity, AgentActivityGroup, TimelineItem } from '../../utils/agentActivity'
import { cleanIntermediateThought, groupActivities } from '../../utils/agentActivity'
import { materialFileIcon, materialFolderIcon, getExtension } from '../../utils/fileIcons'
import { renderFormattedMessage } from '../../utils/markdown'
import { ViewedImage } from './ViewedImages'
import type { AgentProgress } from '../../types'
import McpToolResults from './McpToolResults'
import { setWorkspaceDragData, isImageFileName } from '../../utils/workspaceDrag'

export function InlineThoughtNote({ thought }: { thought: string }) {
  const cleaned = cleanIntermediateThought(thought)
  if (!cleaned) return null
  return (
    <div className="inline-thought-note">
      <div className="inline-thought-note-text">
        {renderFormattedMessage(cleaned) ?? cleaned}
      </div>
    </div>
  )
}
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
  progress?: AgentProgress[]
  chatId?: string
  workspacePath?: string
  onReferenceFile?: (filePath: string) => void
  onOpenArtifact?: (artifact: ArtifactFile) => void
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
  if (!result || !result.trim() || action === 'view_image') return null
  // Do not show inline raw text preview for file operations (filename and diff stats are already shown)
  if (action === 'read_file' || action === 'write_file' || action === 'edit_file' || action === 'apply_patch') return null
  const firstLine = result.trim().split('\n')[0].trim()
  // Filter out JSON brackets, objects, or errors
  if (firstLine.startsWith('[') || firstLine.startsWith('{') || firstLine.startsWith('Error:')) return null
  if (action === 'calculation') {
    const eqIdx = firstLine.indexOf('=')
    if (eqIdx !== -1) {
      const val = firstLine.slice(eqIdx + 1).trim()
      if (val.length > 0 && val.length < 30) return `→ ${val}`
    }
  }
  if (firstLine.length > 0 && firstLine.length <= 35) {
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

export function AgentActivityTable({ activityView, progress, chatId, workspacePath, onReferenceFile, onOpenArtifact }: Props) {
  const groups: AgentActivityGroup[] = activityView.groups || groupActivities(activityView.items)
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>({})
  const [expandedIndices, setExpandedIndices] = useState<Record<string, boolean>>({})

  const toggleGroup = (groupId: string) => {
    setExpandedGroups(prev => ({
      ...prev,
      [groupId]: !prev[groupId],
    }))
  }

  const toggleExpandItem = (key: string, defaultExpanded = false) => {
    setExpandedIndices(prev => ({
      ...prev,
      [key]: !(prev[key] ?? defaultExpanded),
    }))
  }

  const renderSingleItem = (activity: AgentActivity, itemKey: string) => {
    const isItemExpanded = Boolean(expandedIndices[itemKey])
    const output = activity.result?.trim()
    const inlineResult = extractResultPreview(activity.action, output)
    const isFileAction =
      activity.kind === 'file' ||
      activity.action === 'write_file' ||
      activity.action === 'edit_file' ||
      activity.action === 'read_file'
    const isCommand =
      activity.action === 'run_shell' ||
      activity.action === 'run_command' ||
      activity.action === 'bash' ||
      activity.kind === 'terminal'

    const cleanTarget = activity.target ? activity.target.split(' #')[0].trim() : ''
    const fileName = getFilename(cleanTarget)

    const verb =
      activity.semanticVerb ||
      (isCommand
        ? 'Ran'
        : isFileAction
          ? activity.action === 'read_file'
            ? 'Analyzed'
            : 'Edited'
          : activity.label || activity.action || 'Executed')

    return (
      <Fragment key={itemKey}>
        <div
          className={`agent-activity-item modern-timeline-row ${isItemExpanded ? 'is-expanded' : ''}`}
          data-kind={activity.kind}
          data-state={activity.state}
          style={{ cursor: output ? 'pointer' : 'default' }}
          onClick={() => output && toggleExpandItem(itemKey)}
        >
          <span className="modern-timeline-verb">{verb}</span>

          {isFileAction && (
            <span className="modern-timeline-icon-wrap" aria-hidden="true">
              {renderActivityIcon(activity.kind, cleanTarget, activity.action)}
            </span>
          )}

          <span
            className={`agent-activity-text modern-timeline-target ${isFileAction ? 'is-file-target' : ''}`}
            title={
              isFileAction
                ? `Drag file to chat or click @ to reference: ${cleanTarget}`
                : activity.target
            }
          >
            <span
              className={`agent-activity-target-text ${isFileAction ? 'is-file-target' : ''}`}
              draggable={isFileAction}
              onDragStart={
                isFileAction
                  ? (e) => {
                      e.stopPropagation()
                      setWorkspaceDragData(e.dataTransfer, {
                        path: cleanTarget,
                        name: fileName,
                        root: workspacePath,
                        kind: isImageFileName(cleanTarget) ? 'image' : 'file',
                      })
                    }
                  : undefined
              }
            >
              {isFileAction ? fileName : (activity.intentTitle || activity.target)}
            </span>

            {activity.lineRange && (
              <span className="modern-timeline-line-range">{activity.lineRange}</span>
            )}

            {activity.diffStats &&
              (activity.diffStats.additions > 0 || activity.diffStats.deletions > 0) && (
                <span className="modern-timeline-diff-stats">
                  {activity.diffStats.additions > 0 && (
                    <span className="diff-stat-add">+{activity.diffStats.additions}</span>
                  )}
                  {activity.diffStats.deletions > 0 && (
                    <span className="diff-stat-del">-{activity.diffStats.deletions}</span>
                  )}
                </span>
              )}

            {inlineResult && <span className="agent-activity-inline-result">{inlineResult}</span>}
          </span>

          <span className="agent-activity-status-col modern-timeline-status-col">
            {onReferenceFile && isFileAction && cleanTarget && (
              <button
                type="button"
                className="agent-activity-mention-btn"
                onClick={(e) => {
                  e.stopPropagation()
                  onReferenceFile(cleanTarget)
                }}
                title={`Reference in chat: ${fileName}`}
                aria-label={`Reference ${fileName} in chat`}
              >
                @
              </button>
            )}
            {renderStatusIndicator(activity.state)}
            {output && (
              <ChevronRight
                size={12}
                className={`modern-timeline-chevron${isItemExpanded ? ' is-open' : ''}`}
                aria-hidden="true"
              />
            )}
          </span>
        </div>
        {activity.viewedImage && (
          <div className="agent-activity-inline-preview">
            <ViewedImage
              image={activity.viewedImage}
              chatId={chatId}
              variant="inline"
              onOpenArtifact={onOpenArtifact}
            />
          </div>
        )}
        {isItemExpanded && output && (
          <div className="agent-activity-output modern-timeline-output">
            {output.includes('[Self-Correction Guidance]') ? (
              <>
                {output.split('[Self-Correction Guidance]')[0]?.trim() && (
                  <pre>{output.split('[Self-Correction Guidance]')[0]?.trim()}</pre>
                )}
                <div className="diagnostic-guidance-card">
                  <div className="diagnostic-guidance-header">
                    <span>💡 Self-Correction Guidance</span>
                  </div>
                  <pre>{output.split('[Self-Correction Guidance]')[1]?.trim()}</pre>
                </div>
              </>
            ) : (
              <pre>{output}</pre>
            )}
          </div>
        )}
      </Fragment>
    )
  }

  const renderGroup = (group: AgentActivityGroup) => {
    const isGroupExpanded = Boolean(expandedGroups[group.id])
    const verb =
      group.semanticVerb ||
      (group.action?.includes('shell') || group.action?.includes('command')
        ? 'Ran'
        : group.label || group.action || 'Executed')

    return (
      <div key={group.id} className="agent-activity-group modern-timeline-group">
        <div
          className={`agent-activity-group-header modern-timeline-row modern-group-row${isGroupExpanded ? ' is-expanded' : ''}`}
          data-state={group.state}
          onClick={() => toggleGroup(group.id)}
        >
          <span className="modern-timeline-verb">{verb}</span>
          <span className="agent-activity-group-title modern-timeline-target">
            {group.title}
          </span>
          <span className="modern-timeline-status-col">
            <span className="modern-timeline-count-badge">
              {group.count}
            </span>
            {renderStatusIndicator(group.state)}
            <ChevronRight
              size={12}
              className={`modern-timeline-chevron${isGroupExpanded ? ' is-open' : ''}`}
              aria-hidden="true"
            />
          </span>
        </div>

        {isGroupExpanded && (
          <div className="agent-activity-group-subitems modern-group-subitems">
            {group.items.map((activity, itemIdx) => {
              const itemKey = `${group.id}-${itemIdx}-${activity.target}`
              return renderSingleItem(activity, itemKey)
            })}
            {group.action === 'mcp_tool' && progress && (
              <div className="agent-activity-mcp-details" style={{ padding: '4px 12px 10px 24px' }}>
                <McpToolResults progress={progress} chatId={chatId} mode="artifacts-only" />
              </div>
            )}
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="agent-activity-list modern-activity-list">
      {activityView.timeline && activityView.timeline.length > 0
        ? activityView.timeline.map((item, idx) => {
            if (item.kind === 'thought') {
              return <InlineThoughtNote key={item.id || `thought-${idx}`} thought={item.thought} />
            }
            if (item.kind === 'extendedThinking') {
              const itemKey = item.id || `ext-thought-${idx}`
              const isThoughtExpanded = expandedIndices[itemKey] ?? Boolean(item.streaming)
              const thoughtSecs = item.elapsedMs ? `${Math.max(1, Math.round(item.elapsedMs / 1000))}s` : ''
              const thoughtTitle = item.streaming
                ? 'Thinking…'
                : thoughtSecs
                  ? `Thought for ${thoughtSecs}`
                  : 'Thought'

              return (
                <div key={itemKey} className="agent-activity-thought-step modern-timeline-thought-item">
                  <div
                    className={`agent-activity-thought-header modern-timeline-row modern-thought-row${isThoughtExpanded ? ' is-expanded' : ''}`}
                    onClick={() => toggleExpandItem(itemKey, Boolean(item.streaming))}
                  >
                    <span className="modern-timeline-thought-label">
                      {thoughtTitle}
                      <ChevronRight
                        size={12}
                        className={`modern-timeline-chevron inline-chevron${isThoughtExpanded ? ' is-open' : ''}`}
                        aria-hidden="true"
                      />
                    </span>
                  </div>
                  {isThoughtExpanded && (
                    <div className="agent-activity-thought-body modern-thought-body">
                      {item.streaming
                        ? <>{item.thought}<span className="agent-activity-thought-cursor">▍</span></>
                        : (renderFormattedMessage(item.thought) ?? item.thought)}
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
