import React, { memo, useMemo, useState } from 'react'
import { EMPTY_AGENT_PROGRESS, parseFileChangesFromProgress } from '../../agentProgress'
import { conversationPlatform } from '../../platform'
import type { AgentProgress, FileChange, GitCheckpoint } from '../../types'
import type { ArtifactFile } from '../../utils/artifacts'

const { listGitCheckpoints } = conversationPlatform

export interface RewindRequestPayload {
  interaction: any
  changes: FileChange[]
  checkpoints: GitCheckpoint[]
  targetCheckpoint: GitCheckpoint | null
}

export interface TurnFileChangesProps {
  progress?: AgentProgress[]
  changes?: FileChange[]
  interaction?: any
  isLive?: boolean
  defaultOpen?: boolean
  onOpenArtifact?: (artifact: ArtifactFile) => void
  onOpenReview?: () => void
  onRewindRequest?: (payload: RewindRequestPayload) => void
  onRewindError?: (message: string) => void
}

export const TurnFileChanges = memo(function TurnFileChanges({
  progress,
  changes: providedChanges,
  interaction,
  isLive = false,
  defaultOpen = true,
  onOpenArtifact,
  onOpenReview,
  onRewindRequest,
  onRewindError,
}: TurnFileChangesProps) {
  const changes = useMemo(
    () => providedChanges ?? parseFileChangesFromProgress(progress ?? EMPTY_AGENT_PROGRESS),
    [providedChanges, progress],
  )
  const [isOpen] = useState(defaultOpen)
  const [showAll, setShowAll] = useState(false)
  const [openFileDiffs, setOpenFileDiffs] = useState<Record<string, boolean>>({})

  if (changes.length === 0) return null

  const totalAdditions = changes.reduce((sum, c) => sum + c.additions, 0)
  const totalDeletions = changes.reduce((sum, c) => sum + c.deletions, 0)
  const createdCount = changes.filter((c) => c.created).length
  const modifiedCount = changes.length - createdCount

  let summaryLabel = ''
  if (createdCount > 0 && modifiedCount === 0) {
    summaryLabel = `${createdCount} ${createdCount === 1 ? 'file created' : 'files created'}`
  } else if (createdCount > 0 && modifiedCount > 0) {
    summaryLabel = `${createdCount} created, ${modifiedCount} modified`
  } else {
    summaryLabel = `${changes.length} ${changes.length === 1 ? 'file changed' : 'files changed'}`
  }

  const visibleChanges = showAll ? changes : changes.slice(0, 3)

  const handleRewind = async (e: React.MouseEvent) => {
    e.stopPropagation()
    if (!interaction || !onRewindRequest) return
    const chatId = interaction.chatId || 'cli'
    try {
      const checkpoints = await listGitCheckpoints(chatId)
      if (!checkpoints || checkpoints.length === 0) {
        onRewindError?.('No git checkpoints recorded for this session.')
        return
      }

      const changePaths = new Set(changes.map((c) => c.path.trim().toLowerCase()))
      const matching = checkpoints.filter((cp) => {
        if (!cp.targetPath) return false
        const cpPath = cp.targetPath.trim().toLowerCase()
        return changePaths.has(cpPath) || Array.from(changePaths).some((p) => cpPath.endsWith(p) || p.endsWith(cpPath))
      })

      let targetCp: GitCheckpoint | null = null
      if (matching.length > 0) {
        targetCp = matching.reduce((earliest, curr) => (curr.step < earliest.step ? curr : earliest), matching[0])
      } else {
        const interactionTs = interaction.createdAt ? new Date(interaction.createdAt).getTime() / 1000 : 0
        if (interactionTs > 0) {
          const afterCreated = checkpoints.filter((cp) => cp.timestamp >= interactionTs - 10)
          if (afterCreated.length > 0) {
            targetCp = afterCreated[0]
          }
        }
        if (!targetCp) {
          targetCp = checkpoints[checkpoints.length - 1]
        }
      }

      onRewindRequest({
        interaction,
        changes,
        checkpoints,
        targetCheckpoint: targetCp,
      })
    } catch (err: any) {
      onRewindError?.(`Failed to load checkpoints: ${err?.message || String(err)}`)
    }
  }

  const renderChangesList = () => (
    <div className="file-changes-list" style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
      {visibleChanges.map((change) => {
        const isDiffOpen = Boolean(openFileDiffs[change.path])
        const fileName = change.path.split('/').pop() || change.path
        const dirPath = change.path.includes('/') ? change.path.substring(0, change.path.lastIndexOf('/')) : ''

        return (
          <div key={change.path} className="file-changes-item">
            <div
              className="file-changes-item-header"
              onClick={() => setOpenFileDiffs((current) => ({ ...current, [change.path]: !current[change.path] }))}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  style={{ color: 'var(--text-muted)' }}
                >
                  <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
                  <polyline points="14 2 14 8 20 8" />
                </svg>
                <span className={`file-changes-filename ${change.created ? 'is-new' : ''}`}>
                  {fileName}
                  {dirPath && <span className="file-changes-dirpath">{dirPath}</span>}
                  {change.created && <span className="file-changes-badge-new">[NEW FILE]</span>}
                </span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '0.76rem' }}>
                {change.created ? (
                  <span className="file-changes-count-add">
                    +{change.additions} {change.additions === 1 ? 'line' : 'lines'}
                  </span>
                ) : (
                  <>
                    {change.additions > 0 && <span className="file-changes-count-add">+{change.additions}</span>}
                    {change.deletions > 0 && <span className="file-changes-count-del">-{change.deletions}</span>}
                  </>
                )}
                {onOpenArtifact && (
                  <button
                    type="button"
                    className="file-changes-preview-btn"
                    onClick={(e) => {
                      e.stopPropagation()
                      const fallbackContent =
                        change.created && change.hunks.length > 0
                          ? change.hunks.map((h) => h.newText).filter(Boolean).join('\n')
                          : undefined
                      onOpenArtifact({ path: change.path, content: fallbackContent })
                    }}
                    title="Open Live Preview Split View"
                  >
                    Preview
                  </button>
                )}
                <span
                  style={{
                    color: 'var(--text-muted)',
                    transform: isDiffOpen ? 'rotate(90deg)' : 'none',
                    display: 'inline-block',
                    transition: 'transform 0.15s',
                  }}
                >
                  &gt;
                </span>
              </div>
            </div>

            {isDiffOpen && (
              <div className="file-changes-diff-box">
                {change.hunks.map((hunk, hIdx) => (
                  <div
                    key={hIdx}
                    className="file-changes-diff-hunk"
                    style={{ marginBottom: hIdx < change.hunks.length - 1 ? '10px' : 0 }}
                  >
                    {hunk.oldText && (
                      <div className="file-changes-diff-del">
                        {hunk.oldText.split('\n').map((line, lIdx) => (
                          <div key={lIdx} style={{ display: 'flex' }}>
                            <span className="file-changes-diff-sign-del">-</span>
                            <span>{line}</span>
                          </div>
                        ))}
                      </div>
                    )}
                    {hunk.newText && (
                      <div className="file-changes-diff-add">
                        {hunk.newText.split('\n').map((line, lIdx) => (
                          <div key={lIdx} style={{ display: 'flex' }}>
                            <span className="file-changes-diff-sign-add">+</span>
                            <span>{line}</span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        )
      })}
      {changes.length > 3 && (
        <button
          type="button"
          className="file-changes-show-more"
          aria-expanded={showAll}
          onClick={() => setShowAll((prev) => !prev)}
        >
          {showAll ? 'Show fewer files' : `Show ${changes.length - 3} more ${changes.length - 3 === 1 ? 'file' : 'files'}`}
          <span aria-hidden="true" className={showAll ? 'is-expanded' : ''}>
            ⌄
          </span>
        </button>
      )}
    </div>
  )

  if (isLive) {
    return (
      <div className="message ai-message agent-activity-message" style={{ marginTop: '4px', marginBottom: '8px' }}>
        <div className="agent-activity-card file-changes-card">
          <div className="file-changes-summary-row">
            <div className="file-changes-summary">
              <span className="file-changes-summary-icon" aria-hidden="true">
                ✓
              </span>
              <span className="file-changes-summary-copy">
                <span className="file-changes-summary-label">{summaryLabel}</span>
                <span className="file-changes-summary-counts">
                  {totalAdditions > 0 && <span className="file-changes-count-add">+{totalAdditions}</span>}
                  {totalDeletions > 0 && <span className="file-changes-count-del">-{totalDeletions}</span>}
                </span>
              </span>
            </div>
            {onOpenReview && (
              <button
                type="button"
                className="file-changes-review-btn"
                title="Review changed code"
                onClick={onOpenReview}
              >
                Review
              </button>
            )}
          </div>

          {isOpen && (
            <div className="file-changes-list-container" style={{ marginTop: '8px' }}>
              {renderChangesList()}
            </div>
          )}
        </div>
      </div>
    )
  }

  return (
    <div className="file-changes-summary-container">
      <div className="agent-activity-card file-changes-card">
        <div className="file-changes-summary-row">
          <div className="file-changes-summary">
            <span className="file-changes-summary-icon" aria-hidden="true">
              ⊞
            </span>
            <span className="file-changes-summary-copy">
              <span className="file-changes-summary-label">{summaryLabel}</span>
              <span className="file-changes-summary-counts">
                {totalAdditions > 0 && <span className="file-changes-count-add">+{totalAdditions}</span>}
                {totalDeletions > 0 && <span className="file-changes-count-del">-{totalDeletions}</span>}
              </span>
            </span>
          </div>
          <div className="file-changes-summary-actions">
            {onRewindRequest && (
              <button
                type="button"
                className="file-changes-rewind-btn"
                onClick={handleRewind}
                title="Rewind workspace to before these file edits (Git Checkpoint)"
              >
                <svg
                  width="12"
                  height="12"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <polyline points="1 4 7 4 7 10" />
                  <path d="M3 9a9 9 0 1 0 2.13-5.36L1 7" />
                </svg>
                Undo
              </button>
            )}
            {onOpenReview && (
              <button
                type="button"
                className="file-changes-review-btn"
                title="Review changed code"
                onClick={onOpenReview}
              >
                Review
              </button>
            )}
          </div>
        </div>
        {isOpen && <div className="file-changes-list-container">{renderChangesList()}</div>}
      </div>
    </div>
  )
})

export default TurnFileChanges
