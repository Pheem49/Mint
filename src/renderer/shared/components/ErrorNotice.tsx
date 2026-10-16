import '../css/error-notice.css'

export default function ErrorNotice({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  const summary = /HTTP\s+429\b|"code"\s*:\s*429/.test(message)
    ? 'The provider is temporarily rate-limited (HTTP 429). Wait briefly or select another model.'
    : message.includes('invalid arguments for tool')
      ? 'AI sent invalid tool arguments. Correct the arguments and try again.'
      : message.length > 240 ? `${message.slice(0, 240)}…` : message

  return (
    <section className="mint-error mint-error-notice" role="alert" aria-label="Error notification">
      <div className="mint-error-notice-content">
        <p>{summary}</p>
        {summary !== message && (
          <details>
            <summary>Error details</summary>
            <pre>{message.slice(0, 12000)}{message.length > 12000 ? '\n… (details truncated)' : ''}</pre>
          </details>
        )}
      </div>
      <button type="button" className="mint-error-notice-close" aria-label="Dismiss error" onClick={onDismiss}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </section>
  )
}
