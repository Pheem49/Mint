import { useEffect, useMemo, useState, memo } from 'react'
import { createPortal } from 'react-dom'
import '../css/pictures.css'
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
  onPreview: (picture: PictureEntry) => void
}

const PictureCardItem = memo(({ picture, filterType, index, onDeleteClick, onPreview }: PictureCardItemProps) => {
  const isVideo = filterType === 'video'
  const fullSrc = getPictureSrc(picture, false)
  const thumbnailSrc = getPictureSrc(picture, true)
  const [imageSrc, setImageSrc] = useState(thumbnailSrc)
  const [imageFailed, setImageFailed] = useState(false)
  const [usedFullSize, setUsedFullSize] = useState(false)

  useEffect(() => {
    setImageSrc(thumbnailSrc)
    setImageFailed(false)
    setUsedFullSize(false)
  }, [picture.id, thumbnailSrc])

  const handleImageError = () => {
    if (!usedFullSize && thumbnailSrc !== fullSrc) {
      setUsedFullSize(true)
      setImageSrc(fullSrc)
    } else {
      setImageFailed(true)
    }
  }

  const retryImage = (event: { stopPropagation: () => void }) => {
    event.stopPropagation()
    setImageFailed(false)
    setUsedFullSize(false)
    setImageSrc(thumbnailSrc)
  }

  return (
    <article className="picture-card" key={picture.id}>
      <button
        type="button"
        className="picture-card-delete-btn"
        title="Delete item"
        aria-label={`Delete ${picture.message || picture.filename}`}
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
        <button type="button" className="picture-card-preview" onClick={() => onPreview(picture)} aria-label={`Preview video ${picture.message || picture.filename}`}>
          <video src={fullSrc} preload="metadata" muted />
          <span className="picture-video-badge">Video</span>
          <span className="picture-open-hint">▶ Play video</span>
        </button>
      ) : imageFailed ? (
        <div className="picture-load-error" role="status">
          <span>Could not load this picture</span>
          <div>
            <button type="button" onClick={retryImage}>Try again</button>
            <a href={fullSrc} target="_blank" rel="noreferrer">Open file</a>
          </div>
        </div>
      ) : (
        <button type="button" className="picture-card-preview" onClick={() => onPreview(picture)} aria-label={`View picture ${picture.message || picture.filename}`}>
          <img
            src={imageSrc}
            alt={picture.message || picture.filename}
            loading={index < 8 ? 'eager' : 'lazy'}
            decoding="async"
            onError={handleImageError}
          />
          <span className="picture-open-hint">View full image</span>
        </button>
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
  const [visibleCount, setVisibleCount] = useState(20)
  const [deletingPicture, setDeletingPicture] = useState<PictureEntry | null>(null)
  const [previewPicture, setPreviewPicture] = useState<PictureEntry | null>(null)
  const [previewFailed, setPreviewFailed] = useState(false)
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
    setVisibleCount(20)
  }, [view, pictures, filterType])

  useEffect(() => {
    if (!previewPicture) return
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setPreviewPicture(null)
    }
    document.addEventListener('keydown', handleKeyDown)
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
      document.body.style.overflow = previousOverflow
    }
  }, [previewPicture])

  const openPreview = (picture: PictureEntry) => {
    setPreviewFailed(false)
    setPreviewPicture(picture)
  }

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
        <div><span className="pictures-kicker">Gallery</span><h2>Saved pictures</h2></div>
        <div className="pictures-header-actions">
          <button className="pictures-close-btn" onClick={() => onSetView('chat')}>Close gallery</button>
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
                onPreview={openPreview}
              />
            ))}
          </div>
          {visibleCount < filteredPictures.length && (
            <div className="pictures-load-more-container">
              <button
                type="button"
                className="pictures-load-more-btn"
                onClick={() => setVisibleCount((prev) => prev + 20)}
              >
                Load More ({filteredPictures.length - visibleCount} remaining)
              </button>
            </div>
          )}
        </>
      )}

      {previewPicture && typeof document !== 'undefined' && createPortal(
        <div className="picture-preview-overlay" role="presentation" onClick={() => setPreviewPicture(null)}>
          <section className="picture-preview-dialog" role="dialog" aria-modal="true" aria-label={previewPicture.message || previewPicture.filename} onClick={(event) => event.stopPropagation()}>
            <header className="picture-preview-header">
              <span title={previewPicture.message || previewPicture.filename}>{previewPicture.message || previewPicture.filename}</span>
              <button type="button" className="picture-preview-close" autoFocus aria-label="Close preview" onClick={() => setPreviewPicture(null)}>×</button>
            </header>
            <div className="picture-preview-content">
              {previewFailed ? (
                <div className="picture-preview-error" role="status">
                  <span>Could not load this file.</span>
                  <a href={getPictureSrc(previewPicture, false)} target="_blank" rel="noreferrer">Open file</a>
                </div>
              ) : filterType === 'video' ? (
                <video src={getPictureSrc(previewPicture, false)} controls onError={() => setPreviewFailed(true)} />
              ) : (
                <img src={getPictureSrc(previewPicture, false)} alt={previewPicture.message || previewPicture.filename} onError={() => setPreviewFailed(true)} />
              )}
            </div>
          </section>
        </div>,
        document.body
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
