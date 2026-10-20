import { memo, useEffect, useMemo, useRef, useState } from 'react'
import { X } from 'lucide-react'
import type { AgentProgress } from '../types'
import { viewedImagesFrom, type ViewedImageItem } from '../utils/viewedImages'
import { mediaPlatform } from '../platform'
import '../css/viewed-images.css'

export function ViewedImage({ image, chatId, variant = 'card' }: { image: ViewedImageItem; chatId?: string; variant?: 'card' | 'inline' }) {
  const [src, setSrc] = useState(image.src || '')
  const [broken, setBroken] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const dialog = useRef<HTMLDialogElement>(null)

  useEffect(() => {
    if (image.src) {
      setSrc(image.src)
      setBroken(false)
      return
    }
    if (image.artifactId) {
      let active = true
      let loaded = ''
      mediaPlatform.readMcpArtifact(image.artifactId, chatId).then(url => {
        if (!active) { if (url.startsWith('blob:')) URL.revokeObjectURL(url); return }
        loaded = url
        setSrc(url)
        setBroken(false)
      }).catch(() => {
        if (active) setBroken(true)
      })
      return () => {
        active = false
        if (loaded.startsWith('blob:')) URL.revokeObjectURL(loaded)
      }
    }
  }, [image.src, image.artifactId, chatId])

  useEffect(() => {
    if (expanded) dialog.current?.showModal()
  }, [expanded])

  if (variant === 'inline') {
    return (
      <figure className="viewed-image-inline" data-activity-image={image.id}>
        {broken || !src ? (
          <p className="viewed-image-inline-status" role="status">
            {broken ? 'Image preview unavailable' : 'Loading image…'}
          </p>
        ) : (
          <button
            type="button"
            className="viewed-image-inline-btn"
            aria-label={`Expand ${image.name}`}
            onClick={() => setExpanded(true)}
          >
            <img src={src} alt={`Viewed image: ${image.name}`} loading="lazy" onError={() => setBroken(true)} />
            <span className="viewed-image-inline-overlay">
              <span className="viewed-image-inline-badge">Click to zoom</span>
            </span>
          </button>
        )}
        <dialog
          ref={dialog}
          className="viewed-image-dialog"
          aria-label={`Image preview: ${image.name}`}
          onClose={() => setExpanded(false)}
          onClick={event => { if (event.target === event.currentTarget) dialog.current?.close() }}
        >
          <header>
            <span title={image.path}>{image.name}</span>
            <button type="button" aria-label="Close image preview" onClick={() => dialog.current?.close()}>
              <X size={18} />
            </button>
          </header>
          {expanded && src && <img src={src} alt={image.name} />}
        </dialog>
      </figure>
    )
  }

  return <figure className="viewed-image" data-viewed-image={image.id}>
    {broken || !src ? <p className="viewed-image-error" role="status">{broken ? 'Image preview unavailable' : 'Loading image…'}</p> :
      <button type="button" className="viewed-image-button" aria-label={`Expand ${image.name}`} onClick={() => setExpanded(true)}>
        <img src={src} alt={`Viewed image: ${image.name}`} loading="lazy" onError={() => setBroken(true)} />
      </button>}
    <figcaption title={image.path}>{image.name}</figcaption>
    <dialog ref={dialog} className="viewed-image-dialog" aria-label={`Image preview: ${image.name}`}
      onClose={() => setExpanded(false)} onClick={event => { if (event.target === event.currentTarget) dialog.current?.close() }}>
      <header><span title={image.path}>{image.name}</span><button type="button" aria-label="Close image preview" onClick={() => dialog.current?.close()}><X size={18} /></button></header>
      {expanded && src && <img src={src} alt={image.name} />}
    </dialog>
  </figure>
}

export default memo(function ViewedImages({ progress, chatId }: { progress: AgentProgress[]; chatId?: string }) {
  const images = useMemo(() => viewedImagesFrom(progress), [progress])
  if (!images.length) return null
  return <section className="viewed-images" aria-label="Images viewed by Mint">
    {images.map(image => <ViewedImage key={image.id} image={image} chatId={chatId} />)}
  </section>
})

