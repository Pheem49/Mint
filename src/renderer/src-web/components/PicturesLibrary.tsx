import { useEffect, useMemo, useState } from 'react'
import { type PictureEntry, convertFileSrc, isTauriRuntime, getLocalApiBase } from '../tauri'
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

interface PicturesLibraryProps {
  view: DashboardView
  pictures: PictureEntry[]
  onSetView: (view: DashboardView) => void
  onRefreshPictures?: () => Promise<void>
}

export default function PicturesLibrary({ view, pictures, onSetView, onRefreshPictures }: PicturesLibraryProps) {
  const [filterType, setFilterType] = useState<'photo' | 'video'>('photo')
  const [visibleCount, setVisibleCount] = useState(24)
  const [failedImages, setFailedImages] = useState<Set<string>>(new Set())

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
            {visiblePictures.map((picture, index) => {
              const isVideo = filterType === 'video'
              return (
                <article className="picture-card" key={picture.id}>
                  {isVideo ? (
                    <div style={{ position: 'relative', width: '100%', aspectRatio: '4 / 3', display: 'flex', alignItems: 'center', justifyContent: 'center', backgroundColor: '#1a1a1a' }}>
                      {failedImages.has(picture.id) ? (
                        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '8px', color: '#666' }}>
                          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.5 }}>
                            <path d="M23 7l-7 5 7 5V7z"></path>
                            <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
                          </svg>
                          <span style={{ fontSize: '11px', fontWeight: 500, letterSpacing: '0.05em', textTransform: 'uppercase', opacity: 0.8 }}>Video</span>
                        </div>
                      ) : (
                        <>
                          {picture.thumbnailPath || picture.thumbnailUrl ? (
                            <img
                              src={getPictureSrc(picture, true)}
                              alt={picture.message || picture.filename}
                              loading={index < 6 ? 'eager' : 'lazy'}
                              decoding="async"
                              style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                              onError={() => setFailedImages((prev) => new Set([...prev, picture.id]))}
                            />
                          ) : (
                            <video
                              src={getPictureSrc(picture, false)}
                              preload="metadata"
                              style={{ width: '100%', height: '100%', objectFit: 'cover', pointerEvents: 'none' }}
                              onError={() => setFailedImages((prev) => new Set([...prev, picture.id]))}
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
                        </>
                      )}
                    </div>
                  ) : (
                    <div style={{ position: 'relative', width: '100%', aspectRatio: '4 / 3', display: 'flex', alignItems: 'center', justifyContent: 'center', backgroundColor: '#1a1a1a' }}>
                      {failedImages.has(picture.id) ? (
                        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.4 }}>
                          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                          <circle cx="8.5" cy="8.5" r="1.5"></circle>
                          <polyline points="21 15 16 10 5 21"></polyline>
                        </svg>
                      ) : (
                        <img
                          src={getPictureSrc(picture, true)}
                          alt={picture.message || picture.filename}
                          loading={index < 6 ? 'eager' : 'lazy'}
                          decoding="async"
                          style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                          onError={() => setFailedImages((prev) => new Set([...prev, picture.id]))}
                        />
                      )}
                    </div>
                  )}
                  <div className="picture-card-meta"><span>{picture.message || picture.filename}</span></div>
                </article>
              )
            })}
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
    </section>
  )
}
