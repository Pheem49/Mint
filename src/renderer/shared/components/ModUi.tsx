import { useEffect, useState } from 'react'
import type { ModNode } from '../mods'

function ModInput({ node, color, onAction }: { node: ModNode; color: string; onAction: (action: string, value?: string) => void }) {
  const value = String(node.value ?? '')
  const [draft, setDraft] = useState(value)
  useEffect(() => { setDraft(value) }, [value, node.action])
  return <label className={`mod-input${color}`}>{node.label}<input value={draft} onChange={event => setDraft(event.target.value)} onKeyDown={event => {
    if (event.key === 'Enter' && node.action) { event.preventDefault(); onAction(node.action, event.currentTarget.value) }
  }} /></label>
}

export function ModUi({ node, onAction, inheritedColor }: { node: ModNode; onAction: (action: string, value?: string) => void; inheritedColor?: ModNode['color'] }) {
  const selectedColor = node.color ?? inheritedColor
  const color = ['default', 'yellow', 'cyan', 'blue', 'magenta', 'red'].includes(selectedColor ?? '') ? ` mod-color-${selectedColor}` : ''
  switch (node.type) {
    case 'stack': return <div className={`mod-stack${node.direction === 'row' ? ' mod-stack-row' : ''}${color}`}>{node.children?.map((child, i) => <ModUi key={i} node={child} onAction={onAction} inheritedColor={selectedColor} />)}</div>
    case 'text': return <p className={`mod-text${color}`}>{node.text}</p>
    case 'badge': return <span className={`mod-badge${color}`}>{node.text}</span>
    case 'button': return <button className={color.trim() || undefined} type="button" onClick={() => node.action && onAction(node.action)}>{node.label}</button>
    case 'input': return <ModInput node={node} color={color} onAction={onAction} />
    case 'progress': return <label className={`mod-progress${color}`}>{node.label}<progress aria-label={node.label} value={Number(node.value)} max={node.max} /></label>
    case 'table': return <div className={`mod-table-scroll${color}`}><table><thead><tr>{node.columns?.map((column, i) => <th key={i}>{column}</th>)}</tr></thead><tbody>{node.rows?.map((row, i) => <tr key={i}>{row.map((cell, j) => <td key={j}>{cell}</td>)}</tr>)}</tbody></table></div>
    default: return null
  }
}
