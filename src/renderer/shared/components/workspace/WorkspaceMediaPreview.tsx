import { useEffect, useRef, useState } from 'react'
import WorkspaceFileInfo from './WorkspaceFileInfo'
import '../../css/workspace-media-preview.css'

export default function WorkspaceMediaPreview({ kind, url, path, root, mimeType }: { kind: 'audio' | 'video'; url: string; path: string; root?: string; mimeType?: string }) {
  const player = useRef<HTMLMediaElement>(null)
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    const media = player.current
    return () => { if (media) { media.pause(); media.removeAttribute('src'); media.querySelector('source')?.removeAttribute('src'); media.load() } }
  }, [url])
  return <section className={`workspace-media-preview is-${kind}`} aria-label={`${kind === 'audio' ? 'Audio' : 'Video'} preview`}>
    {failed ? <><p role="alert" className="workspace-media-error">This file could not be played in Mint. Open it with an external app instead.</p><WorkspaceFileInfo path={path} root={root} /></> : kind === 'audio'
      ? <audio ref={player as React.RefObject<HTMLAudioElement>} controls preload="metadata" aria-label={path} onError={() => setFailed(true)}><source src={url} type={mimeType} /></audio>
      : <video ref={player as React.RefObject<HTMLVideoElement>} controls preload="metadata" playsInline aria-label={path} onError={() => setFailed(true)}><source src={url} type={mimeType} /></video>}
  </section>
}
