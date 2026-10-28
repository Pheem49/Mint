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

  it('manages queued prompts with add, edit, dequeue, remove, and pause operations', () => {
    let state = initialConversationState()
    expect(state.queuedPrompts).toEqual([])
    expect(state.queuePaused).toBe(false)

    const prompt1 = {
      id: 'q-1',
      chatId: 'chat-1',
      text: 'First queued prompt',
      createdAt: 1000,
    }
    const prompt2 = {
      id: 'q-2',
      chatId: 'chat-1',
      text: 'Second queued prompt',
      imageAttachments: [{ dataUri: 'data:...', name: 'pic.png' }],
      createdAt: 2000,
    }

    // Queue first prompt: should clear composer message and add to queuedPrompts
    state = transitionConversation(state, { type: 'compose', message: 'First queued prompt' })
    state = transitionConversation(state, { type: 'prompt_queued', prompt: prompt1 })
    expect(state.message).toBe('')
    expect(state.queuedPrompts).toHaveLength(1)
    expect(state.queuedPrompts[0]).toEqual(prompt1)

    // Queue second prompt with image
    state = transitionConversation(state, { type: 'prompt_queued', prompt: prompt2 })
    expect(state.queuedPrompts).toHaveLength(2)

    // Pause queue (e.g. when user clicks stop)
    state = transitionConversation(state, { type: 'queue_paused_set', paused: true })
    expect(state.queuePaused).toBe(true)

    // Edit second prompt: removes from queue and restores into composer
    state = transitionConversation(state, { type: 'queued_prompt_restored_to_composer', prompt: prompt2 })
    expect(state.queuedPrompts).toHaveLength(1)
    expect(state.queuedPrompts[0].id).toBe('q-1')
    expect(state.message).toBe('Second queued prompt')
    expect(state.imageAttachments).toHaveLength(1)

    // Dequeue first prompt
    state = transitionConversation(state, { type: 'queued_prompt_dequeued', id: 'q-1' })
    expect(state.queuedPrompts).toHaveLength(0)
  })
})
