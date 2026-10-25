import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react'
import { Library, RefreshCw, Search, Trash2, X } from 'lucide-react'
import { libraryPlatform } from '../../platform'
import { createLibraryController } from '../../library/controller'
import { downloadLibraryFile } from '../../library/useLibraryFile'
import type { LibraryItem } from '../../types'
import LibraryItemCard from './LibraryItemCard'
import LibraryPreview from './LibraryPreview'
import '../../css/library.css'

const filters = [['', 'All'], ['image', 'Images'], ['video', 'Videos'], ['file', 'Files']] as const

export default function LibraryPage({ chatId, onClose, onOpenConversation }: {
  chatId?: string
  onClose?: () => void
  onOpenConversation?: (id: string) => void
}) {
  const controller = useMemo(() => createLibraryController({ list: libraryPlatform.listLibrary, setTrashed: libraryPlatform.setLibraryTrashed, remove: libraryPlatform.removeLibraryItem }), [])
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot)
  const [kind, setKind] = useState<'' | 'image' | 'video' | 'file'>('')
  const [search, setSearch] = useState('')
  const [debounced, setDebounced] = useState('')
  const [scope, setScope] = useState('all')
  const [trashed, setTrashed] = useState(() => new URLSearchParams(location.search).get('trash') === '1')
  const [linkedItem, setLinkedItem] = useState<LibraryItem | null>(null)
  const [preview, setPreview] = useState<LibraryItem | null>(null)
  const [downloadError, setDownloadError] = useState('')
  const [downloading, setDownloading] = useState('')
  const [navigation, setNavigation] = useState(0)
  const closePreview = useCallback(() => setPreview(null), [])

  useEffect(() => {
    const onNavigate = () => setNavigation(value => value + 1)
    window.addEventListener('popstate', onNavigate)
    return () => window.removeEventListener('popstate', onNavigate)
  }, [])
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(search), 200)
    return () => clearTimeout(timer)
  }, [search])
  useEffect(() => {
    void controller.setQuery({ kind: kind || undefined, search: debounced, chatId: scope === 'session' ? chatId : undefined, trashed })
  }, [kind, debounced, scope, chatId, trashed, controller])
  useEffect(() => {
    const onFocus = () => { void controller.refresh() }
    window.addEventListener('focus', onFocus)
    return () => window.removeEventListener('focus', onFocus)
  }, [controller])
  useEffect(() => {
    if (!state.items.some(item => item.thumbnailStatus === 'pending' && !item.deletedAt)) return
    const timer = setTimeout(() => { void controller.refresh() }, 2500)
    return () => clearTimeout(timer)
  }, [state.items, controller])
  useEffect(() => {
    const query = new URLSearchParams(location.search)
    const id = query.get('item')
    let active = true
    setLinkedItem(null)
    setPreview(null)
    setDownloadError('')
    setTrashed(query.get('trash') === '1')
    if (id) {
      setKind('')
      setSearch('')
      setScope('all')
      libraryPlatform.getLibraryItem(id).then(item => {
        if (!active) return
        setLinkedItem(item)
        if (item.deletedAt) {
          setTrashed(true)
        } else if (item.availability === 'ready') {
          setPreview(item)
        }
      }).catch(error => { if (active) setDownloadError(String(error)) })
    }
    return () => { active = false }
  }, [navigation])

  async function download(item: LibraryItem) {
    setDownloading(item.id)
    setDownloadError('')
    try { await downloadLibraryFile(item.id, item.name) }
    catch (error) { setDownloadError(String(error)) }
    finally { setDownloading('') }
  }
  async function move(item: LibraryItem) {
    await controller.setTrashed(item.id, !trashed)
    if (!controller.getSnapshot().actionError && linkedItem?.id === item.id) setLinkedItem(null)
  }
  async function remove(item: LibraryItem) {
    await controller.remove(item.id)
    if (!controller.getSnapshot().actionError && linkedItem?.id === item.id) setLinkedItem(null)
  }
  // A Sources link must find its target even when the entry is outside the first cursor page.
  const includeLinked = linkedItem && !!linkedItem.deletedAt === trashed
    && !state.items.some(item => item.id === linkedItem.id) && !search && !kind && scope === 'all'
  const visibleItems = includeLinked ? [linkedItem, ...state.items] : state.items
  const filtered = search || kind || scope === 'session'

  return (
    <section className="library-page" aria-label="Library">
      <div className="library-content">
        <header className="library-header">
          <div>
            <div className="library-eyebrow"><Library size={16} />Your files</div>
            <h1>Library</h1>
            <p>Images, videos, and files from your conversations.</p>
          </div>
          <div className="library-header-actions">
            <button type="button" aria-label="Refresh library" disabled={state.loading} onClick={() => { void controller.refresh() }}>
              <RefreshCw size={17} />
            </button>
            {onClose && <button type="button" aria-label="Close library" onClick={onClose}><X size={18} /></button>}
          </div>
        </header>
        <div className="library-toolbar">
          <div className="library-filters" role="group" aria-label="File type">
            {filters.map(([value, label]) => (
              <button key={value} type="button" data-filter={value || 'all'} aria-pressed={kind === value} onClick={() => setKind(value)}>{label}</button>
            ))}
          </div>
          <button type="button" className="library-trash-toggle" data-view={trashed ? 'active' : 'trash'} aria-pressed={trashed} onClick={() => setTrashed(value => !value)}>
            {trashed ? <Library size={15} /> : <Trash2 size={15} />} {trashed ? 'Back to library' : 'Trash'}
          </button>
        </div>
        <div className="library-search-row">
          <label className="library-search">
            <Search size={16} />
            <input type="search" placeholder="Search filenames" aria-label="Search filenames" value={search} onChange={event => setSearch(event.target.value)} />
          </label>
          {chatId && (
            <select aria-label="Conversation scope" value={scope} onChange={event => setScope(event.target.value)}>
              <option value="all">All conversations</option><option value="session">Current conversation</option>
            </select>
          )}
        </div>
        {trashed && <p className="library-trash-note">Restore files to Library, or remove them from Mint to move them to system Trash on the computer running Mint.</p>}
        {state.error && (
          <div className="library-error" role="alert"><span>{state.error}</span><button type="button" onClick={() => { void controller.refresh() }}>Retry</button></div>
        )}
        {(state.actionError || downloadError) && <p className="library-action-error" role="alert">{state.actionError || downloadError}</p>}
        {state.loading && !state.items.length ? <p className="library-empty" role="status">Loading library…</p> : !visibleItems.length && !state.error ? (
          <div className="library-empty">
            <Library size={30} />
            <h2>{filtered ? 'No files match these filters' : trashed ? 'Trash is empty' : 'Your library starts here'}</h2>
            <p>{trashed ? 'Files you move to Trash can be restored here.' : 'Send an image, video, or file in a conversation to keep it here.'}</p>
          </div>
        ) : null}
        <div className={`library-items ${kind === 'image' || kind === 'video' ? 'is-grid' : ''}`}>
          {visibleItems.map(item => (
            <LibraryItemCard key={item.id} item={item} linked={linkedItem?.id === item.id} trashed={trashed}
              actionPending={!!state.actionId} downloadPending={!!downloading}
              onPreview={setPreview} onDownload={download} onMove={move} onRemove={remove} />
          ))}
        </div>
        {state.nextCursor && (
          <button type="button" className="library-load-more" disabled={state.loading} onClick={() => { void controller.loadMore() }}>
            {state.loading ? 'Loading…' : 'Load more'}
          </button>
        )}
      </div>
      {preview && <LibraryPreview item={preview} onClose={closePreview} onOpenConversation={onOpenConversation} />}
    </section>
  )
}
