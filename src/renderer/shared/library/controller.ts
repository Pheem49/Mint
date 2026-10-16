import type { LibraryItem, LibraryPage, LibraryQuery } from '../types'
export interface LibraryOperations {
  list(query: LibraryQuery): Promise<LibraryPage>
  setTrashed(id: string, trashed: boolean): Promise<void>
  remove(id: string): Promise<void>
}
export interface LibraryState { items: LibraryItem[]; query: LibraryQuery; nextCursor: string | null; loading: boolean; error: string; actionId: string | null; actionError: string }
export function createLibraryController(api: LibraryOperations) {
  let state: LibraryState = { items: [], query: {}, nextCursor: null, loading: false, error: '', actionId: null, actionError: '' }
  let generation = 0
  let loadedPages = 1
  const listeners = new Set<() => void>()
  const update = (patch: Partial<LibraryState>) => { state = { ...state, ...patch }; listeners.forEach(listener => listener()) }
  const load = async (append = false) => {
    if (append && (state.loading || !state.nextCursor)) return
    const current = ++generation
    const query = { ...state.query, cursor: append ? state.nextCursor : undefined }
    const pagesToRefresh = append ? 1 : loadedPages
    update({ loading: true, error: '' })
    try {
      let page = await api.list(query)
      if (current !== generation) return
      let pagesRead = 1
      const refreshedItems = [...page.items]
      while (!append && pagesRead < pagesToRefresh && page.nextCursor) {
        page = await api.list({ ...query, cursor: page.nextCursor })
        if (current !== generation) return
        refreshedItems.push(...page.items)
        pagesRead++
      }
      if (!append) page = { ...page, items: refreshedItems }
      loadedPages = append ? loadedPages + 1 : pagesRead
      const items = append ? [...new Map([...state.items, ...page.items].map(item => [item.id, item])).values()] : page.items
      update({ items, nextCursor: page.nextCursor })
    } catch (error) { if (current === generation) update({ error: String(error) }) }
    finally { if (current === generation) update({ loading: false }) }
  }
  const mutate = async (id: string, operation: () => Promise<void>) => {
    if (state.actionId) return
    const current = generation
    update({ actionId: id, actionError: '' })
    try {
      await operation()
      if (current === generation) { generation++; update({ items: state.items.filter(item => item.id !== id), loading: false }) }
      else await load()
    } catch (error) { update({ actionError: String(error) }) }
    finally { update({ actionId: null }) }
  }
  return {
    remove: (id: string) => mutate(id, () => api.remove(id)),
    getSnapshot: () => state,
    subscribe: (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener) } },
    setQuery: async (query: LibraryQuery) => { loadedPages = 1; update({ query: { ...query, cursor: undefined }, items: [], nextCursor: null, actionError: '' }); await load() },
    refresh: () => load(), loadMore: () => load(true),
    setTrashed: (id: string, trashed: boolean) => mutate(id, () => api.setTrashed(id, trashed)),
  }
}
