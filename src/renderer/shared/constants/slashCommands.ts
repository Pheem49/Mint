export interface SlashCommand {
  command: string
  description: string
  usage?: string
  category?: 'system' | 'workspace' | 'models' | 'tools'
}

export const SLASH_COMMANDS: SlashCommand[] = [
  { command: '/cd', description: 'Change active workspace directory', usage: '/cd <path>', category: 'workspace' },
  { command: '/code', description: 'Run in code-agent mode', usage: '/code <task>', category: 'tools' },
  { command: '/fast', description: 'Toggle fast mode (hide thinking traces)', usage: '/fast [on|off]', category: 'system' },
  { command: '/help', description: 'Show interactive help menu', usage: '/help', category: 'system' },
  { command: '/image', description: 'Attach image from disk', usage: '/image [path] [prompt]', category: 'tools' },
  { command: '/image-provider', description: 'List or switch default image gen provider', usage: '/image-provider [name]', category: 'models' },
  { command: '/learn', description: 'Import persistent skill/instruction file', usage: '/learn [path]', category: 'tools' },
  { command: '/mcp', description: 'List configured MCP servers & status', usage: '/mcp [list|allow]', category: 'tools' },
  { command: '/memory', description: 'Manage long-term memory store', usage: '/memory [list|clear|get|set]', category: 'tools' },
  { command: '/models', description: 'List AI providers or switch active provider/model', usage: '/models [provider/model]', category: 'models' },
  { command: '/multi-agent', description: 'Toggle Multi-Agent Collaboration system', usage: '/multi-agent [on|off]', category: 'system' },
  { command: '/paste', description: 'Attach image from clipboard', usage: '/paste [prompt]', category: 'tools' },
  { command: '/plugins', description: 'List or generate plugins/skills', usage: '/plugins [name]', category: 'tools' },
  { command: '/release-notes', description: 'Show release notes for the current version', usage: '/release-notes', category: 'system' },
  { command: '/skill add', description: 'Add or install global skill file or folder', usage: '/skill add <path>', category: 'tools' },
  { command: '/stats', description: 'Show session statistics', usage: '/stats', category: 'system' },
  { command: '/veo', description: 'Generate video using Google Veo', usage: '/veo <prompt>', category: 'tools' },
  { command: '/video-provider', description: 'List or switch default video gen provider', usage: '/video-provider [name]', category: 'models' },
]

