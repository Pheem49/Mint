import React, { useMemo } from 'react'
import { renderFormattedMessage, renderCopyIcon, renderSpeakerIcon } from '../utils/markdown'
import { fallbackNotice } from '../utils/providers'
import { ThinkingBlock } from './ThinkingBlock'
import { thoughtsFrom, hasAgentToolActivity } from '../agentProgress'
import WeatherCard from './WeatherCard'
import StockCard from './StockCard'
import CalculationCard from './CalculationCard'
import ImageSearchCard from './ImageSearchCard'

export interface ChatMessageItemProps {
  interaction: any
  copiedId: string | number | null
  speakingText: string | null
  agentActivitySnapshots: Record<string, any[]>
  thinkingExpanded: Record<string, boolean>
  openActivityIds?: Record<string, boolean>
  openReviewIds?: Record<string, boolean>
  openFileDiffs?: Record<string, boolean>
  onThinkingExpandedChange: (key: string, expanded: boolean) => void
  handleCopyMessage: (id: string | number, text: string) => void
  speak: (text: string) => void
  renderCompletedActivity: (interaction: any) => React.ReactNode
  renderFileChanges: (interaction: any) => React.ReactNode
  renderWebSearchSources: (interaction: any) => React.ReactNode
}

const ChatMessageItem = React.memo(
  function ChatMessageItem(props: ChatMessageItemProps) {
    const {
      interaction,
      copiedId,
      speakingText,
      agentActivitySnapshots,
      thinkingExpanded,
      openActivityIds,
      openReviewIds,
      openFileDiffs,
      onThinkingExpandedChange,
      handleCopyMessage,
      speak,
      renderCompletedActivity,
      renderFileChanges,
      renderWebSearchSources,
    } = props

    const isSystemEvent = interaction.provider === 'system' && interaction.model === 'provider_change'

    const memoizedUserContent = useMemo(() => {
      if (!interaction.userText) return null
      return renderFormattedMessage(interaction.userText)
    }, [interaction.userText])

    const memoizedAiContent = useMemo(() => {
      if (!interaction.aiText) return null
      return renderFormattedMessage(interaction.aiText)
    }, [interaction.aiText])

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

    const progress = agentActivitySnapshots[String(interaction.id)] ?? interaction.agentActivity ?? []
    const thoughts = thoughtsFrom(progress)
    const isUserCopied = copiedId === `user-${interaction.id}`
    const isAiCopied = copiedId === interaction.id
    const isSpeaking = speakingText === interaction.aiText
    const isExpanded = thinkingExpanded[String(interaction.id)] ?? false

    const fallbackWeatherData = useMemo(() => {
      if (interaction.aiText && (interaction.aiText.includes('```weather_json') || interaction.aiText.includes('```weather-json'))) {
        return null
      }
      for (const event of progress || []) {
        if (event.type === 'ToolEnd' && (event.data?.action === 'weather' || event.data?.name === 'weather')) {
          const res = event.data.result || event.data.output || ''
          const match = typeof res === 'string' && res.match(/```weather_json\s*([\s\S]*?)\s*```/)
          if (match && match[1]) {
            try {
              return JSON.parse(match[1])
            } catch {}
          }
        }
      }
      return null
    }, [interaction.aiText, progress])

    const fallbackStockData = useMemo(() => {
      if (interaction.aiText && (interaction.aiText.includes('```stock_json') || interaction.aiText.includes('```stock-json'))) {
        return null
      }
      for (const event of progress || []) {
        if (event.type === 'ToolEnd' && (event.data?.action === 'stock' || event.data?.name === 'stock')) {
          const res = event.data.result || event.data.output || ''
          const match = typeof res === 'string' && res.match(/```stock_json\s*([\s\S]*?)\s*```/)
          if (match && match[1]) {
            try {
              return JSON.parse(match[1])
            } catch {}
          }
        }
      }
      return null
    }, [interaction.aiText, progress])

    const fallbackCalcData = useMemo(() => {
      if (interaction.aiText && (interaction.aiText.includes('```calculation_json') || interaction.aiText.includes('```calculation-json'))) {
        return null
      }
      for (const event of progress || []) {
        if (event.type === 'ToolEnd' && (event.data?.action === 'calculation' || event.data?.name === 'calculation')) {
          const res = event.data.result || event.data.output || ''
          const match = typeof res === 'string' && res.match(/```calculation_json\s*([\s\S]*?)\s*```/)
          if (match && match[1]) {
            try {
              return JSON.parse(match[1])
            } catch {}
          }
        }
      }
      return null
    }, [interaction.aiText, progress])

    const fallbackImageSearchData = useMemo(() => {
      if (interaction.aiText && (interaction.aiText.includes('```image_search_json') || interaction.aiText.includes('```image-search-json'))) {
        return null
      }
      for (const event of progress || []) {
        if (event.type === 'ToolEnd' && (event.data?.action === 'image_search' || event.data?.name === 'image_search')) {
          const res = event.data.result || event.data.output || ''
          const match = typeof res === 'string' && res.match(/```image_search_json\s*([\s\S]*?)\s*```/)
          if (match && match[1]) {
            try {
              return JSON.parse(match[1])
            } catch {}
          }
        }
      }
      return null
    }, [interaction.aiText, progress])

    return (
      <div key={interaction.id} style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
        {interaction.userText && (
          <div className="message user-message">
            <div className="bubble-wrapper">
              <div className="message-bubble">{memoizedUserContent}</div>
              <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                <button
                  type="button"
                  className={`msg-action-btn copy-btn ${isUserCopied ? 'is-copied' : ''}`}
                  onClick={() => handleCopyMessage(`user-${interaction.id}`, interaction.userText)}
                  title={isUserCopied ? 'คัดลอกแล้ว (Copied!)' : 'คัดลอกข้อความ (Copy)'}
                >
                  {renderCopyIcon(isUserCopied)}
                </button>
              </div>
            </div>
          </div>
        )}
        <div className="message ai-message">
          <div className="bubble-wrapper">
            {renderCompletedActivity(interaction)}
            {renderFileChanges(interaction)}
            <ThinkingBlock
              blockKey={String(interaction.id)}
              thoughts={thoughts}
              expanded={isExpanded}
              onExpandedChange={onThinkingExpandedChange}
              showEmptyHint={hasAgentToolActivity(progress) && thoughts.length === 0}
            />
            <div className="message-bubble">
              {fallbackWeatherData && <WeatherCard data={fallbackWeatherData} />}
              {fallbackStockData && <StockCard data={fallbackStockData} />}
              {fallbackCalcData && <CalculationCard data={fallbackCalcData} />}
              {fallbackImageSearchData && <ImageSearchCard data={fallbackImageSearchData} />}
              {memoizedAiContent}
            </div>
            {renderWebSearchSources(interaction)}
            <div className="message-time" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <button className="provider-badge">{interaction.provider} • {interaction.model}</button>
              {fallbackNotice(interaction) && <span className="provider-fallback-notice">{fallbackNotice(interaction)}</span>}
              <span>{new Date(interaction.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
              <div className="message-action-buttons" style={{ display: 'flex', alignItems: 'center', gap: '4px', marginLeft: 'auto' }}>
                <button
                  type="button"
                  className={`msg-action-btn copy-btn ${isAiCopied ? 'is-copied' : ''}`}
                  onClick={() => handleCopyMessage(interaction.id, interaction.aiText)}
                  title={isAiCopied ? 'คัดลอกแล้ว (Copied!)' : 'คัดลอกข้อความ (Copy message)'}
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
              </div>
            </div>
          </div>
        </div>
      </div>
    )
  },
  (prevProps, nextProps) => {
    const p = prevProps.interaction
    const n = nextProps.interaction
    if (p.id !== n.id || p.aiText !== n.aiText || p.userText !== n.userText || p.createdAt !== n.createdAt) {
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

    const prevExpanded = prevProps.thinkingExpanded[String(p.id)] ?? false
    const nextExpanded = nextProps.thinkingExpanded[String(n.id)] ?? false
    if (prevExpanded !== nextExpanded) return false

    const prevOpenActivity = prevProps.openActivityIds?.[String(p.id)] ?? false
    const nextOpenActivity = nextProps.openActivityIds?.[String(n.id)] ?? false
    if (prevOpenActivity !== nextOpenActivity) return false

    const prevOpenReview = prevProps.openReviewIds?.[String(p.id)] ?? false
    const nextOpenReview = nextProps.openReviewIds?.[String(n.id)] ?? false
    if (prevOpenReview !== nextOpenReview) return false

    const prevOpenFileDiffs = prevProps.openFileDiffs
    const nextOpenFileDiffs = nextProps.openFileDiffs
    if (prevOpenFileDiffs !== nextOpenFileDiffs) return false

    const prevSnap = prevProps.agentActivitySnapshots[String(p.id)]
    const nextSnap = nextProps.agentActivitySnapshots[String(n.id)]
    if (prevSnap !== nextSnap) return false

    return true
  }
)

export default ChatMessageItem
