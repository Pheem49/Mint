import type { ArtifactFile } from '../../utils/artifacts'
import React, { useMemo, useState, useRef, useCallback, useEffect } from 'react'
import { FormattedMessage, renderCopyIcon, renderSpeakerIcon } from '../../utils/markdown'
import { fallbackNotice } from '../../utils/providers'
import McpToolResults from '../activity/McpToolResults'
import ViewedImages from '../activity/ViewedImages'
import { parseUtcDate } from '../../utils/ui'
import { parseWebSearchSources } from '../../utils/agentActivity'
import { EMPTY_AGENT_PROGRESS, hasAgentDrawerActivity, thoughtsFrom, extendedThoughtsFrom } from '../../agentProgress'
import { AgentActivityDrawer } from '../activity/AgentActivityDrawer'
import { ThinkingBlock } from './ThinkingBlock'
import SourcesBlock from './SourcesBlock'
import { TurnFileChanges, type RewindRequestPayload } from './TurnFileChanges'
import { ToolResultCards } from '../cards/ToolResultCards'
import { ArtifactCards } from './ArtifactCards'
import { AttachedDocumentCard } from './AttachedDocumentCard'
import { parseAttachedDocument } from '../../utils/attachedDocument'
import ProviderBrandIcon from '../ui/ProviderBrandIcon'

export interface ChatMessageItemProps {
  interaction: any
  copiedId: string | number | null
  onEditMessage?: (text: string) => void
  onResendFrom?: (fromId: number, newPrompt: string) => void
  speakingText: string | null
  agentActivitySnapshots: Record<string, any[]>
  openActivityIds?: Record<string, boolean>
  thinkingExpanded?: Record<string, boolean>
  onThinkingExpandedChange?: (key: string, open: boolean) => void
  handleCopyMessage: (id: string | number, text: string) => void
  speak: (text: string) => void
  toggleActivity: (id: string) => void
  chatId?: string
  workspacePath?: string
  onReferenceFile?: (filePath: string) => void
  customProviders?: Array<{ id: string; displayName?: string; logoDataUrl?: string }>
  onOpenArtifact?: (artifact: ArtifactFile) => void
  onOpenReview?: () => void
  onRewindRequest?: (payload: RewindRequestPayload) => void
  onRewindError?: (message: string) => void
}

