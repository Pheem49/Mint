import { useState } from 'react'
import { File, ExternalLink } from 'lucide-react'
import { workspaceFileUrl } from '../../utils/workspaceDrag'
import '../../css/workspace-file-info.css'

export default function WorkspaceFileInfo({ path, root }: { path: string; root?: string }) {
  const [opening, setOpening] = useState(false)
  const [error, setError] = useState('')
  const name = path.replace(/\\/g, '/').split('/').pop() || path
  const open = async () => {
    if (!root || opening) return
    setOpening(true); setError('')
    try {
      const target = workspaceFileUrl(root, path)
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('run_desktop_action', { action: { type: 'open_url', target } })
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { setOpening(false) }
  }
  return <section className="workspace-file-info" aria-label="File information">
    <File size={32} aria-hidden="true" />
    <h2>{name}</h2>
    <p>Preview is not available for this file type.</p>
    <dl><dt>File type</dt><dd>{name.includes('.') ? name.split('.').pop()!.toUpperCase() : 'File'}</dd><dt>Path</dt><dd>{path}</dd><dt>Workspace</dt><dd>{root || 'Unavailable'}</dd></dl>
    <button type="button" disabled={!root || opening} aria-busy={opening} onClick={() => { void open() }}><ExternalLink size={15} aria-hidden="true" />{opening ? 'Opening…' : 'Open with external app'}</button>
    {error && <p className="workspace-file-info-error" role="alert">Could not open file: {error}</p>}
  </section>
}
