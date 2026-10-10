import { memo } from 'react'
import { renderFormattedMessage } from '../utils/markdown'
import type { WebSearchSource } from '../utils/agentActivity'

// Keep the Markdown subtree stable when copy, speech, timers, or activity change.
export const FormattedMessage = memo(function FormattedMessage({ text, sources }: {
  text: string
  sources?: WebSearchSource[]
}) {
  return renderFormattedMessage(text, sources)
})
