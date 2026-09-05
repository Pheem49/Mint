import React, { useState } from 'react'
import '../css/management-views.css'

/**
 * Per-server view of `config.allowedMcpTools[serverName]` — the list the agent's
 * `mcp_tool` gate checks. Mirrors `mint_core::mcp::allow_tool_in` semantics: a
 * `"*"` entry means "any tool", adding a specific tool when `*` is present is a
 * no-op, and an empty list means the agent cannot call the server at all.
 *
 * Styled with the shared `management-*` classes so it matches the MCP view.
 */
export interface McpToolAllowlistProps {
  serverName: string
  config: any
  updateField: (field: string, value: any) => void
  /** Optional — enables the "Discover tools" button. */
  listServerTools?: (name: string) => Promise<string[]>
}

function currentTools(config: any, server: string): string[] {
  const value = config?.allowedMcpTools?.[server]
  return Array.isArray(value) ? value.filter((t: unknown): t is string => typeof t === 'string') : []
}

export const McpToolAllowlist: React.FC<McpToolAllowlistProps> = ({
  serverName,
  config,
  updateField,
  listServerTools,
}) => {
  const tools = currentTools(config, serverName)
  const wildcard = tools.includes('*')
  const [draft, setDraft] = useState('')
  const [discovered, setDiscovered] = useState<string[] | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const write = (next: string[]) => {
    const all = { ...(config?.allowedMcpTools || {}) }
    if (next.length === 0) delete all[serverName]
    else all[serverName] = next
    updateField('allowedMcpTools', all)
  }

  const allow = (raw: string) => {
    const tool = raw.trim()
    if (!tool) return
    if (tool === '*') return write(['*'])
    if (wildcard || tools.includes(tool)) return
    write([...tools, tool])
  }
  const disallow = (tool: string) => write(tools.filter((t) => t !== tool))
  const toggleWildcard = () => (wildcard ? write([]) : write(['*']))

  const discover = async () => {
    if (!listServerTools) return
    setBusy(true)
    setError(null)
    try {
      setDiscovered(await listServerTools(serverName))
    } catch (e: any) {
      setError(e?.message || 'Could not reach the server')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div style={{ marginTop: 16 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
        <span className="management-label" style={{ margin: 0 }}>
          Allowed tools
        </span>
        <label
          style={{ display: 'flex', alignItems: 'center', gap: 6, marginLeft: 'auto', fontSize: '0.85rem' }}
        >
          <input type="checkbox" checked={wildcard} onChange={toggleWildcard} />
          Allow all (*)
        </label>
      </div>

      {wildcard ? (
        <p className="management-plugin-desc" style={{ whiteSpace: 'normal', margin: '4px 0' }}>
          The agent may call every tool on <code>{serverName}</code>.
        </p>
      ) : tools.length === 0 ? (
        <p className="management-plugin-desc" style={{ whiteSpace: 'normal', margin: '4px 0' }}>
          No tools allowed yet — the agent can’t call this server.
        </p>
      ) : (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginBottom: 8 }}>
          {tools.map((t) => (
            <span
              key={t}
              className="management-tag"
              style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
            >
              {t}
              <button
                type="button"
                onClick={() => disallow(t)}
                aria-label={`Disallow ${t}`}
                style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'inherit', opacity: 0.7, padding: 0, lineHeight: 1 }}
              >
                ✕
              </button>
            </span>
          ))}
        </div>
      )}

      {!wildcard && (
        <div style={{ display: 'flex', gap: 8, marginBottom: 8, flexWrap: 'wrap' }}>
          <input
            type="text"
            className="management-input-field"
            placeholder="tool name"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                allow(draft)
                setDraft('')
              }
            }}
            style={{ flex: '1 1 160px', minWidth: 140 }}
          />
          <button
            type="button"
            className="management-action-btn"
            onClick={() => {
              allow(draft)
              setDraft('')
            }}
          >
            Add
          </button>
          {listServerTools && (
            <button type="button" className="management-action-btn" onClick={discover} disabled={busy}>
              {busy ? 'Discovering…' : 'Discover tools'}
            </button>
          )}
        </div>
      )}

      {error && (
        <p className="management-plugin-desc" style={{ whiteSpace: 'normal', color: '#f87171', margin: '4px 0' }}>
          {error}
        </p>
      )}

      {!wildcard && discovered && (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {discovered.length === 0 && (
            <span className="management-plugin-desc">Server reported no tools.</span>
          )}
          {discovered.map((t) => {
            const on = tools.includes(t)
            return (
              <button
                key={t}
                type="button"
                className="management-action-btn"
                onClick={() => (on ? disallow(t) : allow(t))}
                style={{ opacity: on ? 1 : 0.65 }}
              >
                {on ? '✓ ' : '+ '}
                {t}
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

export default McpToolAllowlist
