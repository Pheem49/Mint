import { type PictureEntry, convertFileSrc } from '../tauri'
import type { DashboardView } from './DashboardSidebar'

interface PicturesLibraryProps {
  view: DashboardView
  pictures: PictureEntry[]
  onSetView: (view: DashboardView) => void
  onRefreshPictures?: () => Promise<void>
}

export default function PicturesLibrary({ view, pictures, onSetView, onRefreshPictures }: PicturesLibraryProps) {
  return (
    <section className={`pictures-library ${view === 'pictures' ? 'is-visible' : ''}`} aria-hidden={view !== 'pictures'}>
      <header className="pictures-header">
        <div><span className="pictures-kicker">Gallery</span><h2>Saved Pictures</h2></div>
        <div className="pictures-header-actions">
          <button className="pictures-close-btn" onClick={() => onSetView('chat')}>Close Gallery</button>
          <button type="button" className="picture-refresh-btn" title="Refresh" onClick={() => onRefreshPictures?.()}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.13-3.36L23 10"/><path d="M20.49 15a9 9 0 0 1-14.13 3.36L1 14"/></svg>
          </button>
          <button type="button" className="picture-folder-btn" disabled={pictures.length === 0} onClick={() => window.settingsApi?.openFolder(pictures[0]?.path || '')}>
            Open Folder
          </button>
        </div>
      </header>
      {pictures.length === 0 ? (
        <div className="pictures-empty">
          <div className="pictures-empty-icon" style={{ display: 'flex', justifyContent: 'center', marginBottom: '12px', opacity: 0.3 }}>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline>
            </svg>
          </div>
          <p>No saved pictures yet</p>
          <span>Images appear here after a message with an attachment is sent successfully.</span>
        </div>
      ) : (
        <div className="pictures-grid">
          {pictures.map((picture, index) => (
            <article className="picture-card" key={picture.id}>
              <img src={convertFileSrc(picture.thumbnailPath || picture.thumbnailUrl || picture.path)} alt={picture.message || picture.filename} loading={index < 6 ? 'eager' : 'lazy'} decoding="async" />
              <div className="picture-card-meta"><span>{picture.message || picture.filename}</span></div>
            </article>
          ))}
        </div>
      )}
    </section>
  )
}
