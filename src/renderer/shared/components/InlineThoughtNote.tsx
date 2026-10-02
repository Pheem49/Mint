/**
 * shared/components/InlineThoughtNote.tsx
 * Renders a short, inline agent thought note in the activity timeline.
 * These are brief notes like "Switched provider", "Using JSON mode fallback",
 * or the agent's short reasoning about what to do next — distinct from
 * extended model thinking (which goes in ThinkingBlock).
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import { renderFormattedMessage } from '../utils/markdown'
import { cleanIntermediateThought } from '../utils/agentActivity'

interface Props {
  thought: string
}

export function InlineThoughtNote({ thought }: Props) {
  const cleaned = cleanIntermediateThought(thought)
  if (!cleaned) return null
  return (
    <div className="inline-thought-note">
      <div className="inline-thought-note-text">
        {renderFormattedMessage(cleaned) ?? cleaned}
      </div>
    </div>
  )
}

