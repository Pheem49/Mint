import { useCallback, useEffect, useRef, useState } from 'react'
import { workspacePlatform } from './platform'
import type { WorkspaceHistoryEntry, WorkspaceSnapshot } from './types'

const busyRoots = new Set<string>()
const HISTORY_EVENT = 'mint:workspace-history-changed'

export function announceWorkspaceChange(snapshot: WorkspaceSnapshot) {
  window.dispatchEvent(new CustomEvent(HISTORY_EVENT, { detail: snapshot }))
}

/** Shared by the workspace panel and titlebar; actions stay scoped to a root. */
export function useWorkspaceUndo(root: string, revision = 0, onSnapshot?: (snapshot: WorkspaceSnapshot) => void) {
  const [entries, setEntries] = useState<WorkspaceHistoryEntry[]>([])
  const [loading, setLoading] = useState(false)
  const loadedRoot = useRef(root)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const rootRef = useRef(root)
  rootRef.current = root
  const snapshotHandler = useRef(onSnapshot)
  snapshotHandler.current = onSnapshot

  useEffect(() => {
    let active = true
    let request = 0
    if (loadedRoot.current !== root) {
      loadedRoot.current = root
      setEntries([])
      setError('')
      setBusy(false)
    }
    const refresh = async () => {
      const current = ++request
      if (!root.trim()) { setLoading(false); return }
      setLoading(true)
      try {
        const result = await workspacePlatform.listWorkspaceHistory(root)
        if (active && current === request) setEntries(result)
      } catch (reason) {
        if (active && current === request) setError(reason instanceof Error ? reason.message : String(reason))
      } finally {
        if (active && current === request) setLoading(false)
      }
    }
    const changed = (event: Event) => {
      const snapshot = (event as CustomEvent<WorkspaceSnapshot>).detail
      if (snapshot?.path === root) {
        snapshotHandler.current?.(snapshot)
        void refresh()
      }
    }
    void refresh()
    window.addEventListener('focus', refresh)
    window.addEventListener(HISTORY_EVENT, changed)
    return () => {
      active = false
      window.removeEventListener('focus', refresh)
      window.removeEventListener(HISTORY_EVENT, changed)
    }
  }, [root, revision])

  const undo = useCallback(async (expectedId?: string) => {
    if (!entries.length || busyRoots.has(root)) return false
    if (expectedId && entries[0].id !== expectedId) {
      setError("Workspace history changed; review the latest action before undoing")
      return false
    }
    busyRoots.add(root)
    setBusy(true)
    setError('')
    try {
      const snapshot = await workspacePlatform.undoWorkspaceAction(root, revision, expectedId || entries[0].id)
      announceWorkspaceChange(snapshot)
      const result = await workspacePlatform.listWorkspaceHistory(root)
      if (rootRef.current === root) setEntries(result)
      return true
    } catch (reason) {
      if (rootRef.current === root) setError(reason instanceof Error ? reason.message : String(reason))
      return false
    } finally {
      busyRoots.delete(root)
      if (rootRef.current === root) setBusy(false)
    }
  }, [entries, root, revision])

  return { entries, busy, loading, error, undo }
}
