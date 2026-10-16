import '../css/tools-panel.css'

export default function ToolsPanelToggle({ open, count = 0, onToggle, className = '' }: {
  open: boolean; count?: number; onToggle: () => void; className?: string
}) {
  return <button type="button" className={`tools-panel-toggle ${className}${open ? ' is-open' : ''}`}
    aria-label={open ? 'Close tools panel' : 'Open tools panel'} aria-expanded={open}
    title={open ? 'Close tools panel' : 'Open tools panel'} onClick={onToggle}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16M18 8h1M18 12h1M18 16h1"/></svg>
    <span className="tools-toggle-label">Tools</span>
    {count > 0 && <span className="tools-task-count" aria-label={`${count} active background tasks`}>{count}</span>}
  </button>
}
