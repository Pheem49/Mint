import { useEffect, useMemo, useReducer, useRef } from 'react'
import type { ConversationState } from './coordinator'
import { initialConversationState, transitionConversation } from './coordinator'
import type { AgentProgress, ChatResponse } from '../types'

interface CoordinatorOptions {
  conversationId: string
  draftKeyPrefix: string
  workspaceStorageKey: string
}

export function useConversationCoordinator(
  initial: Partial<ConversationState>,
  options: CoordinatorOptions,
) {
  const [conversation, dispatch] = useReducer(transitionConversation, initial, initialConversationState)
  const actions = useMemo(() => ({
    compose: (message: string) => dispatch({ type: 'compose', message }),
    selectWorkspace: (path: string) => dispatch({ type: 'workspace_selected', path }),
    attachImage: (image: ConversationState['imageAttachments'][number]) => dispatch({ type: 'image_attached', image }),
    removeImage: (index: number) => dispatch({ type: 'image_removed', index }),
    attachVideo: (video: ConversationState['videoAttachments'][number]) => dispatch({ type: 'video_attached', video }),
    removeVideo: (index: number) => dispatch({ type: 'video_removed', index }),
    attachDocument: (document: ConversationState['documentAttachment']) => dispatch({ type: 'document_attached', document }),
    startRun: (message: string, imageCount = 0, videoCount = 0) => dispatch({
      type: 'run_started', message, imageCount, videoCount,
    }),
    receiveChunk: (chunk: string) => dispatch({ type: 'chunk_received', chunk }),
    receiveProgress: (progress: AgentProgress[]) => dispatch({ type: 'progress_received', progress }),
    clearProgress: () => dispatch({ type: 'progress_cleared' }),
    clearStream: () => dispatch({ type: 'stream_cleared' }),
    requestApproval: (approval: unknown) => dispatch({ type: 'approval_requested', approval }),
    finishRun: () => dispatch({ type: 'run_finished' }),
    cancelRun: () => dispatch({ type: 'run_cancelled' }),
    clearComposer: () => dispatch({ type: 'composer_cleared' }),
    switchSession: (draft = '') => dispatch({ type: 'session_switched', draft }),
    executeStream: async (
      execute: (onChunk: (chunk: string) => void) => Promise<ChatResponse>,
    ) => {
      // Keep provider streaming internal and reveal only the authoritative
      // completed response. This avoids partial Markdown/card layouts and
      // keeps Desktop and Web behavior aligned with the CLI.
      const response = await execute(() => {})
      dispatch({ type: 'chunk_received', chunk: response.text })
      dispatch({ type: 'response_received', response })
      return response
    },
    executeApproval: async (execute: () => Promise<void>) => {
      try {
        await execute()
      } finally {
        dispatch({ type: 'approval_resolved' })
      }
    },
    executeCancellation: async (execute: () => Promise<void>) => {
      try {
        await execute()
      } finally {
        dispatch({ type: 'run_cancelled' })
      }
    },
  }), [])

  const loadedConversationRef = useRef(options.conversationId)
  useEffect(() => {
    if (loadedConversationRef.current === options.conversationId) return
    loadedConversationRef.current = options.conversationId
    let draft = ''
    try {
      draft = window.localStorage.getItem(`${options.draftKeyPrefix}${options.conversationId}`) || ''
    } catch {
      // Draft persistence is best-effort in private browsing modes.
    }
    actions.switchSession(draft)
  }, [actions, options.conversationId, options.draftKeyPrefix])

  useEffect(() => {
    const timer = window.setTimeout(() => {
      try {
        const key = `${options.draftKeyPrefix}${options.conversationId}`
        if (conversation.message) window.localStorage.setItem(key, conversation.message)
        else window.localStorage.removeItem(key)
      } catch {
        // Draft persistence is best-effort in private browsing modes.
      }
    }, 300)
    return () => window.clearTimeout(timer)
  }, [conversation.message, options.conversationId, options.draftKeyPrefix])

  useEffect(() => {
    try {
      if (conversation.workspacePath) {
        window.localStorage.setItem(options.workspaceStorageKey, conversation.workspacePath)
      } else {
        window.localStorage.removeItem(options.workspaceStorageKey)
      }
    } catch {
      // Workspace persistence is best-effort in private browsing modes.
    }
  }, [conversation.workspacePath, options.workspaceStorageKey])

  return { conversation, actions }
}
