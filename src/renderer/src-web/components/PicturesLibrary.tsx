import { useEffect, useMemo, useState } from 'react'
import { type PictureEntry, convertFileSrc } from '../tauri'
import type { DashboardView } from './DashboardSidebar'

const INITIAL_VISIBLE_PICTURES = 18
const PICTURE_RENDER_BATCH_SIZE = 18
const PICTURE_RENDER_BATCH_DELAY_MS = 80

interface PicturesLibraryProps {
  view: DashboardView
  pictures: PictureEntry[]
  onSetView: (view: DashboardView) => void
}

export default function PicturesLibrary({ view, pictures, onSetView }: PicturesLibraryProps) {
  const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_PICTURES)
  const visiblePictures = useMemo(
    () => pictures.slice(0, visibleCount),
    [pictures, visibleCount],
  )

  useEffect(() => {
    if (view !== 'pictures') {
      setVisibleCount(INITIAL_VISIBLE_PICTURES)
      return
    }

    setVisibleCount(INITIAL_VISIBLE_PICTURES)
  }, [view, pictures])

  useEffect(() => {
    if (view !== 'pictures' || visibleCount >= pictures.length) return

    const timer = window.setTimeout(() => {
      setVisibleCount((current) => Math.min(current + PICTURE_RENDER_BATCH_SIZE, pictures.length))
    }, PICTURE_RENDER_BATCH_DELAY_MS)

    return () => window.clearTimeout(timer)
  }, [view, pictures.length, visibleCount])

  if (view !== 'pictures') return null

  return (
    <section className="pictures-library">
      <header className="pictures-header">
        <div><span className="pictures-kicker">Gallery</span><h2>Saved Pictures</h2></div>
        <div className="pictures-header-actions">
          <button className="pictures-close-btn" onClick={() => onSetView('chat')}>Close Gallery</button>
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
          {visiblePictures.map((picture, index) => (
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
