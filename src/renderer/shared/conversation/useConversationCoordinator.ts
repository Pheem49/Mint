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
  const dashboardSnapshotRef = useRef<ReturnType<typeof actions.getSnapshot> | null>(null)
  const getDashboardSnapshot = () => {
    const next = actions.getSnapshot()
    const prev = dashboardSnapshotRef.current
    if (prev && prev !== next) {
      let onlyMessageChanged = prev.message !== next.message
      if (onlyMessageChanged) {
        for (const key of Object.keys(next) as Array<keyof typeof next>) {
          if (key !== 'message' && prev[key] !== next[key]) {
            onlyMessageChanged = false
            break
          }
        }
      }
      if (onlyMessageChanged) {
        return prev
      }
    }
    dashboardSnapshotRef.current = next
    return next
  }

  const conversation = useSyncExternalStore(actions.subscribe, getDashboardSnapshot, getDashboardSnapshot)

  useEffect(() => actions.watch({ window, document }, options.historyVisible),
    [actions, options.historyVisible, conversation.conversationId])

  useEffect(() => {
    let timer: number | undefined
    const unsubscribe = actions.subscribe(() => {
      const snap = actions.getSnapshot()
      if (timer !== undefined) window.clearTimeout(timer)
      timer = window.setTimeout(() => {
        try {
          const key = `${latest.current.draftKeyPrefix}${snap.conversationId}`
          if (snap.message) window.localStorage.setItem(key, snap.message)
          else window.localStorage.removeItem(key)
        } catch {
          // Draft persistence is best-effort in private browsing modes.
        }
      }, 300)
    })
    return () => {
      unsubscribe()
      if (timer !== undefined) window.clearTimeout(timer)
    }
  }, [actions])

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
