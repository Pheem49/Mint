import React, { useState } from 'react'
import '../css/management-views.css'
import { MCP_REGISTRY, type McpRegistryEntry } from '../constants/mcpRegistry'
import { renderMcpSvgIcon } from '../constants/plugins'

/**
 * The "pick a known MCP server" list shown in the MCP Catalog modal (and inline
 * above the Settings › Plugins Add form). Selecting an entry prompts for any
 * `argInputs` it declares, then calls `onPick` with the entry and those values;
 * the host fills the Add form's command / args / env state. Styled with the
 * shared `management-*` classes so it matches the rest of the MCP view.
 */
export interface McpRegistryPickerProps {
  /** Names already in `config.mcpServers` — their rows show "Added". */
  configuredNames: string[]
  onPick: (entry: McpRegistryEntry, argValues: string[], envSeed: Record<string, string>) => void
  /** Show the trailing "— or add manually —" divider. Off when the picker is
   *  its own modal with no manual form beneath it. */
  showManualHint?: boolean
}

export const McpRegistryPicker: React.FC<McpRegistryPickerProps> = ({
  configuredNames,
  onPick,
  showManualHint = true,
}) => {
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
    <div>
      <div className="management-control-bar">
        <div className="management-search-wrapper">
          <input
            type="text"
            className="management-search-input"
            placeholder="Search servers..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <svg
            className="management-search-icon"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
        </div>
      </div>

      <div className="mgmt-row-stack" style={{ maxHeight: 320, overflowY: 'auto' }}>
        {filtered.map((entry) => {
          const added = configured.has(entry.key)
          return (
            <button
              key={entry.key}
              type="button"
              disabled={added}
              onClick={() => open(entry)}
              className="management-plugin-row"
              style={{
                width: '100%',
                textAlign: 'left',
                opacity: added ? 0.55 : 1,
                cursor: added ? 'default' : 'pointer',
                borderColor: selected === entry.key ? 'var(--accent, #10b981)' : undefined,
              }}
            >
              <div className="management-card-icon">{renderMcpSvgIcon(entry.key, entry.icon)}</div>
              <div className="management-plugin-info">
                <div className="management-plugin-name">
                  {entry.name}
                  {added && <span className="management-tag">Added</span>}
                  {(entry.requiredEnv?.length ?? 0) > 0 && !added && (
                    <span className="management-tag">needs API key</span>
                  )}
                </div>
                <div className="management-plugin-desc">{entry.desc}</div>
              </div>
            </button>
          )
        })}
      </div>

      {sel && (
        <div
          style={{
            marginTop: 12,
            padding: '14px 16px',
            border: '1px solid var(--border, rgba(255, 255, 255, 0.08))',
            borderRadius: 'var(--radius-xl, 14px)',
            background: 'var(--surface-bg, rgba(30, 41, 59, 0.45))',
          }}
        >
          <div className="management-card-title-group" style={{ marginBottom: 10 }}>
            <div className="management-card-icon">{renderMcpSvgIcon(sel.key, sel.icon)}</div>
            <h3 className="management-card-title" style={{ fontSize: '1rem' }}>
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
            </h3>
          </div>

          {(sel.argInputs || []).map((input, i) => (
            <div key={input.label} className="management-form-group">
              <label className="management-label">{input.label}</label>
              <input
                type="text"
                className="management-input-field"
                placeholder={input.placeholder || ''}
                value={argValues[i] || ''}
                onChange={(e) => {
                  const next = [...argValues]
                  next[i] = e.target.value
                  setArgValues(next)
                }}
              />
            </div>
          ))}

          {(sel.requiredEnv || []).length > 0 && (
            <p className="management-plugin-desc" style={{ whiteSpace: 'normal', marginTop: 4 }}>
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
            </p>
          )}

          <div style={{ display: 'flex', gap: 8, marginTop: 10 }}>
            <button type="button" className="management-primary-btn" onClick={use} disabled={!argsReady}>
              Use this server
            </button>
            <button type="button" className="management-action-btn" onClick={() => setSelected(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {showManualHint && (
        <div
          className="management-section-title"
          style={{ margin: '16px 0 4px', textAlign: 'center', opacity: 0.5, fontWeight: 400 }}
        >
          — or add manually —
        </div>
      )}
    </div>
  )
}

export default McpRegistryPicker
