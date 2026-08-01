import { useEffect, useMemo, useState, memo } from 'react'
import { createPortal } from 'react-dom'
import { type PictureEntry, convertFileSrc, isTauriRuntime, getLocalApiBase, deleteSavedPicture } from '../tauri'
import type { DashboardView } from './DashboardSidebar'

const getPictureSrc = (picture: PictureEntry, useThumbnail = false) => {
  if (isTauriRuntime()) {
    if (useThumbnail && (picture.thumbnailPath || picture.thumbnailUrl)) {
      return convertFileSrc(picture.thumbnailPath || picture.thumbnailUrl || '');
    }
    return convertFileSrc(picture.path || picture.url || '');
  } else {
    const apiBase = getLocalApiBase();
    if (useThumbnail && (picture.thumbnailPath || picture.thumbnailUrl)) {
      return `${apiBase}/thumbnails/${picture.id}.thumb.png`;
    }
    return `${apiBase}/pictures/${encodeURIComponent(picture.filename)}`;
  }
}

interface PictureCardItemProps {
  picture: PictureEntry
  filterType: 'photo' | 'video'
  index: number
  onDeleteClick: (picture: PictureEntry) => void
}

const PictureCardItem = memo(({ picture, filterType, index, onDeleteClick }: PictureCardItemProps) => {
  const isVideo = filterType === 'video'
  return (
    <article className="picture-card" key={picture.id}>
      <button
        type="button"
        className="picture-card-delete-btn"
        title="Delete item"
        onClick={(e) => {
          e.stopPropagation()
          onDeleteClick(picture)
        }}
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="3 6 5 6 21 6"></polyline>
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
        </svg>
      </button>
      {isVideo ? (
        <div style={{ position: 'relative', width: '100%', aspectRatio: '4 / 3', display: 'flex', alignItems: 'center', justifyContent: 'center', backgroundColor: '#1a1a1a' }}>
          {picture.thumbnailPath || picture.thumbnailUrl ? (
            <img
              src={getPictureSrc(picture, true)}
              alt={picture.message || picture.filename}
              loading={index < 8 ? 'eager' : 'lazy'}
              decoding="async"
              style={{ width: '100%', height: '100%', objectFit: 'cover' }}
              onError={(e) => {
                const img = e.currentTarget
                if (!img.dataset.fallback) {
                  img.dataset.fallback = 'true'
                  img.src = getPictureSrc(picture, false)
                }
              }}
            />
          ) : (
            <video
              src={getPictureSrc(picture, false)}
              preload="metadata"
              style={{ width: '100%', height: '100%', objectFit: 'cover', pointerEvents: 'none' }}
            />
          )}
          <div style={{
            position: 'absolute',
            top: '8px',
            left: '8px',
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            backgroundColor: 'rgba(0, 0, 0, 0.75)',
            color: '#fff',
            padding: '4px 8px',
            borderRadius: '4px',
            fontSize: '11px',
            fontWeight: 600,
            pointerEvents: 'none',
            backdropFilter: 'blur(4px)',
            border: '1px solid rgba(255, 255, 255, 0.1)'
          }}>
            <span>📹 Video</span>
          </div>
        </div>
      ) : (
        <div style={{ position: 'relative', width: '100%', aspectRatio: '4 / 3', display: 'flex', alignItems: 'center', justifyContent: 'center', backgroundColor: '#1a1a1a' }}>
          <img
            src={getPictureSrc(picture, true)}
            alt={picture.message || picture.filename}
            loading={index < 8 ? 'eager' : 'lazy'}
            decoding="async"
            style={{ width: '100%', height: '100%', objectFit: 'cover' }}
            onError={(e) => {
              const img = e.currentTarget
              if (!img.dataset.fallback) {
                img.dataset.fallback = 'true'
                img.src = getPictureSrc(picture, false)
              }
            }}
          />
        </div>
      )}
      <div className="picture-card-meta"><span>{picture.message || picture.filename}</span></div>
    </article>
  )
})

interface PicturesLibraryProps {
  view: DashboardView
  pictures: PictureEntry[]
  onSetView: (view: DashboardView) => void
  onRefreshPictures?: () => Promise<void>
}