const ChatMessageItem = React.memo(
  function ChatMessageItem(props: ChatMessageItemProps) {
    const {
      interaction,
      copiedId,
      onEditMessage,
      onResendFrom,
      speakingText,
      agentActivitySnapshots,
      openActivityIds,
      thinkingExpanded,
      onThinkingExpandedChange,
      handleCopyMessage,
      speak,
      toggleActivity,
      onOpenArtifact,
      onOpenReview,
      onRewindRequest,
      onRewindError,
    } = props

    const [isEditing, setIsEditing] = useState(false)
    const [draftText, setDraftText] = useState(interaction.userText || '')
    const inlineTextareaRef = useRef<HTMLTextAreaElement | null>(null)

    const parsedUserPrompt = useMemo(() => {
      return parseAttachedDocument(interaction.userText)
    }, [interaction.userText])

    useEffect(() => {
      setDraftText(parsedUserPrompt.userText || '')
    }, [parsedUserPrompt.userText])

    const resizeInlineTextarea = useCallback((el: HTMLTextAreaElement) => {
      el.style.height = 'auto'
      el.style.height = `${Math.min(el.scrollHeight, 400)}px`
    }, [])

    const handleStartEdit = useCallback(() => {
      setIsEditing(true)
      setDraftText(parsedUserPrompt.userText || '')
      requestAnimationFrame(() => {
        if (inlineTextareaRef.current) {
          inlineTextareaRef.current.focus()
          const len = inlineTextareaRef.current.value.length
          inlineTextareaRef.current.setSelectionRange(len, len)
          resizeInlineTextarea(inlineTextareaRef.current)
        }
      })
    }, [parsedUserPrompt.userText, resizeInlineTextarea])

    const handleCancelEdit = useCallback(() => {
      setIsEditing(false)
      setDraftText(parsedUserPrompt.userText || '')
    }, [parsedUserPrompt.userText])

    const handleSaveAndResend = useCallback(() => {
      const trimmed = draftText.trim()
      if (!trimmed) return
      setIsEditing(false)
      if (onResendFrom) {
        onResendFrom(Number(interaction.id), trimmed)
      } else if (onEditMessage) {
        onEditMessage(trimmed)
      }
    }, [draftText, interaction.id, onResendFrom, onEditMessage])

    const isSystemEvent = interaction.provider === 'system' && interaction.model === 'provider_change'

    const memoizedUserContent = useMemo(() => {
      if (!interaction.userText) return null
      const { userText, attachedDocument } = parsedUserPrompt
      return (
        <>
          {userText && <FormattedMessage text={userText} />}
          {attachedDocument && <AttachedDocumentCard document={attachedDocument} />}
        </>
      )
    }, [interaction.userText, parsedUserPrompt])

    const progress = agentActivitySnapshots[String(interaction.id)] ?? interaction.agentActivity ?? EMPTY_AGENT_PROGRESS
    const hasDrawerActivity = useMemo(() => hasAgentDrawerActivity(progress), [progress])
    const extendedThoughts = useMemo(
      () => (hasDrawerActivity ? [] : extendedThoughtsFrom(progress)),
      [hasDrawerActivity, progress],
    )
    const shortThoughts = useMemo(
      () => (hasDrawerActivity || extendedThoughts.length > 0 ? [] : thoughtsFrom(progress)),
      [hasDrawerActivity, extendedThoughts.length, progress],
    )
    const webSources = useMemo(() => parseWebSearchSources(progress), [progress])
    const memoizedAiContent = useMemo(() => {
      if (!interaction.aiText) return null
      return <FormattedMessage text={interaction.aiText} sources={webSources} />
    }, [interaction.aiText, webSources])

    if (isSystemEvent) {
      const rawText = interaction.userText || ''
      const cleanText = rawText.replace(/^Changed model to\s*/i, '').trim()
      return (
        <div key={interaction.id} className="system-event-divider">
          <div className="system-event-line" />
          <div className="system-event-pill">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z" />
            </svg>
            <span>{cleanText}</span>
          </div>
          <div className="system-event-line" />
        </div>
      )
    }

    const isUserCopied = copiedId === `user-${interaction.id}`
    const isAiCopied = copiedId === interaction.id
    const isSpeaking = speakingText === interaction.aiText

    return (
      <div key={interaction.id} style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
        {interaction.userText && (
          <div className="message user-message">
            <div className="bubble-wrapper">
              {isEditing ? (
                <div className="inline-edit-container">
                  <textarea
                    ref={inlineTextareaRef}
                    className="inline-edit-textarea"
                    value={draftText}
                    onChange={(e) => {
                      setDraftText(e.target.value)
                      resizeInlineTextarea(e.currentTarget)
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Escape') {
                        e.preventDefault()
                        handleCancelEdit()
                      } else if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
                        e.preventDefault()
                        handleSaveAndResend()
                      }
                    }}
                    rows={2}
                    placeholder="Edit message..."
                  />
                  <div className="inline-edit-footer">
                    <span className="inline-edit-hint">Esc to cancel · Ctrl+Enter to save</span>
                    <div className="inline-edit-actions">
                      <button
                        type="button"
                        className="inline-edit-btn cancel"
                        onClick={handleCancelEdit}
                      >
                        Cancel
                      </button>
                      <button
                        type="button"
                        className="inline-edit-btn submit"
                        onClick={handleSaveAndResend}
                        disabled={!draftText.trim()}
                      >
                        Save &amp; Resend
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                <>
                  <div className="message-bubble">{memoizedUserContent}</div>
                  <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span>{parseUtcDate(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                    <button
                      type="button"
                      className={`msg-action-btn copy-btn ${isUserCopied ? 'is-copied' : ''}`}
                      onClick={() => handleCopyMessage(`user-${interaction.id}`, parsedUserPrompt.userText || interaction.userText)}
                      title={isUserCopied ? 'Copied!' : 'Copy'}
                    >
                      {renderCopyIcon(isUserCopied)}
                    </button>
                    {(onResendFrom || onEditMessage) && interaction.provider !== 'system' && (
                      <button
                        type="button"
                        className="msg-action-btn edit-btn"
                        onClick={handleStartEdit}
                        title="Edit & resend"
                      >
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M12 20h9"></path>
                          <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
                        </svg>
                      </button>
                    )}
                  </div>
                </>
              )}
            </div>
          </div>
        )}
        <div className="message ai-message">
          <div className="bubble-wrapper">
            {interaction.status !== 'queued' && hasDrawerActivity && (
              <AgentActivityDrawer
                progress={progress}
                id={String(interaction.id)}
                historical={interaction.status !== 'running'}
                defaultOpen={interaction.status === 'running'}
                isOpen={openActivityIds?.[String(interaction.id)] ?? (interaction.status === 'running')}
                onToggle={toggleActivity}
                chatId={props.chatId || interaction.chatId}
                workspacePath={props.workspacePath}
                onReferenceFile={props.onReferenceFile}
                onOpenArtifact={onOpenArtifact}
              />
            )}
            {interaction.status !== 'queued' && !hasDrawerActivity && extendedThoughts.length > 0 && (
              <ThinkingBlock
                blockKey={`extended-${interaction.id}`}
                thoughts={extendedThoughts}
                isLive={interaction.status === 'running'}
                variant="extended"
                expanded={thinkingExpanded?.[`extended-${interaction.id}`]}
                onExpandedChange={onThinkingExpandedChange}
              />
            )}
            {interaction.status !== 'queued' && !hasDrawerActivity && shortThoughts.length > 0 && (
              <ThinkingBlock
                blockKey={String(interaction.id)}
                thoughts={shortThoughts}
                isLive={interaction.status === 'running'}
                expanded={thinkingExpanded?.[String(interaction.id)]}
                onExpandedChange={onThinkingExpandedChange}
              />
            )}
            {interaction.status !== 'queued' && interaction.status !== 'running' && (
              <TurnFileChanges
                progress={progress}
                interaction={interaction}
                workspacePath={props.workspacePath}
                onReferenceFile={props.onReferenceFile}
                onOpenArtifact={onOpenArtifact}
                onOpenReview={onOpenReview}
                onRewindRequest={onRewindRequest}
                onRewindError={onRewindError}
              />
            )}
            <div className="message-bubble">
              {interaction.status === 'queued' && <span>Queued for this session…</span>}
              {interaction.status === 'running' && <span>Mint is responding…</span>}
              {interaction.status === 'failed' && <span>This turn failed. Send the prompt again to retry.</span>}
              {interaction.status === 'interrupted' && <span>This turn was interrupted. Send the prompt again to retry.</span>}
              <McpToolResults progress={progress} running={interaction.status === 'running'} interrupted={interaction.status === 'interrupted'} chatId={props.chatId || interaction.chatId} mode="artifacts-only" />
              <ViewedImages progress={progress} chatId={props.chatId || interaction.chatId} onOpenArtifact={onOpenArtifact} />
              <ToolResultCards progress={progress} aiText={interaction.aiText} />
              <ArtifactCards progress={progress} aiText={interaction.aiText} workspacePath={interaction.workspace || undefined} onOpenArtifact={onOpenArtifact} />
              {memoizedAiContent}
            </div>
            {webSources.length > 0 && <SourcesBlock sources={webSources} />}
            <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              {interaction.status === 'completed' && <span className="provider-model-chip" data-provider={(interaction.provider || '').toLowerCase()} title={`${interaction.provider} • ${interaction.model}`}>
                <ProviderBrandIcon provider={interaction.provider} model={interaction.model} customProviders={props.customProviders} size={13} />
                <span className="provider-chip-name">{interaction.provider}</span>
                <span className="provider-chip-divider">/</span>
                <span className="provider-chip-model">{interaction.model}</span>
              </span>}
              {fallbackNotice(interaction) && <span className="provider-fallback-notice">{fallbackNotice(interaction)}</span>}
              <span className="message-timestamp">{parseUtcDate(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
              {interaction.status === 'completed' && <div className="message-action-buttons" style={{ display: 'flex', alignItems: 'center', gap: '4px', marginLeft: 'auto' }}>
                <button
                  type="button"
                  className={`msg-action-btn copy-btn ${isAiCopied ? 'is-copied' : ''}`}
                  onClick={() => handleCopyMessage(interaction.id, interaction.aiText)}
                  title={isAiCopied ? 'Copied!' : 'Copy message'}
                >
                  {renderCopyIcon(isAiCopied)}
                </button>
                <button
                  type="button"
                  className={`msg-action-btn tts-btn ${isSpeaking ? 'is-speaking' : ''}`}
                  onClick={() => speak(interaction.aiText)}
                  title={isSpeaking ? 'Stop reading' : 'Read aloud'}
                >
                  {renderSpeakerIcon(isSpeaking)}
                </button>
              </div>}
            </div>
          </div>
        </div>
      </div>
    )
  },
  (prevProps, nextProps) => {
    if (
      prevProps.onEditMessage !== nextProps.onEditMessage ||
      prevProps.onResendFrom !== nextProps.onResendFrom ||
      prevProps.handleCopyMessage !== nextProps.handleCopyMessage ||
      prevProps.speak !== nextProps.speak ||
      prevProps.toggleActivity !== nextProps.toggleActivity ||
      prevProps.onOpenArtifact !== nextProps.onOpenArtifact ||
      prevProps.onOpenReview !== nextProps.onOpenReview ||
      prevProps.onRewindRequest !== nextProps.onRewindRequest ||
      prevProps.onRewindError !== nextProps.onRewindError
    ) {
      return false
    }
    const p = prevProps.interaction
    const n = nextProps.interaction
    if (
      p.id !== n.id ||
      p.aiText !== n.aiText ||
      p.userText !== n.userText ||
      p.createdAt !== n.createdAt ||
      p.status !== n.status ||
      p.agentActivity !== n.agentActivity ||
      p.provider !== n.provider ||
      p.model !== n.model ||
      p.fallbackProvider !== n.fallbackProvider ||
      p.fallbackReason !== n.fallbackReason ||
      p.chatId !== n.chatId
    ) {
      return false
    }

    const prevUserCopied = prevProps.copiedId === `user-${p.id}`
    const nextUserCopied = nextProps.copiedId === `user-${n.id}`
    if (prevUserCopied !== nextUserCopied) return false

    const prevAiCopied = prevProps.copiedId === p.id
    const nextAiCopied = nextProps.copiedId === n.id
    if (prevAiCopied !== nextAiCopied) return false

    const prevSpeaking = prevProps.speakingText === p.aiText
    const nextSpeaking = nextProps.speakingText === n.aiText
    if (prevSpeaking !== nextSpeaking) return false

    const prevOpenActivity = prevProps.openActivityIds?.[String(p.id)] ?? false
    const nextOpenActivity = nextProps.openActivityIds?.[String(n.id)] ?? false
    if (prevOpenActivity !== nextOpenActivity) return false

    const prevSnap = prevProps.agentActivitySnapshots[String(p.id)]
    const nextSnap = nextProps.agentActivitySnapshots[String(n.id)]
    if (prevSnap !== nextSnap) return false

    return true
  },
)

export default ChatMessageItem
