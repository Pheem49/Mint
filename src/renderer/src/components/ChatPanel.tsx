import { convertFileSrc } from '@tauri-apps/api/core'
import type { ChangeEvent, FormEvent, KeyboardEvent, RefObject } from 'react'
import type { ChatResponse, PictureEntry, RuntimeStatus } from '../tauri'
import type { DashboardView } from './DashboardSidebar'

interface ApprovalDetails {
  title: string
  body: string
  reason?: string
  isDangerous: boolean
}

function badge(provider: string, model: string) {
  return [provider, model].filter(Boolean).join(' / ')
}

function renderApprovalDetails(approval: any): ApprovalDetails {
  if (!approval) return { title: 'Action Pending Approval', body: 'No action details available.', isDangerous: false }
  if (approval.WriteFile) return { title: 'Write File', body: `Path: ${approval.WriteFile.path}`, reason: approval.WriteFile.diff ? `Diff:\n${approval.WriteFile.diff}` : 'Writing new file content.', isDangerous: false }
  if (approval.ApplyPatch) return { title: 'Apply Patch', body: `Path: ${approval.ApplyPatch.path}`, reason: approval.ApplyPatch.diff ? `Diff:\n${approval.ApplyPatch.diff}` : 'Applying code patch.', isDangerous: false }
  if (approval.RunShell) return { title: 'Run Shell Command', body: approval.RunShell.command, reason: 'Executing shell commands can modify your system.', isDangerous: true }
  if (approval.NoteWrite) return { title: 'Write Note', body: `Path: ${approval.NoteWrite.path}`, reason: 'Creating or updating workspace notes.', isDangerous: false }
  if (approval.RunPlugin) return { title: `Run Plugin: ${approval.RunPlugin.name}`, body: approval.RunPlugin.instruction, reason: 'Executing a native plugin action.', isDangerous: false }
  if (approval.McpTool) {
    const { server, tool, arguments: args } = approval.McpTool
    return { title: `Run MCP Tool: ${server}/${tool}`, body: typeof args === 'string' ? args : JSON.stringify(args, null, 2), reason: 'Running external MCP tool.', isDangerous: false }
  }
  return { title: 'Unknown Action', body: JSON.stringify(approval, null, 2), reason: 'Requires approval to proceed.', isDangerous: false }
}

interface ChatPanelProps {
  interactions: any[]
  sending: boolean
  sendingMessage: string
  sendingHasImage: boolean
  streamedReply: string
  streamedResponse: ChatResponse | null
  message: string
  imageDataUri: string | null
  imageName: string
  pendingApproval: any | null
  smartContext: boolean
  agentMode: boolean
  status: RuntimeStatus | null
  chatEnd: RefObject<HTMLDivElement | null>
  welcomeInteraction: any
  onSubmit: (event: FormEvent<HTMLFormElement>) => void
  onSelectImage: (event: ChangeEvent<HTMLInputElement>) => void
  onSetMessage: (message: string) => void
  onRemoveImage: () => void
  onSetSmartContext: (enabled: boolean) => void
  onSetAgentMode: (enabled: boolean) => void
  onSetProvider: (provider: string) => void
  onApproval: (approved: boolean) => void
}

