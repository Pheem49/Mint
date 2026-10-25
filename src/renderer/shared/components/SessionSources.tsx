import { useState } from 'react'
import { File, FileText, Globe, Image, Paperclip, Search, Video } from 'lucide-react'
import type { SessionSource } from '../utils/sessionSources'
import type { ArtifactFile } from './ArtifactPreviewPanel'
import { libraryPlatform } from '../platform'
import '../css/session-sources.css'

function SourceIcon({ source }: { source: SessionSource }) {
  const src = source.kind === 'web' ? source.faviconUrl : source.thumbnailDataUri
  const [failedSrc, setFailedSrc] = useState<string>()
  const Icon = source.kind === 'web' ? Globe : source.kind === 'image' ? Image : source.kind === 'video' ? Video : /\.(pdf|md|txt|docx?)$/i.test(source.name) ? FileText : File
  return <span className={`session-source-icon is-${source.kind}`}>
    {src && src !== failedSrc ? <img src={src} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setFailedSrc(src)} /> : <Icon size={15} aria-hidden="true" />}
  </span>
}

export default function SessionSources({ sources, loading, error, onRetry, onOpenLibrary, onOpenArtifact }: {
  sources: SessionSource[]
  loading: boolean
  error: string
  onRetry: () => void
  onOpenLibrary: (url: string) => void
  onOpenArtifact?: (artifact: ArtifactFile) => void
}) {
  const [query, setQuery] = useState('')
  const [openingSourceId, setOpeningSourceId] = useState<string | null>(null)
  const groups = [
    { label: 'Images', kinds: ['image'] },
    { label: 'Files', kinds: ['file', 'video'] },
    { label: 'Web searches', kinds: ['web'] },
  ]
  const search = query.trim().toLocaleLowerCase()
  const shown = sources.filter(source => !search || [source.name, source.detail, source.url ?? ''].some(value => value.toLocaleLowerCase().includes(search)))

  const handleSourceClick = async (source: SessionSource, event: React.MouseEvent) => {
    if (source.kind === 'web' || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return
    event.preventDefault()

    if (source.kind === 'image' && onOpenArtifact) {
      if (openingSourceId) return
      setOpeningSourceId(source.id)
      try {
        let imageUrl = ''
        let isObjectUrl = false

        if (source.libraryItemId) {
          try {
            const blob = await libraryPlatform.readLibraryFile(source.libraryItemId, false)
            imageUrl = URL.createObjectURL(blob)
            isObjectUrl = true
          } catch (err) {
            console.warn('Could not read full library file for source, falling back to thumbnail:', err)
          }
        }

        if (!imageUrl && source.thumbnailDataUri) {
          imageUrl = source.thumbnailDataUri
          isObjectUrl = false
        }

        if (imageUrl) {
          onOpenArtifact({
            path: `sources/${source.id}/${source.name}`,
            url: imageUrl,
            type: 'image',
            objectUrl: isObjectUrl,
          })
          return
        }
      } finally {
        setOpeningSourceId(null)
      }
    }

    if (source.url) {
      onOpenLibrary(source.url)
    }
  }

  return <section className="session-sources" aria-label="Session sources">
    <header className="session-sources-heading"><span><Paperclip size={13} aria-hidden="true" />Sources <small>This session</small></span><span>{sources.length}</span></header>
    {(sources.length > 8 || query) && <label className="session-sources-search">
      <Search size={14} aria-hidden="true" />
      <input type="search" aria-label="Search Sources" placeholder="Search Sources…" value={query} onChange={event => setQuery(event.target.value)} />
    </label>}
    {loading && <p className="session-sources-empty" role="status">Loading session sources…</p>}
    {error && <div className="session-sources-error" role="status"><span>Could not load saved sources.</span><button type="button" onClick={onRetry}>Retry</button></div>}
    {!loading && !error && sources.length === 0 && <p className="session-sources-empty">Images and files you send, and pages found by web search, will appear here.</p>}
    {sources.length > 0 && shown.length === 0 && <p className="session-sources-empty" role="status">No sources match your search.</p>}
    {groups.map(group => {
      const items = shown.filter(source => group.kinds.includes(source.kind))
      if (!items.length) return null
      return <div className="session-sources-group" key={group.label}>
        <h3>{group.label}<span>{search ? `${items.length} / ${sources.filter(source => group.kinds.includes(source.kind)).length}` : items.length}</span></h3>
        <ul>{items.map(source => {
          const content = <><SourceIcon source={source} /><span className="session-source-copy"><strong>{source.name}</strong><small>{source.detail}</small></span></>
          const isInteractive = Boolean(source.url || (source.kind === 'image' && onOpenArtifact && (source.libraryItemId || source.thumbnailDataUri)))
          const isOpening = openingSourceId === source.id
          return <li key={source.id}>{isInteractive ? <a
            className={`session-source-row${isOpening ? ' is-opening' : ''}`}
            href={source.url || '#'}
            role={source.url ? undefined : 'button'}
            aria-busy={isOpening ? 'true' : undefined}
            target={source.kind === 'web' ? '_blank' : undefined}
            rel={source.kind === 'web' ? 'noopener noreferrer' : undefined}
            title={source.kind === 'image' && onOpenArtifact ? `${source.name}\nOpen image preview in Tools` : `${source.name}\n${source.url || ''}`}
            onClick={event => void handleSourceClick(source, event)}
          >{content}</a> : <div className="session-source-row" title={source.name}>{content}</div>}</li>
        })}</ul>
      </div>
    })}
  </section>
}
