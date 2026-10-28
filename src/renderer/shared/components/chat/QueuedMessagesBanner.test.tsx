import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import QueuedMessagesBanner from './QueuedMessagesBanner'
import type { QueuedPrompt } from '../../types'

describe('QueuedMessagesBanner', () => {
  it('renders null when queuedPrompts is empty', () => {
    const html = renderToStaticMarkup(
      <QueuedMessagesBanner
        queuedPrompts={[]}
        queuePaused={false}
        sending={false}
        onRemove={() => {}}
        onEdit={() => {}}
        onClear={() => {}}
        onSteer={() => {}}
      />
    )
    expect(html).toBe('')
  })

  it('renders queued prompts list with text and attachments', () => {
    const mockPrompts: QueuedPrompt[] = [
      {
        id: 'q-1',
        chatId: 'chat-1',
        text: 'First instruction in queue',
        createdAt: 1000,
      },
      {
        id: 'q-2',
        chatId: 'chat-1',
        text: 'Second instruction with image',
        imageAttachments: [{ dataUri: 'data:...', name: 'diagram.png' }],
        documentAttachment: { filename: 'notes.pdf', dataUri: 'data:...' },
        createdAt: 2000,
      },
    ]

    const html = renderToStaticMarkup(
      <QueuedMessagesBanner
        queuedPrompts={mockPrompts}
        queuePaused={false}
        sending={true}
        onRemove={() => {}}
        onEdit={() => {}}
        onClear={() => {}}
        onSteer={() => {}}
      />
    )

    expect(html).toContain('Queued')
    expect(html).toContain('2') // Badge count
    expect(html).toContain('First instruction in queue')
    expect(html).toContain('Second instruction with image')
    expect(html).toContain('1 image')
    expect(html).toContain('notes.pdf')
    expect(html).toContain('Sends automatically after current turn completes')
    expect(html).toContain('Send now')
  })

  it('renders paused status and resume button when queuePaused is true', () => {
    const mockPrompts: QueuedPrompt[] = [
      {
        id: 'q-1',
        chatId: 'chat-1',
        text: 'Paused message',
        createdAt: 1000,
      },
    ]

    const html = renderToStaticMarkup(
      <QueuedMessagesBanner
        queuedPrompts={mockPrompts}
        queuePaused={true}
        sending={false}
        onRemove={() => {}}
        onEdit={() => {}}
        onClear={() => {}}
        onSteer={() => {}}
        onResumeQueue={() => {}}
      />
    )

    expect(html).toContain('Queue paused')
    expect(html).toContain('Resume')
  })
})
