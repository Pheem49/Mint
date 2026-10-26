/// <reference path="../../env.d.ts" />
import { describe, expect, it, vi } from 'vitest'
// @ts-ignore react-dom/server lacks types without @types/react-dom
import { renderToStaticMarkup } from 'react-dom/server'
import { FormattedMessage } from '../../utils/markdown'
import ChatMessageItem, { type ChatMessageItemProps } from './ChatMessageItem'
import { TurnFileChanges } from './TurnFileChanges'
import type { AgentProgress } from '../../types'
import { compactAgentProgressForPersistence, reduceLiveAgentProgress } from '../../agentProgress'

const action = () => {}
const base: ChatMessageItemProps = {
  interaction: { id: 7, status: 'completed', aiText: 'Ready', userText: 'Hello', createdAt: '2026-10-10T00:00:00Z' },
  copiedId: null,
  speakingText: null,
  agentActivitySnapshots: {},
  handleCopyMessage: action,
  speak: action,
  toggleActivity: action,
}
const unchanged = (next: Partial<ChatMessageItemProps>) => (ChatMessageItem as any).compare(base, { ...base, ...next })

it('does not render duplicate completion status pills in message bubble', () => {
  const progress: any[] = [{ type: 'RunCompleted', data: { summary: { outcome: 'UNVERIFIED' } } }]
  const html = renderToStaticMarkup(<ChatMessageItem {...base} interaction={{ ...base.interaction, agentActivity: progress }} />)
  expect(html).not.toContain('data-completion-status')
})

describe('message update isolation and TurnFileChanges deep module', () => {
  it('updates only the message whose activity drawer state or snapshot changed', () => {
    expect(unchanged({ openActivityIds: { '8': true } })).toBe(true)
    expect(unchanged({ openActivityIds: { '7': true } })).toBe(false)
  })
  it('renders TurnFileChanges summary and show-more button when >3 files changed', () => {
    const changes = [1, 2, 3, 4].map((i) => ({
      path: `src/file-${i}.ts`,
      created: i === 1,
      additions: 5,
      deletions: 1,
      hunks: [{ oldText: 'old', newText: 'new' }],
    }))
    const html = renderToStaticMarkup(<TurnFileChanges changes={changes} />)
    expect(html).toContain('1 created, 3 modified')
    expect(html).toContain('Show 1 more file')
  })
  it('does not retain stale user actions', () => {
    expect(unchanged({ onEditMessage: () => {} })).toBe(false)
  })
})

describe('pasted message line breaks on Desktop and Web', () => {
  it('renders CR-only bullet lines from TUI history as separate list items', () => {
    const html = renderToStaticMarkup(<FormattedMessage text={'Token weather\r\r• ☀ Clear\r• 134.4k / 200k\r• ▲ +98.3k last turn'} />)
    expect(html.match(/class="chat-list-item chat-bullet-item"/g) ?? []).toHaveLength(3)
    expect(html).toContain('☀ Clear')
    expect(html).toContain('▲ +98.3k last turn')
  })
})

describe('view_image results on Desktop and Web', () => {
  const image = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII='
  const progress: AgentProgress[] = [
    { type: 'ToolStart', data: { callId: 'view-1', action: 'view_image', input: { path: '/workspace/screens/mobile.png' } } },
    { type: 'ToolEnd', data: { callId: 'view-1', action: 'view_image', input: { path: '/workspace/screens/mobile.png' }, result: `${image}\n\n[Hook] Image inspected` } },
    { type: 'ToolEnd', data: { action: 'view_image', input: { path: '/private/denied.png' }, status: 'failed', result: image } },
    { type: 'ToolEnd', data: { action: 'read_file', input: { path: 'note.txt' }, result: image } },
  ]
  it.each(['completed', 'failed', 'interrupted'])('keeps viewed images visible in %s history with Activity collapsed', status => {
    const saved = JSON.parse(JSON.stringify(compactAgentProgressForPersistence(progress)))
    const html = renderToStaticMarkup(<ChatMessageItem {...base} interaction={{ ...base.interaction, status, agentActivity: saved }} />)
    expect(html).toContain(`src="${image}"`)
    expect(html).toContain('alt="Viewed image: mobile.png"')
    expect(html).toContain('aria-label="Expand mobile.png"')
    expect(html.match(/data-viewed-image=/g)).toHaveLength(1)
    expect(html).not.toContain('Expand denied.png')
  })
  it('keeps a viewed image after many later tools complete in the same run', () => {
    const later: AgentProgress[] = Array.from({ length: 40 }, (_, index) => ({ type: 'ToolEnd', data: { action: 'read_file', input: { path: `file-${index}.md` }, result: 'Read' } }))
    const live = [...progress, ...later].reduce(reduceLiveAgentProgress, [] as AgentProgress[])
    const html = renderToStaticMarkup(<ChatMessageItem {...base} agentActivitySnapshots={{ '7': live }} />)
    expect(html).toContain('aria-label="Expand mobile.png"')
  })
  it.each(['https://example.com/private.png', 'data:text/html;base64,PHNjcmlwdD4=', 'data:image/png;base64,%%%invalid', 'Error: cannot read image', 'Image exists but is too large to inline'])('does not embed an unavailable or non-image result: %s', result => {
    const html = renderToStaticMarkup(<ChatMessageItem {...base} interaction={{ ...base.interaction, agentActivity: [{ type: 'ToolEnd', data: { action: 'view_image', input: { path: 'unavailable.png' }, result } }] }} />)
    expect(html).not.toContain('data-viewed-image=')
  })
  it('forwards onOpenArtifact to viewed image preview trigger', () => {
    const onOpenArtifact = vi.fn()
    const saved = JSON.parse(JSON.stringify(compactAgentProgressForPersistence(progress)))
    const html = renderToStaticMarkup(
      <ChatMessageItem
        {...base}
        onOpenArtifact={onOpenArtifact}
        interaction={{ ...base.interaction, status: 'completed', agentActivity: saved }}
      />
    )
    expect(html).toContain('title="Open full image in Tools Surface"')
    expect(html).toContain('aria-label="Expand mobile.png"')
  })

  it.each(['completed', 'failed', 'interrupted'] as const)(
    'renders activity drawer for %s turns when progress exists',
    (status) => {
      const html = renderToStaticMarkup(
        <ChatMessageItem
          {...base}
          interaction={{ ...base.interaction, status, agentActivity: progress }}
        />
      )
      expect(html).toContain('agent-activity-history')
      expect(html).toContain('agent-activity-toggle')
    }
  )

  it('does not render historical activity drawer when queued or running', () => {
    for (const status of ['queued', 'running'] as const) {
      const html = renderToStaticMarkup(
        <ChatMessageItem
          {...base}
          interaction={{ ...base.interaction, status, agentActivity: progress }}
        />
      )
      expect(html).not.toContain('agent-activity-history')
    }
  })
})
