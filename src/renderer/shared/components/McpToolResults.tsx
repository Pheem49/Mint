import { useEffect, useRef, useState } from 'react'
import type { AgentProgress, McpArtifact } from '../types'
import { mediaPlatform } from '../platform'
import '../css/mcp-results.css'

function Artifact({ artifact }: { artifact: McpArtifact }) {
  const [src, setSrc] = useState('')
  const [error, setError] = useState('')
  const dialog = useRef<HTMLDialogElement>(null)
  useEffect(() => {
    if (artifact.kind === 'link') return
    let active = true
    let loaded = ''
    mediaPlatform.readMcpArtifact(artifact.id).then(url => {
      if (!active) { if (url.startsWith('blob:')) URL.revokeObjectURL(url); return }
      loaded = url; setSrc(url)
    }).catch(() => { if (active) setError('Attachment unavailable') })
    return () => { active = false; if (loaded.startsWith('blob:')) URL.revokeObjectURL(loaded) }
  }, [artifact.id, artifact.kind])
  if (artifact.kind === 'link') {
    // Custom resource URIs are copied as text; only web URLs are navigable.
    return /^https?:\/\//i.test(artifact.uri || '')
      ? <a href={artifact.uri} target="_blank" rel="noreferrer">{artifact.name}</a>
      : <span>{artifact.name}: {artifact.uri}</span>
  }
  return <div className="mcp-artifact">
    {artifact.kind === 'image' && src && <>
      <button type="button" className="mcp-image-button" onClick={() => dialog.current?.showModal()} aria-label={`View ${artifact.name}`}>
        <img src={src} alt={artifact.name} onError={() => setError('Image unavailable')} />
      </button>
      <dialog ref={dialog} className="mcp-image-dialog" aria-label={artifact.name}>
        <button type="button" onClick={() => dialog.current?.close()}>Close</button>
        <img src={src} alt={artifact.name} />
      </dialog>
    </>}
    {src ? <a href={src} download={artifact.name}>{artifact.name} · {(artifact.size / 1024).toFixed(1)} KB</a> : <span>{artifact.name} · {error || 'Loading attachment…'}</span>}
    {error && src && <span role="status">{error}</span>}
  </div>
}

export default function McpToolResults({ progress, running = false, interrupted = false }: { progress: AgentProgress[]; running?: boolean; interrupted?: boolean }) {
  const [, tick] = useState(0)
  const started = useRef(new Map<string, number>())
  const calls = new Map<string, { label: string; update?: Extract<AgentProgress, { type: 'ToolProgress' }>['data']['update']; status?: string; device?: Extract<AgentProgress,{type:'DeviceState'}>['data']['update'] }>()
  const discovery=new Map<string,string>()
  const artifacts = new Map<string, McpArtifact>()
  const warnings = new Set<string>()
  for (const event of progress) {
    if (event.type === 'ToolStart' && event.data.action === 'mcp_tool' && event.data.callId) {
      const id = event.data.callId
      if (!started.current.has(id)) started.current.set(id, Date.now())
      calls.set(id, { label: `${event.data.input.server}/${event.data.input.tool}` })
    } else if (event.type === 'ToolDiscovery') {
      discovery.set(`${event.data.server}/${event.data.tool}`,event.data.message)
    } else if (event.type === 'DeviceState') {
      const {callId,server,tool,update}=event.data
      calls.set(callId,{...calls.get(callId),label:`${server}/${tool}`,device:update})
    } else if (event.type === 'ToolProgress') {
      const { callId, server, tool, update } = event.data
      calls.set(callId, { ...calls.get(callId), label: `${server}/${tool}`, update })
    } else if (event.type === 'ToolEnd' && event.data.callId && event.data.action === 'mcp_tool') {
      const previous = calls.get(event.data.callId)
      calls.set(event.data.callId, { label: previous?.label || `${event.data.input.server}/${event.data.input.tool}`, ...previous, status: event.data.status || (/^(Error:|Blocked|User denied)/.test(event.data.result) ? 'failed' : 'success') })
    } else if (event.type === 'ToolArtifacts') {
      for (const artifact of event.data.artifacts) artifacts.set(artifact.id, artifact)
      for (const warning of event.data.warnings) warnings.add(warning)
    }
  }
  useEffect(() => {
    if (!running) return
    const timer = setInterval(() => tick(n => n + 1), 1000)
    return () => clearInterval(timer)
  }, [running])
  if (!calls.size && !artifacts.size && !warnings.size && !discovery.size) return null
  return <section className="mcp-results" aria-label="MCP results">
    {[...discovery].map(([label,message])=><div key={label} className="mcp-call-status" role="status"><strong>{label}</strong><span>{message}</span></div>)}
    {[...calls].map(([id, call]) => {
      const active = running && !call.status
      const elapsed = Math.max(call.update?.elapsedSecs || 0, Math.floor((Date.now() - (started.current.get(id) || Date.now())) / 1000))
      const percent = call.update?.total ? Math.min(100, Math.round(call.update.progress / call.update.total * 100)) : undefined
      const deviceLabel=call.device && (active && !interrupted || ['verified','failed','unconfirmed'].includes(call.device.state)) ? ({accepted:'Command accepted; awaiting device confirmation',running:'Awaiting device completion',verified:'Device completion verified',failed:'Device reported failure',unconfirmed:'Device completion unconfirmed'}[call.device.state]) : undefined
      const status = call.status === 'cancelled' || (!call.status && interrupted) ? 'Stopped waiting; remote work may still be running.'
        : call.status === 'timed_out' ? 'Timed out; remote completion is unconfirmed.'
        : call.status === 'failed' ? 'Failed' : call.status === 'success' ? 'Completed' : active ? `Working · ${elapsed}s` : 'Interrupted; result unconfirmed'
      return <div key={id} className="mcp-call-status" role="status">
        <strong>{call.label}</strong><span>{deviceLabel || status}</span>
        {call.device && <><span>{call.device.message}</span><span>Target: {JSON.stringify(call.device.target)} · Actual: {JSON.stringify(call.device.actual)}</span></>}
        {active && call.update?.message && <span>{call.update.message}</span>}
        {active && percent !== undefined && <progress max={100} value={percent} aria-label={`${call.label} progress`}>{percent}%</progress>}
      </div>
    })}
    <div className="mcp-artifacts">{[...artifacts.values()].map(artifact => <Artifact key={artifact.id} artifact={artifact} />)}</div>
    {[...warnings].map(warning => <p key={warning} role="status">{warning}</p>)}
  </section>
}
