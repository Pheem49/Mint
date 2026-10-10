import { describe, expect, it } from 'vitest'
import ChatMessageItem, { type ChatMessageItemProps } from './ChatMessageItem'

const action = () => {}
const base: ChatMessageItemProps = {
  interaction: { id: 7, status: 'completed', aiText: 'Ready', userText: 'Hello', createdAt: '2026-10-10T00:00:00Z' },
  copiedId: null, speakingText: null, agentActivitySnapshots: {}, thinkingExpanded: {},
  onThinkingExpandedChange: action, handleCopyMessage: action, speak: action,
  toggleActivity: action, renderFileChanges: () => null,
}
const unchanged = (next: Partial<ChatMessageItemProps>) => (ChatMessageItem as any).compare(base, { ...base, ...next })

describe('message update isolation', () => {
  it('updates a file summary when its initially expanded review is collapsed', () => {
    expect(unchanged({ openReviewIds: { '7': false } })).toBe(false)
  })
  it('updates only the message whose file details or show-more state changed', () => {
    expect(unchanged({ openFileDiffs: { '8-/other.ts': true } })).toBe(true)
    expect(unchanged({ openFileDiffs: { '7-/own.ts': true } })).toBe(false)
    expect(unchanged({ showAllFileChanges: { '8': true } })).toBe(true)
    expect(unchanged({ showAllFileChanges: { '7': true } })).toBe(false)
  })
  it('does not retain stale user actions', () => {
    expect(unchanged({ onEditMessage: () => {} })).toBe(false)
  })
})
