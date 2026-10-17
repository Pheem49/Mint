import { normalizeWorkspacePath, workspacePaths } from './workspaces'

const REMOVED_WORKSPACES_KEY = 'mint:removed-workspace-paths'
export const EMPTY_REMOVED_WORKSPACES: string[] = []

/** List visibility only: workspace ownership, sessions and files remain untouched. */
export function createWorkspaceListPreferences(storage: Pick<Storage, 'getItem' | 'setItem'>) {
  let snapshot: string[] | null = null
  let sessionOnly = false
  const listeners = new Set<() => void>()
  const read = () => {
    try {
      const value: unknown = JSON.parse(storage.getItem(REMOVED_WORKSPACES_KEY) || '[]')
      return Array.isArray(value) ? workspacePaths(value.filter((path): path is string => typeof path === 'string')) : []
    } catch { return snapshot ?? [] }
  }
  const getSnapshot = (): string[] => snapshot ??= read()
  const update = (paths: string[], persist: boolean) => {
    if (JSON.stringify(paths) === JSON.stringify(getSnapshot())) return
    snapshot = paths
    if (persist) {
      try { storage.setItem(REMOVED_WORKSPACES_KEY, JSON.stringify(paths)); sessionOnly = false } catch { sessionOnly = true }
    }
    listeners.forEach(listener => listener())
  }
  return {
    getSnapshot,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    remove(path: string) {
      const normalized = normalizeWorkspacePath(path)
      if (normalized) update(workspacePaths([...getSnapshot(), normalized]), true)
    },
    restore(path: string) {
      const normalized = normalizeWorkspacePath(path)
      update(getSnapshot().filter(path => path !== normalized), true)
    },
    reload() { if (!sessionOnly) update(read(), false) },
  }
}

export const workspaceListPreferences = createWorkspaceListPreferences({
  getItem: key => window.localStorage.getItem(key),
  setItem: (key, value) => window.localStorage.setItem(key, value),
})

export function subscribeWorkspaceListPreferences(listener: () => void) {
  const unsubscribe = workspaceListPreferences.subscribe(listener)
  workspaceListPreferences.reload()
  const onStorage = (event: StorageEvent) => {
    if (!event.key || event.key === REMOVED_WORKSPACES_KEY) workspaceListPreferences.reload()
  }
  window.addEventListener('storage', onStorage)
  return () => { unsubscribe(); window.removeEventListener('storage', onStorage) }
}
