import React from 'react'
import { detectArtifactType } from './ArtifactPreviewPanel'
import type { WorkspaceTreeEntry } from '../types'

interface Props {
  entry: WorkspaceTreeEntry
  pending: boolean
  onPreview?: () => void
  onOpenHtml?: () => void
}

export default function WorkspacePreviewActions({ entry, pending, onPreview, onOpenHtml }: Props) {
  const type = detectArtifactType(entry.name)
  if (entry.kind !== 'file' || type === 'code') return null
  return <>
    {type === 'html' && onOpenHtml && <button type="button" role="menuitem" disabled={pending} onClick={onOpenHtml}>Open in Browser</button>}
    {onPreview && <button type="button" role="menuitem" disabled={pending} onClick={onPreview}>Preview in Mint</button>}
  </>
}
