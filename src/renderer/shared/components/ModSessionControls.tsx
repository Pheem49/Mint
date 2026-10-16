import type { ModTestReport } from '../mods'

export function ModSessionControls({ enabled, busy, onToggle }: { enabled: boolean; busy: boolean; onToggle: (enabled: boolean) => void }) {
  return <div className="mod-session-controls"><button type="button" disabled={busy} aria-pressed={enabled} onClick={() => onToggle(!enabled)}>Hot reload: {enabled ? 'On' : 'Off'}</button><p className="mod-hint">Applies to this session. Mods with permissions require review for each new revision.</p></div>
}
export function ModTestResults({ report }: { report: ModTestReport }) {
  return <section aria-label="Mod test results"><p role="status">{report.passed} passed · {report.failed} failed</p>{report.tests.map((test,i) => <div key={i}><strong>{test.passed ? '✓' : '✗'} {test.name}</strong><p className="mod-hint">{test.file}</p>{test.error && <pre className="mod-error">{test.error}</pre>}</div>)}</section>
}
