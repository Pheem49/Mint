import { SLASH_COMMANDS } from '../constants/slashCommands'
import RELEASE_NOTES from '../../../../Release_Note.md?raw'

export interface SlashCommandExecutionResult {
  handled: boolean
  action?:
    | 'clear_chat'
    | 'toggle_fast'
    | 'toggle_multi_agent'
    | 'set_agent_mode'
    | 'set_provider_model'
    | 'change_workspace'
    | 'open_image_picker'
    | 'paste_image'
    | 'open_plugins'
    | 'generate_veo'
    | 'exit_session'
    | 'system_message'
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

export function executeSlashCommand(
  input: string,
  context: {
    activeProvider?: string
    activeModel?: string
    availableProviders?: string[]
    workspacePath?: string
    interactionsCount?: number
    fastMode?: boolean
    multiAgent?: boolean
  } = {}
): SlashCommandExecutionResult {
  const parsed = parseSlashCommand(input)
  if (!parsed) return { handled: false }

  const { command, rest } = parsed

  switch (command) {
    case '/help': {
      let helpText = `### ⚡ Mint Interactive Slash Commands\n\n`
      helpText += `| Command | Description | Usage |\n`
      helpText += `| :--- | :--- | :--- |\n`
      SLASH_COMMANDS.forEach((c) => {
        helpText += `| \`${c.command}\` | ${c.description} | \`${c.usage || c.command}\` |\n`
      })
      helpText += `\n*Tip: You can also use \`@\` in the chat bar to reference workspace files!*`
      return {
        handled: true,
        action: 'system_message',
        systemText: helpText,
      }
    }

    case '/fast': {
      const mode = rest.toLowerCase()
      const targetState = mode === 'on' ? true : mode === 'off' ? false : !(context.fastMode ?? false)
      return {
        handled: true,
        action: 'toggle_fast',
        payload: { enabled: targetState },
        systemText: `⚡ Fast Mode set to **${targetState ? 'ON (Thinking hidden)' : 'OFF (Thinking visible)'}**.`,
      }
    }

    case '/code': {
      return {
        handled: true,
        action: 'set_agent_mode',
        payload: { prompt: rest, agentMode: true },
      }
    }

    case '/multi-agent': {
      const mode = rest.toLowerCase()
      const targetState = mode === 'on' ? true : mode === 'off' ? false : !(context.multiAgent ?? false)
      return {
        handled: true,
        action: 'toggle_multi_agent',
        payload: { enabled: targetState },
        systemText: `🤖 Multi-Agent Collaboration set to **${targetState ? 'ON' : 'OFF'}**.`,
      }
    }

    case '/cd': {
      return {
        handled: true,
        action: 'change_workspace',
        payload: { path: rest },
      }
    }

    case '/models': {
      if (rest) {
        return {
          handled: true,
          action: 'set_provider_model',
          payload: { target: rest },
          systemText: `🔄 Switching provider/model to \`${rest}\`...`,
        }
      }
      const providers = context.availableProviders || []
      let text = `### 🤖 Available Providers & Current Model\n\n`
      text += `- **Active Provider:** \`${context.activeProvider || 'None'}\`\n`
      text += `- **Active Model:** \`${context.activeModel || 'Default'}\`\n\n`
      text += `**Available Providers:** ${providers.map((p) => `\`${p}\``).join(', ') || 'None'}\n\n`
      text += `*Use \`/models <provider_name>\` to switch active provider.*`
      return {
        handled: true,
        action: 'system_message',
        systemText: text,
      }
    }

    case '/image-provider': {
      if (rest) {
        return {
          handled: true,
          action: 'system_message',
          systemText: `🎨 Image generation provider set to \`${rest}\`.`,
        }
      }
      return {
        handled: true,
        action: 'system_message',
        systemText: `🎨 **Image Gen Providers:** Gemini Imagen 3, OpenAI DALL-E 3, Flux.1, HuggingFace.\nSwitch via Settings > Media Providers.`,
      }
    }

    case '/video-provider': {
      if (rest) {
        return {
          handled: true,
          action: 'system_message',
          systemText: `🎬 Video generation provider set to \`${rest}\`.`,
        }
      }
      return {
        handled: true,
        action: 'system_message',
        systemText: `🎬 **Video Gen Providers:** Google Veo 2.0, Runway Gen-3.\nSwitch via Settings > Media Providers.`,
      }
    }

    case '/veo': {
      if (!rest) {
        return {
          handled: true,
          action: 'system_message',
          systemText: `⚠️ Please specify a prompt for Veo video generation. Usage: \`/veo <prompt>\``,
        }
      }
      return {
        handled: true,
        action: 'generate_veo',
        payload: { prompt: rest },
      }
    }

    case '/image': {
      return {
        handled: true,
        action: 'open_image_picker',
        payload: { pathOrPrompt: rest },
      }
    }

    case '/paste': {
      return {
        handled: true,
        action: 'paste_image',
        payload: { prompt: rest },
      }
    }

    case '/mcp': {
      return {
        handled: true,
        action: 'system_message',
        systemText: `🔌 **MCP Servers Status**\n\n- Active MCP plugins configured in Settings > Plugins.\n- Open Settings > Plugins tab to add or configure Model Context Protocol tools.`,
      }
    }

    case '/plugins': {
      return {
        handled: true,
        action: 'open_plugins',
      }
    }

    case '/stats': {
      let statsText = `### 📊 Mint Session Statistics\n\n`
      statsText += `- **Active Workspace:** \`${context.workspacePath || 'Global'}\`\n`
      statsText += `- **Current Provider:** \`${context.activeProvider || 'Unknown'}\`\n`
      statsText += `- **Current Model:** \`${context.activeModel || 'Default'}\`\n`
      statsText += `- **Total Interactions:** \`${context.interactionsCount ?? 0}\`\n`
      statsText += `- **Fast Mode:** \`${context.fastMode ? 'Enabled' : 'Disabled'}\`\n`
      statsText += `- **Multi-Agent:** \`${context.multiAgent ? 'Enabled' : 'Disabled'}\`\n`
      return {
        handled: true,
        action: 'system_message',
        systemText: statsText,
      }
    }

    case '/release-notes': {
      return {
        handled: true,
        action: 'system_message',
        systemText: RELEASE_NOTES.trim(),
      }
    }

    case '/memory': {
      return {
        handled: true,
        action: 'system_message',
        systemText: `🧠 **Long-Term Memory Store**\n\nManage profile variables & interactions in Settings > Memory.`,
      }
    }

    case '/learn':
    case '/skill': {
      return {
        handled: true,
        action: 'system_message',
        systemText: `📚 **Skill & Instruction Manager**\n\nImport skills in Settings > Plugins or place markdown files in \`.agents/skills/\`.`,
      }
    }

    default:
      return { handled: false }
  }
}
