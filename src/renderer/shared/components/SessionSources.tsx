import { useState } from 'react'
import { File, FileText, Globe, Image, Paperclip, Video } from 'lucide-react'
import type { SessionSource } from '../utils/sessionSources'
import '../css/session-sources.css'

export default function SessionSources({ sources, loading, error, onRetry, onOpenLibrary }: {
  sources: SessionSource[]; loading: boolean; error: string; onRetry: () => void; onOpenLibrary: (url: string) => void
}) {
  const [expanded, setExpanded] = useState(false)
  const groups = [
    { label: 'Images', kinds: ['image'] },
    { label: 'Files', kinds: ['file', 'video'] },
    { label: 'Web searches', kinds: ['web'] },
  ]
  const initialSources = groups.flatMap(group => sources.filter(source => group.kinds.includes(source.kind)).slice(0, 2))
  const shown = expanded ? sources : initialSources
  return <section className="session-sources" aria-label="Session sources">
    <header className="session-sources-heading"><span><Paperclip size={13} aria-hidden="true" />Sources <small>This session</small></span><span>{sources.length}</span></header>
    {loading && <p className="session-sources-empty" role="status">Loading session sources…</p>}
    {error && <div className="session-sources-error" role="status"><span>Could not load saved sources.</span><button type="button" onClick={onRetry}>Retry</button></div>}
    {!loading && !error && sources.length === 0 && <p className="session-sources-empty">Images and files you send, and pages found by web search, will appear here.</p>}
    {groups.map(group => {
      const items = shown.filter(source => group.kinds.includes(source.kind))
      if (!items.length) return null
      return <div className="session-sources-group" key={group.label}>
        <h3>{group.label}</h3>
        <ul>{items.map(source => {
          const Icon = source.kind === 'web' ? Globe : source.kind === 'image' ? Image : source.kind === 'video' ? Video : /\.(pdf|md|txt|docx?)$/i.test(source.name) ? FileText : File
          const content = <><span className={`session-source-icon is-${source.kind}`}>{source.thumbnailDataUri ? <img src={source.thumbnailDataUri} alt="" onError={event => { event.currentTarget.style.display = 'none' }} /> : <Icon size={15} aria-hidden="true" />}</span><span className="session-source-copy"><strong>{source.name}</strong><small>{source.detail}</small></span></>
          return <li key={source.id}>{source.url ? <a className="session-source-row" href={source.url} target={source.kind === 'web' ? '_blank' : undefined} rel={source.kind === 'web' ? 'noopener noreferrer' : undefined} title={source.url} onClick={event => {
            if (source.kind === 'web' || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return
            event.preventDefault()
            onOpenLibrary(source.url)
          }}>{content}</a> : <div className="session-source-row" title={source.name}>{content}</div>}</li>
        })}</ul>
      </div>
    })}
    {sources.length > initialSources.length && <button type="button" className="session-sources-more" aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>{expanded ? 'Show less' : `Show ${sources.length - shown.length} more sources`}</button>}
  </section>
}
