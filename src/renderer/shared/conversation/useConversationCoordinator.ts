import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import type { ConversationState } from './coordinator'
import { createConversationLifecycle, type ConversationEffects, type ConversationTransport } from './lifecycle'
import { conversationPlatform } from '../platform'

interface CoordinatorOptions extends ConversationEffects {
  conversationId: string
  draftKeyPrefix: string
  workspaceStorageKey: string
  historyVisible: boolean
}

/** React adapter: persistence and browser signals enter the same lifecycle tests use. */
export function useConversationCoordinator(
  initial: Partial<ConversationState>,
  options: CoordinatorOptions,
  transport: ConversationTransport = conversationPlatform,
) {
  const latest = useRef(options)
  latest.current = options
  const [actions] = useState(() => createConversationLifecycle(options.conversationId, initial, transport, {
    onError: error => latest.current.onError?.(error),
    onApproval: (approval, chatId) => latest.current.onApproval?.(approval, chatId),
    onReply: (response, chatId) => latest.current.onReply?.(response, chatId),
    onActivityPersisted: id => latest.current.onActivityPersisted?.(id),
    onRunCompleted: () => latest.current.onRunCompleted?.(),
    onVisibleRunCompleted: chatId => latest.current.onVisibleRunCompleted?.(chatId),
  }, {
    read: id => window.localStorage.getItem(`${latest.current.draftKeyPrefix}${id}`) || '',
    write: (id, text) => {
      const key = `${latest.current.draftKeyPrefix}${id}`
      if (text) window.localStorage.setItem(key, text)
      else window.localStorage.removeItem(key)
    },
  }))
  const conversation = useSyncExternalStore(actions.subscribe, actions.getSnapshot, actions.getSnapshot)

  useEffect(() => actions.watch({ window, document }, options.historyVisible),
    [actions, options.historyVisible, conversation.conversationId])

  useEffect(() => {
    const timer = window.setTimeout(() => {
      try {
        const key = `${options.draftKeyPrefix}${conversation.conversationId}`
        if (conversation.message) window.localStorage.setItem(key, conversation.message)
        else window.localStorage.removeItem(key)
      } catch {
        // Draft persistence is best-effort in private browsing modes.
      }
    }, 300)
    return () => window.clearTimeout(timer)
  }, [conversation.message, conversation.conversationId, options.draftKeyPrefix])

  useEffect(() => {
    try {
      if (conversation.workspacePath) window.localStorage.setItem(options.workspaceStorageKey, conversation.workspacePath)
      else window.localStorage.removeItem(options.workspaceStorageKey)
    } catch {
      // Workspace persistence is best-effort in private browsing modes.
    }
  }, [conversation.workspacePath, options.workspaceStorageKey])

  return { conversation, actions }
}
