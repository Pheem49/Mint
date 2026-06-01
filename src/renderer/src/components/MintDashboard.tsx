import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from 'react'
import {
  clearChatHistory,
  getRecentInteractions,
  getRuntimeStatus,
  listSavedPictures,
  streamChatMessage,
  type ChatResponse,
  type InteractionMemory,
  type PictureEntry,
  type RuntimeStatus,
} from '../tauri'

type DashboardView = 'chat' | 'pictures' | 'model'

function errorMessage(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason)
}

function readImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error ?? new Error('Unable to read image'))
    reader.readAsDataURL(file)
  })
}

function badge(provider: string, model: string) {
  return [provider, model].filter(Boolean).join(' / ')
}

export default function MintDashboard() {
  const [view, setView] = useState<DashboardView>('chat')
  const [status, setStatus] = useState<RuntimeStatus | null>(null)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const [interactions, setInteractions] = useState<InteractionMemory[]>([])
  const [pictures, setPictures] = useState<PictureEntry[]>([])
  const [sending, setSending] = useState(false)
  const [streamedReply, setStreamedReply] = useState('')
  const [streamedResponse, setStreamedResponse] = useState<ChatResponse | null>(null)
  const [imageDataUri, setImageDataUri] = useState<string | null>(null)
  const [imageName, setImageName] = useState('')
  const [modelVisible, setModelVisible] = useState(
    () => window.localStorage.getItem('mint:model-visible') === 'true',
  )
  const chatEnd = useRef<HTMLDivElement | null>(null)

  async function refreshHistory() {
    const history = await getRecentInteractions()
    setInteractions(history.reverse())
  }

  async function refreshPictures() {
    setPictures(await listSavedPictures())
  }

  useEffect(() => {
    getRuntimeStatus().then(setStatus).catch((reason: unknown) => setError(errorMessage(reason)))
    refreshHistory().catch((reason: unknown) => setError(errorMessage(reason)))
    window.api.onSpotlightToChat((query) => {
      setView('chat')
      setMessage(query)
    })
  }, [])

  useEffect(() => {
    if (view === 'pictures') {
      refreshPictures().catch((reason: unknown) => setError(errorMessage(reason)))
    }
  }, [view])

  useEffect(() => {
    chatEnd.current?.scrollIntoView({ behavior: 'smooth' })
  }, [interactions, streamedReply])

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const trimmed = message.trim()
    if (!trimmed || sending) return
    setSending(true)
    setError('')
    setStreamedReply('')
    setStreamedResponse(null)
    try {
      const response = await streamChatMessage(
        trimmed,
        (chunk) => setStreamedReply((current) => `${current}${chunk}`),
        imageDataUri,
      )
      setStreamedResponse(response)
      setMessage('')
      setImageDataUri(null)
      setImageName('')
      await refreshHistory()
      await refreshPictures()
      setStreamedReply('')
      setStreamedResponse(null)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSending(false)
    }
  }

  async function selectImage(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    if (!file) return
    try {
      setImageDataUri(await readImage(file))
      setImageName(file.name)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      event.target.value = ''
    }
  }

  async function clearHistory(action: 'New chat' | 'Clear history') {
    if (!window.confirm(`${action} will clear the current conversation history. Continue?`)) return
    try {
      await clearChatHistory()
      setInteractions([])
      setStreamedReply('')
      setStreamedResponse(null)
      setMessage('')
      setImageDataUri(null)
      setImageName('')
    } catch (reason) {
      setError(errorMessage(reason))
    }
  }

  function toggleModel() {
    const next = !modelVisible
    window.localStorage.setItem('mint:model-visible', String(next))
    setModelVisible(next)
  }

  return (
    <main className="mint-app">
      <aside className="mint-sidebar">
        <div className="mint-brand">
          <strong>Mint</strong>
          <span>{status?.backend ?? 'connecting'}</span>
        </div>
        <nav aria-label="Workspace">
          <button className={view === 'chat' ? 'active' : ''} onClick={() => setView('chat')}>Chat</button>
          <button className={view === 'pictures' ? 'active' : ''} onClick={() => setView('pictures')}>Pictures</button>
          <button onClick={() => window.api.openSettings()}>Settings</button>
          <button className={view === 'model' ? 'active' : ''} onClick={() => setView('model')}>Model</button>
        </nav>
        <div className="mint-sidebar__footer">
          <span>{status?.activeProvider ?? 'provider'}</span>
          <button onClick={() => clearHistory('New chat')}>New Chat</button>
        </div>
      </aside>

      <section className="mint-workspace">
        <header className="mint-toolbar">
          <div>
            <strong>{view === 'chat' ? 'Conversation' : view === 'pictures' ? 'Pictures' : 'Assistant Model'}</strong>
            <span>{status ? `${status.activeProvider} backend` : 'Connecting to native backend'}</span>
          </div>
          <div className="mint-toolbar__actions">
            {view === 'chat' && <button onClick={() => clearHistory('Clear history')}>Clear</button>}
            <button onClick={() => window.api.minimizeWindow()}>Minimize</button>
            <button onClick={() => window.api.closeWindow()}>Close</button>
          </div>
        </header>

        {error && <p className="mint-error">{error}</p>}

        {view === 'chat' && (
          <section className="mint-chat">
            <div className="mint-chat__messages">
              {interactions.length === 0 && !sending && (
                <div className="mint-empty">
                  <strong>Start a conversation</strong>
                  <span>Messages are stored locally and restored when Mint opens again.</span>
                </div>
              )}
              {interactions.map((interaction) => (
                <article className="mint-message-group" key={interaction.id}>
                  <div className="mint-message mint-message--user">{interaction.userText}</div>
                  <div className="mint-message mint-message--assistant">
                    <span>{interaction.aiText}</span>
                    {badge(interaction.provider, interaction.model) && (
                      <small>{badge(interaction.provider, interaction.model)}</small>
                    )}
                  </div>
                </article>
              ))}
              {sending && (
                <article className="mint-message-group">
                  <div className="mint-message mint-message--user">
                    {imageDataUri ? `${message} [Image #1]` : message}
                  </div>
                  <div className="mint-message mint-message--assistant">
                    <span>{streamedReply || 'Thinking...'}</span>
                    {streamedResponse && <small>{badge(streamedResponse.provider, streamedResponse.model)}</small>}
                  </div>
                </article>
              )}
              <div ref={chatEnd} />
            </div>
            <form className="mint-composer" onSubmit={handleSubmit}>
              {imageDataUri && (
                <div className="mint-attachment">
                  <img src={imageDataUri} alt="" />
                  <span>{imageName}</span>
                  <button type="button" onClick={() => { setImageDataUri(null); setImageName('') }}>Remove</button>
                </div>
              )}
              <textarea
                value={message}
                onChange={(event) => setMessage(event.target.value)}
                placeholder="Ask Mint anything..."
                rows={3}
              />
              <div className="mint-composer__actions">
                <label>
                  Attach image
                  <input type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={selectImage} />
                </label>
                <span>{status?.activeProvider ?? ''}</span>
                <button type="submit" disabled={sending || !message.trim()}>
                  {sending ? 'Sending...' : 'Send'}
                </button>
              </div>
            </form>
          </section>
        )}

        {view === 'pictures' && (
          <section className="mint-pictures">
            {pictures.length === 0 ? (
              <div className="mint-empty">
                <strong>No saved pictures yet</strong>
                <span>Images appear here after a message with an attachment is sent successfully.</span>
              </div>
            ) : (
              <div className="mint-pictures__grid">
                {pictures.map((picture) => (
                  <a className="mint-picture" href={picture.url} target="_blank" rel="noreferrer" key={picture.id}>
                    <img src={picture.url} alt={picture.message || picture.filename} />
                    <strong>{picture.message || picture.filename}</strong>
                    <span>{new Date(picture.createdAt).toLocaleString()}</span>
                  </a>
                ))}
              </div>
            )}
          </section>
        )}

        {view === 'model' && (
          <section className="mint-model">
            <div className={`mint-model__stage ${modelVisible ? 'visible' : ''}`}>
              <strong>{modelVisible ? 'Model panel enabled' : 'Model panel hidden'}</strong>
              <span>Live2D rendering and expressions are scheduled for the next phase.</span>
            </div>
            <button onClick={toggleModel}>{modelVisible ? 'Hide model panel' : 'Show model panel'}</button>
          </section>
        )}
      </section>
    </main>
  )
}
