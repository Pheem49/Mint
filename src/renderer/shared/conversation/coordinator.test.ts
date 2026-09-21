import { describe, expect, it } from 'vitest'
import { initialConversationState, transitionConversation } from './coordinator'

describe('conversation coordinator interface', () => {
  it('coordinates compose, run streaming, approval, cancellation, and session switches', () => {
    let state = initialConversationState()
    state = transitionConversation(state, { type: 'compose', message: 'hello' })
    state = transitionConversation(state, { type: 'run_started', message: state.message, imageCount: 1, videoCount: 0 })
    state = transitionConversation(state, { type: 'chunk_received', chunk: 'hel' })
    state = transitionConversation(state, { type: 'chunk_received', chunk: 'lo' })
    state = transitionConversation(state, { type: 'approval_requested', approval: { token: 'approve-1' } })
    expect(state).toMatchObject({ sending: true, streamedReply: 'hello', pendingApproval: { token: 'approve-1' } })

    state = transitionConversation(state, { type: 'run_cancelled' })
    expect(state).toMatchObject({ sending: false, streamedReply: '', pendingApproval: null })

    state = transitionConversation(state, { type: 'session_switched', draft: 'next draft' })
    expect(state).toMatchObject({ message: 'next draft', imageAttachments: [], agentProgress: [] })
  })

  it('deduplicates attachments through semantic events', () => {
    const image = { dataUri: 'data:image/png;base64,a', name: 'a.png' }
    let state = transitionConversation(initialConversationState(), { type: 'image_attached', image })
    state = transitionConversation(state, { type: 'image_attached', image })
    expect(state.imageAttachments).toEqual([image])
  })
})
