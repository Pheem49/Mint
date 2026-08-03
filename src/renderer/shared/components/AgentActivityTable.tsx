import { Fragment, useState } from 'react'
import type { AgentActivityView } from '../utils/agentActivity'
import { materialFileIcon, materialFolderIcon, getExtension } from '../utils/fileIcons'

interface Props {
  activityView: AgentActivityView
}

function getFilename(target: string): string {
  const clean = target.split(' #')[0].trim()
  const segments = clean.split('/')
  return segments[segments.length - 1]
}

function resolveActivityIcon(kind: string, target: string): string | null {
  if (kind === 'file') {
    const filename = getFilename(target)
    const ext = getExtension(filename)
    return materialFileIcon(filename, ext)
  }
  if (kind === 'folder') {
    const foldername = getFilename(target)
    return materialFolderIcon(foldername, false)
  }
  return null
}

export function AgentActivityTable({ activityView }: Props) {
  const [expandedIndices, setExpandedIndices] = useState<Record<number, boolean>>({})

  const toggleExpand = (index: number) => {
    setExpandedIndices(prev => ({
      ...prev,
      [index]: !prev[index],
    }))
  }

  return (
    <div className="agent-activity-list">
      <div className="agent-activity-table-head" aria-hidden="true">
        <span>Tool</span>
        <span />
        <span>Target</span>
        <span />
      </div>
      {activityView.items.map((activity, index) => {
        const isExpanded = !!expandedIndices[index]
        const icon = resolveActivityIcon(activity.kind, activity.target)
        const output = activity.result?.trim()
        const key = `${index}-${activity.label}-${activity.target}`
        return (
          <Fragment key={key}>
            <div
              className="agent-activity-item"
              data-kind={activity.kind}
              data-state={activity.state}
              style={{ cursor: 'pointer' }}
              onClick={() => toggleExpand(index)}
            >
              <span className="agent-activity-label">{activity.label}</span>
              <span className="agent-activity-icon" aria-hidden="true" data-has-img={!!icon}>
                {icon && <img src={icon} alt="" draggable={false} />}
              </span>
              <span
                className="agent-activity-text"
                style={isExpanded ? { whiteSpace: 'normal', wordBreak: 'break-all', overflow: 'visible', textOverflow: 'clip' } : undefined}
              >
                {activity.target}
              </span>
              <span
                className="agent-activity-chevron"
                style={{
                  transform: isExpanded ? 'rotate(90deg)' : 'none',
                  transition: 'transform 0.2s ease',
                  display: 'inline-block',
                }}
                aria-hidden="true"
              >
                &gt;
              </span>
            </div>
            {isExpanded && output && (
              <div className="agent-activity-output">
                <pre>{output}</pre>
              </div>
            )}
          </Fragment>
        )
      })}
    </div>
  )
}

