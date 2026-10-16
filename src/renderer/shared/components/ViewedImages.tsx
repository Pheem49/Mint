import { memo, useEffect, useMemo, useRef, useState } from 'react'
import { X } from 'lucide-react'
import type { AgentProgress } from '../types'
import { viewedImagesFrom } from '../utils/viewedImages'
import '../css/viewed-images.css'

function ViewedImage({ image }: { image: ReturnType<typeof viewedImagesFrom>[number] }) {
  const [broken, setBroken] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const dialog = useRef<HTMLDialogElement>(null)
  useEffect(() => { setBroken(false) }, [image.src])
  useEffect(() => {
    if (expanded) dialog.current?.showModal()
  }, [expanded])

  return <figure className="viewed-image" data-viewed-image={image.id}>
    {broken ? <p className="viewed-image-error" role="status">Image preview unavailable</p> :
      <button type="button" className="viewed-image-button" aria-label={`Expand ${image.name}`} onClick={() => setExpanded(true)}>
        <img src={image.src} alt={`Viewed image: ${image.name}`} loading="lazy" onError={() => setBroken(true)} />
      </button>}
    <figcaption title={image.path}>{image.name}</figcaption>
    <dialog ref={dialog} className="viewed-image-dialog" aria-label={`Image preview: ${image.name}`}
      onClose={() => setExpanded(false)} onClick={event => { if (event.target === event.currentTarget) dialog.current?.close() }}>
      <header><span title={image.path}>{image.name}</span><button type="button" aria-label="Close image preview" onClick={() => dialog.current?.close()}><X size={18} /></button></header>
      {expanded && <img src={image.src} alt={image.name} />}
    </dialog>
  </figure>
}

export default memo(function ViewedImages({ progress }: { progress: AgentProgress[] }) {
  const images = useMemo(() => viewedImagesFrom(progress), [progress])
  if (!images.length) return null
  return <section className="viewed-images" aria-label="Images viewed by Mint">
    {images.map(image => <ViewedImage key={image.id} image={image} />)}
  </section>
})