export default function PicturesLibrary({ view, pictures, onSetView, onRefreshPictures }: PicturesLibraryProps) {
  const [filterType, setFilterType] = useState<'photo' | 'video'>('photo')
  const [visibleCount, setVisibleCount] = useState(24)
  const [deletingPicture, setDeletingPicture] = useState<PictureEntry | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)

  const filteredPictures = useMemo(() => {
    return pictures.filter((picture) => {
      const pathLower = (picture.path || '').toLowerCase()
      const urlLower = (picture.url || '').toLowerCase()
      const mimeLower = (picture.mimeType || '').toLowerCase()

      const isVideo =
        mimeLower.startsWith('video/') ||
        pathLower.endsWith('.mp4') ||
        pathLower.endsWith('.webm') ||
        pathLower.endsWith('.mov') ||
        pathLower.endsWith('.avi') ||
        pathLower.endsWith('.mkv') ||
        urlLower.endsWith('.mp4') ||
        urlLower.endsWith('.webm') ||
        urlLower.endsWith('.mov')

      return filterType === 'video' ? isVideo : !isVideo
    })
  }, [pictures, filterType])

  const visiblePictures = useMemo(
    () => filteredPictures.slice(0, visibleCount),
    [filteredPictures, visibleCount],
  )

  useEffect(() => {
    setVisibleCount(24)
  }, [view, pictures, filterType])

  const handleDeleteConfirm = async () => {
    if (!deletingPicture) return
    const target = deletingPicture
    setIsDeleting(true)
    try {
      await deleteSavedPicture(target.id || target.filename)
      await onRefreshPictures?.()
    } catch (err) {
      console.error('Failed to delete picture:', err)
    } finally {
      setIsDeleting(false)
      setDeletingPicture(null)
    }
  }

  return (
    <section className={`pictures-library ${view === 'pictures' ? 'is-visible' : ''}`} aria-hidden={view !== 'pictures'}>
      <header className="pictures-header">
        <div><span className="pictures-kicker">Gallery</span><h2>Saved Pictures</h2></div>
        <div className="pictures-header-actions">
          <button className="pictures-close-btn" onClick={() => onSetView('chat')}>Close Gallery</button>
          <button type="button" className="picture-refresh-btn" title="Refresh" onClick={() => onRefreshPictures?.()}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.13-3.36L23 10"/><path d="M20.49 15a9 9 0 0 1-14.13 3.36L1 14"/></svg>
          </button>
        </div>
      </header>

      {/* Tabs Filter */}
      <div className="pictures-type-filter">
        <button
          type="button"
          className={`filter-tab-btn ${filterType === 'photo' ? 'active' : ''}`}
          onClick={() => setFilterType('photo')}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
            <circle cx="8.5" cy="8.5" r="1.5"></circle>
            <polyline points="21 15 16 10 5 21"></polyline>
          </svg>
          Photos
        </button>
        <button
          type="button"
          className={`filter-tab-btn ${filterType === 'video' ? 'active' : ''}`}
          onClick={() => setFilterType('video')}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polygon points="23 7 16 12 23 17 23 7"></polygon>
            <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
          </svg>
          Videos
        </button>
      </div>

      {filteredPictures.length === 0 ? (
        <div className="pictures-empty">
          <div className="pictures-empty-icon" style={{ display: 'flex', justifyContent: 'center', marginBottom: '12px', opacity: 0.3 }}>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline>
            </svg>
          </div>
          <p>No saved {filterType === 'video' ? 'videos' : 'pictures'} yet</p>
          <span>Items appear here after a message with an attachment is sent successfully.</span>
        </div>
      ) : (
        <>
          <div className="pictures-grid">
            {visiblePictures.map((picture, index) => (
              <PictureCardItem
                key={picture.id}
                picture={picture}
                filterType={filterType}
                index={index}
                onDeleteClick={setDeletingPicture}
              />
            ))}
          </div>
          {visibleCount < filteredPictures.length && (
            <div className="pictures-load-more-container">
              <button
                type="button"
                className="pictures-load-more-btn"
                onClick={() => setVisibleCount((prev) => prev + 24)}
              >
                Load More ({filteredPictures.length - visibleCount} remaining)
              </button>
            </div>
          )}
        </>
      )}

      {deletingPicture && typeof document !== 'undefined' && createPortal(
        <div className="picture-delete-modal-overlay" onClick={() => setDeletingPicture(null)}>
          <div className="picture-delete-modal" onClick={(e) => e.stopPropagation()}>
            <div className="picture-delete-modal-header">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#ef4444" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="3 6 5 6 21 6"></polyline>
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
              </svg>
              <span>Delete {filterType === 'video' ? 'Video' : 'Picture'}</span>
            </div>
            <p className="picture-delete-modal-body">
              Are you sure you want to permanently delete <strong>{deletingPicture.message || deletingPicture.filename}</strong>? This action cannot be undone.
            </p>
            <div className="picture-delete-modal-actions">
              <button
                type="button"
                className="picture-modal-btn cancel"
                onClick={() => setDeletingPicture(null)}
                disabled={isDeleting}
              >
                Cancel
              </button>
              <button
                type="button"
                className="picture-modal-btn delete"
                onClick={handleDeleteConfirm}
                disabled={isDeleting}
              >
                {isDeleting ? 'Deleting...' : 'Delete'}
              </button>
            </div>
          </div>
        </div>,
        document.body
      )}
    </section>
  )
}
