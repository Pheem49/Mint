import { type FormEvent, useEffect, useState } from 'react'
import {
  getRecentInteractions,
  getRuntimeStatus,
  applyCodeEdits,
  proposeCodeEdits,
  streamChatMessage,
  type CodeEditProposal,
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
  const [editRoot, setEditRoot] = useState('')
  const [editPath, setEditPath] = useState('')
  const [editContent, setEditContent] = useState('')
  const [editProposal, setEditProposal] = useState<CodeEditProposal | null>(null)
  const [editResult, setEditResult] = useState('')

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

  async function previewEdit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setError('')
    setEditResult('')
    try {
      setEditProposal(await proposeCodeEdits(editRoot || '.', [{ path: editPath, content: editContent }]))
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    }
  }

  async function approveEdit() {
    if (!editProposal) return
    setError('')
    try {
      const result = await applyCodeEdits(
        editRoot || '.',
        [{ path: editPath, content: editContent }],
        editProposal.approvalToken,
      )
      setEditResult(JSON.stringify(result, null, 2))
      setEditProposal(null)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
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
            <h2>Native code edit approval</h2>
            <form className="mint-dashboard__edit" onSubmit={previewEdit}>
              <input value={editRoot} onChange={(event) => setEditRoot(event.target.value)} placeholder="Workspace root, e.g. /home/me/project" />
              <input required value={editPath} onChange={(event) => setEditPath(event.target.value)} placeholder="Relative target path" />
              <textarea required value={editContent} onChange={(event) => setEditContent(event.target.value)} placeholder="Replacement file content" />
              <button type="submit">Preview diff</button>
            </form>
            {editProposal && (
              <section className="mint-dashboard__approval">
                <strong>Approval required before writing</strong>
                <pre>{editProposal.edits.map((edit) => edit.diff).join('\n\n')}</pre>
                <div>
                  <button type="button" onClick={approveEdit}>Approve and apply</button>
                  <button type="button" onClick={() => setEditProposal(null)}>Cancel</button>
                </div>
              </section>
            )}
            {editResult && <pre className="mint-dashboard__reply">{editResult}</pre>}
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