export default function ChatPanel({
  interactions,
  sending,
  sendingMessage,
  sendingHasImage,
  streamedReply,
  streamedResponse,
  message,
  imageDataUri,
  imageName,
  pendingApproval,
  smartContext,
  agentMode,
  status,
  chatEnd,
  welcomeInteraction,
  onSubmit,
  onSelectImage,
  onSetMessage,
  onRemoveImage,
  onSetSmartContext,
  onSetAgentMode,
  onSetProvider,
  onApproval,
}: ChatPanelProps) {
  const submitOnEnter = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return
    event.preventDefault()
    event.currentTarget.form?.requestSubmit()
  }

  return (
    <section className="conversation-panel">
      <div className="chat-container">
        {interactions.length === 0 && !sending && (
          <div className="message ai-message" style={{ marginBottom: '16px' }}>
            <div className="bubble-wrapper">
              <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>{welcomeInteraction.aiText}</div>
              <div className="message-time">
                <button className="provider-badge">{welcomeInteraction.provider} • {welcomeInteraction.model}</button>
                <span>14:44</span>
              </div>
            </div>
          </div>
        )}

        {interactions.map((interaction) => (
          <div key={interaction.id} style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
            {interaction.isSystemEvent ? (
              <div className="system-event" style={{ background: 'rgba(16, 185, 129, 0.06)', border: '1px solid rgba(16, 185, 129, 0.2)', borderRadius: '8px', padding: '10px 14px', color: '#a7f3d0', fontSize: '0.82rem', lineHeight: '1.45', alignSelf: 'stretch' }}>
                {interaction.userText}
              </div>
            ) : interaction.userText && (
              <div className="message user-message">
                <div className="bubble-wrapper">
                  <div className="message-bubble">{interaction.userText}</div>
                  <div className="message-time"><span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span></div>
                </div>
              </div>
            )}
            <div className="message ai-message">
              <div className="bubble-wrapper">
                <div className="message-bubble" style={{ whiteSpace: 'pre-wrap' }}>{interaction.aiText}</div>
                <div className="message-time">
                  <button className="provider-badge">{interaction.provider} • {interaction.model}</button>
                  <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                </div>
              </div>
            </div>
          </div>
        ))}

        {sending && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
            <div className="message user-message"><div className="bubble-wrapper"><div className="message-bubble">{sendingHasImage ? `${sendingMessage} [Image #1]` : sendingMessage}</div></div></div>
            <div className="message ai-message thinking-message">
              <div className="bubble-wrapper">
                <div className="message-bubble"><span>{streamedReply || 'Thinking...'}</span></div>
                {streamedResponse && <div className="message-time"><button className="provider-badge">{badge(streamedResponse.provider, streamedResponse.model)}</button></div>}
              </div>
            </div>
          </div>
        )}

        {pendingApproval && (() => {
          const details = renderApprovalDetails(pendingApproval.approval)
          return (
            <div className="message ai-message" style={{ width: '100%' }}>
              <div className="bubble-wrapper" style={{ width: '100%' }}>
                <div className="action-card approval-card" data-tier={details.isDangerous ? 'dangerous' : undefined} style={{ width: '100%' }}>
                  <div className="approval-card-content">
                    <div className="approval-card-title">{details.title}</div>
                    <div className="approval-card-body" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>{details.body}</div>
                    {details.reason && <div className="approval-card-reason" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>{details.reason}</div>}
                  </div>
                  <div className="approval-card-actions">
                    <button type="button" className="approval-btn approval-btn-approve" onClick={() => onApproval(true)}>Approve</button>
                    <button type="button" className="approval-btn approval-btn-cancel" onClick={() => onApproval(false)}>Cancel</button>
                  </div>
                </div>
              </div>
            </div>
          )
        })()}
        <div ref={chatEnd} />
      </div>

      <div className="input-area">
        <div className="smart-context-bar">
          <div className="smart-context-label" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <label className="toggle-switch">
              <input type="checkbox" checked={smartContext} onChange={(event) => onSetSmartContext(event.target.checked)} />
              <span className="slider round" />
            </label>
            <span>Smart Context (Auto-Screen)</span>
          </div>
          <div className="smart-context-label" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <label className="toggle-switch">
              <input type="checkbox" checked={agentMode} onChange={(event) => onSetAgentMode(event.target.checked)} />
              <span className="slider round" />
            </label>
            <span>Agent Mode</span>
          </div>
        </div>

        <form id="chat-form" onSubmit={onSubmit}>
          {imageDataUri && (
            <div className="mint-attachment" style={{ gridColumn: '1 / -1', gridRow: '1', marginBottom: '8px', display: 'flex', alignItems: 'center', gap: '8px' }}>
              <img src={imageDataUri} alt="" style={{ width: '32px', height: '32px', borderRadius: '4px' }} />
              <span style={{ fontSize: '0.76rem', color: 'var(--text-soft)' }}>{imageName}</span>
              <button type="button" onClick={onRemoveImage} style={{ background: 'transparent', border: 0, color: '#ef4444', cursor: 'pointer' }}>✕</button>
            </div>
          )}
          <textarea id="chat-input" value={message} onChange={(event) => onSetMessage(event.target.value)} onKeyDown={submitOnEnter} placeholder="Ask anything, @ to mention, / for actions" rows={1} />
          <button id="vision-btn" type="button" onClick={() => document.getElementById('vision-file-input')?.click()}>👁</button>
          <input id="vision-file-input" type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={onSelectImage} style={{ display: 'none' }} />
          <select className="chat-provider-select" value={status?.activeProvider ?? ''} onChange={(event) => onSetProvider(event.target.value)}>
            {status?.availableProviders.map((provider) => <option key={provider} value={provider}>{provider}</option>)}
          </select>
          <button id="mic-btn" type="button" onClick={() => alert("Voice transcription coming soon!")}>🎙</button>
          <button id="send-btn" type="submit" disabled={sending || !message.trim()}>➤</button>
        </form>
      </div>
    </section>
  )
}

interface PicturesLibraryProps {
  view: DashboardView
  pictures: PictureEntry[]
  onSetView: (view: DashboardView) => void
}

export function PicturesLibrary({ view, pictures, onSetView }: PicturesLibraryProps) {
  if (view !== 'pictures') return null

  return (
    <section className="pictures-library">
      <header className="pictures-header">
        <div><span className="pictures-kicker">Gallery</span><h2>Saved Pictures</h2></div>
        <button className="pictures-close-btn" onClick={() => onSetView('chat')}>Close Gallery</button>
      </header>
      {pictures.length === 0 ? (
        <div className="pictures-empty">
          <div className="pictures-empty-icon" style={{ display: 'flex', justifyContent: 'center', marginBottom: '12px', opacity: 0.3 }}>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline>
            </svg>
          </div>
          <p>No saved pictures yet</p>
          <span>Images appear here after a message with an attachment is sent successfully.</span>
        </div>
      ) : (
        <div className="pictures-grid">
          {pictures.map((picture) => (
            <a className="picture-card" href={picture.url} target="_blank" rel="noreferrer" key={picture.id} onClick={(event) => { event.preventDefault(); window.settingsApi?.openExternal(picture.url || '') }}>
              <img src={convertFileSrc(picture.path)} alt={picture.message || picture.filename} loading="lazy" decoding="async" />
              <div className="picture-card-meta">{picture.message || picture.filename}</div>
            </a>
          ))}
        </div>
      )}
    </section>
  )
}
