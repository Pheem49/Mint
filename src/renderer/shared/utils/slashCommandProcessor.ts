// Fallback slash-command handler for the Web/Desktop chat composer.
//
// Behavior for most commands now lives in the shared Rust engine
// (`mint_core::slash`, reached via `runSlashCommand`); `MintDashboard.handleSubmit`
// tries that first and only calls `executeSlashCommand` when the engine returns
// `not_handled`. What's left here is the handful of commands that need the
// client (a file picker, the clipboard) or aren't worth moving into the engine.

export interface SlashCommandExecutionResult {
  handled: boolean
  action?: 'open_image_picker' | 'paste_image' | 'generate_veo' | 'system_message' | 'resume_session'
  payload?: any
  systemText?: string
}

export function parseSlashCommand(input: string): { command: string; rest: string } | null {
  const trimmed = input.trim()
  if (!trimmed.startsWith('/')) return null
  const spaceIndex = trimmed.search(/\s/)
  if (spaceIndex === -1) {
    return { command: trimmed.toLowerCase(), rest: '' }
  }
  return {
    command: trimmed.slice(0, spaceIndex).toLowerCase(),
    rest: trimmed.slice(spaceIndex).trim(),
  }
}

export function executeSlashCommand(input: string): SlashCommandExecutionResult {
  const parsed = parseSlashCommand(input)
  if (!parsed) return { handled: false }

  const { command, rest } = parsed

  switch (command) {
    case '/image':
      return { handled: true, action: 'open_image_picker', payload: { pathOrPrompt: rest } }

    case '/paste':
      return { handled: true, action: 'paste_image', payload: { prompt: rest } }

    case '/resume':
      return { handled: true, action: 'resume_session', payload: { query: rest } }

    case '/veo': {
      if (!rest) {
        return {
          handled: true,
          action: 'system_message',
          systemText: '⚠️ Usage: `/veo <prompt>`',
        }
      }
      return { handled: true, action: 'generate_veo', payload: { prompt: rest } }
    }

    case '/n8n':
      return {
        handled: true,
        action: 'system_message',
        systemText:
          '🔗 **n8n** runs at http://localhost:5678 — open it in a browser tab. Wire it as an MCP server (Settings > Plugins) to drive workflows from chat.',
      }

    case '/notebook':
      return {
        handled: true,
        action: 'system_message',
        systemText:
          '📓 **SurfSense** runs at http://localhost:3929 — open it in a browser tab. Wire the `surfsense` MCP server (Settings > Plugins) to run tasks from chat.',
      }

    case '/avatar':
      return {
        handled: true,
        action: 'system_message',
        systemText:
          '🧍 **Project Avatar** — use the CLI `/avatar link` to pair a Project Avatar viewer. The separate Mint Companion app connects locally to Mint for Live2D.',
      }

    default:
      return { handled: false }
  }
}
