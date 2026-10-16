import type { AgentProgress, ConversationSources } from '../types'
import { parseWebSearchSources } from './agentActivity'

export interface SessionSource {
  id: string
  kind: 'image' | 'file' | 'video' | 'web'
  name: string
  detail: string
  interactionId?: number
  thumbnailDataUri?: string
  url?: string
}

export function collectSessionSources(persisted: ConversationSources, liveProgress: AgentProgress[] = []): SessionSource[] {
  const attachments: SessionSource[] = persisted.attachments.map(attachment => ({
    id: `attachment:${attachment.id}`, kind: attachment.kind, name: attachment.name,
    detail: attachment.trashed ? 'In Trash · Restore in Library' : attachment.availability === 'missing' ? 'Original file missing' : attachment.kind === 'image' ? 'Image' : attachment.kind === 'video' ? 'Video' : (attachment.name.split('.').length > 1 ? attachment.name.split('.').at(-1)!.toUpperCase() : 'File'),
    url: attachment.libraryItemId ? `/library?${new URLSearchParams({ item: attachment.libraryItemId, ...(attachment.trashed ? { trash: '1' } : {}) })}` : undefined,
    interactionId: attachment.interactionId ?? undefined, thumbnailDataUri: attachment.thumbnailDataUri || undefined,
  }))
  const seen = new Set<string>()
  const web: SessionSource[] = []
  const progress = [...persisted.webSearchProgress, ...liveProgress].filter(event => event.type !== 'ToolEnd' || !event.data.status || event.data.status === 'success')
  for (const source of parseWebSearchSources(progress)) {
    const url = new URL(source.url)
    if (!['http:', 'https:'].includes(url.protocol)) continue
    url.hash = ''
    if (seen.has(url.href)) continue
    seen.add(url.href)
    web.push({ id: `web:${url.href}`, kind: 'web', name: source.title || source.domain, detail: source.domain, url: url.href })
  }
  return [...attachments, ...web]
}
