import React, { memo, useMemo } from 'react'
import type { AgentProgress } from '../../types'
import { detectArtifactType, type ArtifactFile } from '../../utils/artifacts'
import { setWorkspaceDragData } from '../../utils/workspaceDrag'

export interface ArtifactCardsProps {
  progress?: AgentProgress[]
  aiText?: string
  workspacePath?: string
  onOpenArtifact?: (artifact: ArtifactFile) => void
}

interface DetectedArtifact {
  path: string
  name: string
  type: string
  source: 'ai_text' | 'tool_call'
}

const PRESENTATION_EXT_RE = /\.(pdf|pptx)$/i

export function extractArtifacts(
  progress?: AgentProgress[],
  aiText?: string,
  workspacePath?: string
): DetectedArtifact[] {
  const found = new Map<string, DetectedArtifact>()
  const normWorkspace = workspacePath ? workspacePath.replace(/\\/g, '/').replace(/\/+$/, '') : ''

  const addPath = (rawPath: string, source: 'ai_text' | 'tool_call') => {
    let clean = rawPath.trim()
    // Strip wrapping quotes, markdown code ticks, parentheses, trailing punctuation
    clean = clean.replace(/^[`"'(<[]+|[`"')>\].,;:]+$/g, '').trim()
    if (!clean || !PRESENTATION_EXT_RE.test(clean)) return
    // Ignore URL protocols other than file
    if (/^https?:\/\//i.test(clean)) return
    if (clean.startsWith('file://')) clean = clean.slice(7)
    // Avoid common false positives
    if (clean.includes('*') || clean.includes('?') || clean.length < 4) return

    clean = clean.replace(/\\/g, '/')
    const name = clean.split('/').pop() || clean
    const ext = name.split('.').pop()?.toLowerCase() || ''

    if (['pdf', 'pptx'].includes(ext)) {
      // Determine workspace-relative path
      let relativePath = clean
      if (normWorkspace) {
        if (clean.startsWith('/')) {
          if (clean === normWorkspace || clean.startsWith(normWorkspace + '/')) {
            relativePath = clean.slice(normWorkspace.length + 1)
          } else {
            // Absolute path outside current workspace: only keep if matching file name
            relativePath = name
          }
        }
      } else if (clean.startsWith('/')) {
        relativePath = name
      }

      const key = name.toLowerCase()
      const existing = found.get(key)
      if (!existing) {
        found.set(key, {
          path: relativePath,
          name,
          type: ext === 'pdf' ? 'pdf' : 'presentation',
          source,
        })
      } else {
        // If relativePath has subdirectories, prefer more specific relative path
        if (relativePath.includes('/') && !existing.path.includes('/')) {
          found.set(key, {
            path: relativePath,
            name,
            type: existing.type,
            source,
          })
        }
      }
    }
  }

  // Scan aiText for files that Mint explicitly created or referenced
  if (aiText) {
    // Match full paths or filenames mentioned in text or markdown code/links
    const matches = aiText.match(/(?:file:\/\/)?(?:\/[^\s"'`<>]+|[^\s"'`<>]+)\.(?:pdf|pptx)/gi)
    if (matches) {
      for (const m of matches) addPath(m, 'ai_text')
    }
  }

  return Array.from(found.values())
}

export const ArtifactCards = memo(function ArtifactCards({
  progress,
  aiText,
  workspacePath,
  onOpenArtifact,
}: ArtifactCardsProps) {
  const artifacts = useMemo(() => extractArtifacts(progress, aiText, workspacePath), [progress, aiText, workspacePath])

  if (artifacts.length === 0) return null

  return (
    <div
      className="chat-artifact-cards"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
        margin: '8px 0 12px',
        width: '100%',
        boxSizing: 'border-box',
      }}
    >
      {artifacts.map((art) => {
        const isPdf = art.name.toLowerCase().endsWith('.pdf')
        const badgeText = isPdf ? 'PDF' : 'PPTX'
        const badgeColor = isPdf ? '#ef4444' : '#f97316'
        const badgeBg = isPdf ? 'rgba(239, 68, 68, 0.15)' : 'rgba(249, 115, 22, 0.15)'

        return (
          <div
            key={art.path}
            className="chat-artifact-card"
            draggable
            onDragStart={(event) => {
              setWorkspaceDragData(event.dataTransfer, {
                path: art.path,
                root: workspacePath,
              })
            }}
            title={`${art.path} (Drag to chat)`}
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '10px 14px',
              borderRadius: 'var(--radius-sm, 8px)',
              background: 'color-mix(in srgb, var(--surface-bg, #1e1e20) 70%, rgba(255, 255, 255, 0.02))',
              border: '1px solid color-mix(in srgb, var(--border) 60%, transparent)',
              boxShadow: '0 2px 8px rgba(0, 0, 0, 0.15)',
              gap: '12px',
              cursor: 'grab',
              width: '100%',
              boxSizing: 'border-box',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '10px', minWidth: 0 }}>
              <div
                style={{
                  width: '36px',
                  height: '32px',
                  borderRadius: 'var(--radius-xs, 6px)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  background: badgeBg,
                  color: badgeColor,
                  flexShrink: 0,
                  fontSize: '11px',
                  fontWeight: 700,
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px',
                }}
              >
                {badgeText}
              </div>
              <div style={{ minWidth: 0, display: 'flex', flexDirection: 'column' }}>
                <span
                  style={{
                    fontWeight: 600,
                    fontSize: '0.85rem',
                    color: 'var(--text-main, #f4f4f5)',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}
                  title={art.path}
                >
                  {art.name}
                </span>
                <span
                  style={{
                    fontSize: '0.72rem',
                    color: 'var(--text-muted, #a1a1aa)',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}
                >
                  {isPdf ? 'PDF Presentation / Document' : 'PowerPoint Presentation'} • {art.path}
                </span>
              </div>
            </div>

            {onOpenArtifact && (
              <button
                type="button"
                className="chat-artifact-open-btn"
                onClick={() =>
                  onOpenArtifact({
                    path: art.path,
                    workspacePath,
                    type: detectArtifactType(art.path),
                  })
                }
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '5px 11px',
                  borderRadius: 'var(--radius-xs, 6px)',
                  background: 'color-mix(in srgb, var(--text-main, #ffffff) 6%, transparent)',
                  color: 'var(--text-main, #f4f4f5)',
                  fontWeight: 500,
                  fontSize: '0.76rem',
                  border: '1px solid var(--border, rgba(255, 255, 255, 0.12))',
                  cursor: 'pointer',
                  flexShrink: 0,
                  transition: 'background 0.15s ease, border-color 0.15s ease',
                }}
              >
                <span>Open in Side Panel</span>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M5 12h14" />
                  <path d="m12 5 7 7-7 7" />
                </svg>
              </button>
            )}
          </div>
        )
      })}
    </div>
  )
})
