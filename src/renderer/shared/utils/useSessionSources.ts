import { useCallback, useEffect, useMemo, useState } from 'react'
import { conversationPlatform } from '../platform'
import type { AgentProgress, ConversationSources } from '../types'
import { collectSessionSources } from './sessionSources'

/** Shared session ownership and polling for the Desktop and Web source panels. */
export function useSessionSources(chatId: string, liveProgress: AgentProgress[], enabled: boolean) {
  const [state, setState] = useState<{ chatId: string; value: ConversationSources | null; error: string; loading: boolean } | null>(null)
  const [attempt, setAttempt] = useState(0)
  const retry = useCallback(() => setAttempt(value => value + 1), [])
  useEffect(() => {
    if (!enabled) return
    let disposed = false
    let busy = false
    setState({ chatId, value: null, error: '', loading: true })
    const load = async () => {
      if (busy || document.hidden) return
      busy = true
      try {
        const value = await conversationPlatform.getConversationSources(chatId)
        if (!disposed) setState({ chatId, value, error: '', loading: false })
      } catch (error) {
        if (!disposed) setState(previous => ({ chatId, value: previous?.chatId === chatId ? previous.value : null, error: String(error), loading: false }))
      } finally { busy = false }
    }
    void load()
    const timer = window.setInterval(() => { void load() }, 2000)
    return () => { disposed = true; window.clearInterval(timer) }
  }, [enabled, chatId, attempt])
  const current = state?.chatId === chatId ? state : null
  const sources = useMemo(() => enabled ? collectSessionSources(current?.value ?? { attachments: [], webSearchProgress: [] }, liveProgress) : [], [enabled, current, liveProgress])
  return { sources, loading: enabled && (!current || current.loading), error: current?.error || '', retry }
}
