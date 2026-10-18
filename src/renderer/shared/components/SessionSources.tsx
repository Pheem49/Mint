import { useState } from 'react'
import { File, FileText, Globe, Image, Paperclip, Search, Video } from 'lucide-react'
import type { SessionSource } from '../utils/sessionSources'
import '../css/session-sources.css'

function SourceIcon({ source }: { source: SessionSource }) {
  const src = source.kind === 'web' ? source.faviconUrl : source.thumbnailDataUri
  const [failedSrc, setFailedSrc] = useState<string>()
  const Icon = source.kind === 'web' ? Globe : source.kind === 'image' ? Image : source.kind === 'video' ? Video : /\.(pdf|md|txt|docx?)$/i.test(source.name) ? FileText : File
  return <span className={`session-source-icon is-${source.kind}`}>
    {src && src !== failedSrc ? <img src={src} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setFailedSrc(src)} /> : <Icon size={15} aria-hidden="true" />}
  </span>
}

export default function SessionSources({ sources, loading, error, onRetry, onOpenLibrary }: {
  sources: SessionSource[]; loading: boolean; error: string; onRetry: () => void; onOpenLibrary: (url: string) => void
}) {
  const [query, setQuery] = useState('')
  const groups = [
    { label: 'Images', kinds: ['image'] },
    { label: 'Files', kinds: ['file', 'video'] },
    { label: 'Web searches', kinds: ['web'] },
  ]
  const search = query.trim().toLocaleLowerCase()
  const shown = sources.filter(source => !search || [source.name, source.detail, source.url ?? ''].some(value => value.toLocaleLowerCase().includes(search)))
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
          return <li key={source.id}>{source.url ? <a className="session-source-row" href={source.url} target={source.kind === 'web' ? '_blank' : undefined} rel={source.kind === 'web' ? 'noopener noreferrer' : undefined} title={`${source.name}\n${source.url}`} onClick={event => {
            if (source.kind === 'web' || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return
            event.preventDefault()
            onOpenLibrary(source.url)
          }}>{content}</a> : <div className="session-source-row" title={source.name}>{content}</div>}</li>
        })}</ul>
      </div>
    })}
  </section>
}
