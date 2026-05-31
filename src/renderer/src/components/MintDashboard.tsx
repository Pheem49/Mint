import { type FormEvent, useEffect, useState } from 'react'
import {
  getRecentInteractions,
  getRuntimeStatus,
  streamChatMessage,
  type InteractionMemory,
  type RuntimeStatus,
} from '../tauri'

export default function MintDashboard() {
  const [status, setStatus] = useState<RuntimeStatus | null>(null)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const [reply, setReply] = useState('')
  const [interactions, setInteractions] = useState<InteractionMemory[]>([])
  const [sending, setSending] = useState(false)

  useEffect(() => {
    getRuntimeStatus()
      .then(setStatus)
      .catch((reason: unknown) => {
        setError(reason instanceof Error ? reason.message : String(reason))
      })
    getRecentInteractions().then(setInteractions).catch(() => {})
  }, [])

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!message.trim() || sending) return
    setSending(true)
    setError('')
    try {
      setReply('')
      const response = await streamChatMessage(message.trim(), (chunk) => {
        setReply((current) => `${current}${current ? ' ' : ''}${chunk}`)
      })
      setReply(response.text)
      setMessage('')
      setInteractions(await getRecentInteractions())
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setSending(false)
    }
  }

  return (
    <main className="mint-dashboard">
      <section className="mint-dashboard__panel">
        <p className="mint-dashboard__eyebrow">Mint 2 migration shell</p>
        <h1>Rust backend is wired to the React renderer.</h1>
        {status ? (
          <>
            <dl className="mint-dashboard__status">
              <dt>Backend</dt>
              <dd>{status.backend}</dd>
              <dt>Provider</dt>
              <dd>{status.activeProvider}</dd>
              <dt>Config</dt>
              <dd>{status.configPath}</dd>
            </dl>
            <p>Available providers: {status.availableProviders.join(', ')}</p>
            <form className="mint-dashboard__chat" onSubmit={handleSubmit}>
              <input
                value={message}
                onChange={(event) => setMessage(event.target.value)}
                placeholder="Send a message through the Rust backend"
              />
              <button type="submit" disabled={sending}>
                {sending ? 'Sending...' : 'Send'}
              </button>
            </form>
            {reply && <p className="mint-dashboard__reply">{reply}</p>}
            {error && <p className="mint-dashboard__error">{error}</p>}
            <h2>Recent native interactions</h2>
            <ul className="mint-dashboard__interactions">
              {interactions.map((interaction) => (
                <li key={interaction.id}>
                  <strong>{interaction.userText}</strong>
                  <span>{interaction.aiText}</span>
                </li>
              ))}
            </ul>
          </>
        ) : (
          <p>{error || 'Connecting to the native backend...'}</p>
        )}
      </section>
    </main>
  )
}
