import React, { useState } from 'react'
import { MCP_REGISTRY, type McpRegistryEntry } from '../constants/mcpRegistry'

/**
 * The "pick a known MCP server" grid shown above the manual Add form. Selecting
 * an entry prompts for any `argInputs` it declares, then calls `onPick` with the
 * entry and those values; the host fills the form's command / args / env state.
 * Self-contained inline styling so it drops into both the dashboard Add modal
 * and the Settings › Plugins Add box.
 */
export interface McpRegistryPickerProps {
  /** Names already in `config.mcpServers` — their cards show "Added". */
  configuredNames: string[]
  onPick: (entry: McpRegistryEntry, argValues: string[], envSeed: Record<string, string>) => void
}

const cardStyle: React.CSSProperties = {
  textAlign: 'left',
  padding: '8px 10px',
  border: '1px solid rgba(128,128,128,0.35)',
  borderRadius: 8,
  background: 'rgba(255,255,255,0.03)',
  cursor: 'pointer',
  fontSize: '0.85rem',
}

export const McpRegistryPicker: React.FC<McpRegistryPickerProps> = ({ configuredNames, onPick }) => {
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<string | null>(null)
  const [argValues, setArgValues] = useState<string[]>([])

  const configured = new Set(configuredNames)
  const filtered = MCP_REGISTRY.filter(
    (e) =>
      e.name.toLowerCase().includes(query.toLowerCase()) ||
      e.key.toLowerCase().includes(query.toLowerCase()) ||
      (e.desc || '').toLowerCase().includes(query.toLowerCase()),
  )
  const sel = MCP_REGISTRY.find((e) => e.key === selected) || null

  const open = (entry: McpRegistryEntry) => {
    setSelected(entry.key)
    setArgValues((entry.argInputs || []).map(() => ''))
  }

  const use = () => {
    if (!sel) return
    const envSeed: Record<string, string> = {}
    for (const v of sel.requiredEnv || []) envSeed[v.key] = ''
    onPick(sel, argValues, envSeed)
    setSelected(null)
    setArgValues([])
    setQuery('')
  }

  const argsReady = !sel || (sel.argInputs || []).every((_, i) => argValues[i]?.trim())

  return (
    <div style={{ marginBottom: 16 }}>
      <div style={{ fontSize: '0.8rem', fontWeight: 600, opacity: 0.8, marginBottom: 8 }}>
        From catalog
      </div>
      <input
        type="text"
        placeholder="Search servers…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        style={{ width: '100%', marginBottom: 8 }}
      />

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fill, minmax(180px, 1fr))',
          gap: 8,
          maxHeight: 200,
          overflowY: 'auto',
        }}
      >
        {filtered.map((entry) => {
          const added = configured.has(entry.key)
          return (
            <button
              key={entry.key}
              type="button"
              disabled={added}
              onClick={() => open(entry)}
              style={{
                ...cardStyle,
                opacity: added ? 0.5 : 1,
                cursor: added ? 'default' : 'pointer',
                borderColor: selected === entry.key ? 'var(--accent, #10b981)' : cardStyle.border as string,
              }}
            >
              <div style={{ fontWeight: 600 }}>
                {entry.icon ? `${entry.icon} ` : ''}
                {entry.name}
                {added && <span style={{ opacity: 0.7, fontWeight: 400 }}> · Added</span>}
              </div>
              <div style={{ opacity: 0.7, marginTop: 2 }}>{entry.desc}</div>
              {(entry.requiredEnv?.length ?? 0) > 0 && (
                <div style={{ opacity: 0.6, marginTop: 2, fontSize: '0.78rem' }}>needs an API key</div>
              )}
            </button>
          )
        })}
      </div>

      {sel && (
        <div
          style={{
            marginTop: 10,
            padding: '10px 12px',
            border: '1px solid rgba(128,128,128,0.35)',
            borderRadius: 8,
          }}
        >
          <div style={{ fontWeight: 600, marginBottom: 6 }}>
            {sel.icon ? `${sel.icon} ` : ''}
            {sel.name}
            {sel.docs && (
              <a
                href={sel.docs}
                target="_blank"
                rel="noreferrer"
                style={{ marginLeft: 8, fontSize: '0.8rem', fontWeight: 400 }}
              >
                docs ↗
              </a>
            )}
          </div>

          {(sel.argInputs || []).map((input, i) => (
            <div key={input.label} style={{ marginBottom: 6 }}>
              <label style={{ display: 'block', fontSize: '0.8rem', opacity: 0.8 }}>{input.label}</label>
              <input
                type="text"
                placeholder={input.placeholder || ''}
                value={argValues[i] || ''}
                onChange={(e) => {
                  const next = [...argValues]
                  next[i] = e.target.value
                  setArgValues(next)
                }}
                style={{ width: '100%' }}
              />
            </div>
          ))}

          {(sel.requiredEnv || []).length > 0 && (
            <div style={{ fontSize: '0.8rem', opacity: 0.8, margin: '4px 0 8px' }}>
              You’ll fill in{' '}
              {(sel.requiredEnv || []).map((v, i) => (
                <React.Fragment key={v.key}>
                  {i > 0 && ', '}
                  <code>{v.key}</code>
                  {v.help && (
                    <a href={v.help} target="_blank" rel="noreferrer" style={{ marginLeft: 4 }}>
                      get key ↗
                    </a>
                  )}
                </React.Fragment>
              ))}{' '}
              below.
            </div>
          )}

          <div style={{ display: 'flex', gap: 8 }}>
            <button type="button" onClick={use} disabled={!argsReady}>
              Use this server
            </button>
            <button type="button" onClick={() => setSelected(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}

      <div style={{ margin: '12px 0 4px', textAlign: 'center', opacity: 0.5, fontSize: '0.8rem' }}>
        — or add manually —
      </div>
    </div>
  )
}

export default McpRegistryPicker
