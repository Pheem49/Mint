// The slash-command catalog is authored once in `slash-commands.json` at the repo
// root and shared with the Rust CLI (`crates/mint-cli/src/interactive/commands.rs`).
// Each entry's `surfaces` array decides which UIs list it — here we keep only the
// commands surfaced to `web`/`desktop`, dropping CLI-only ones like `/bg`, `/jobs`,
// `/shells`, `/exit`, `/plan`.
import manifest from '../../../../slash-commands.json'

export interface SlashCommand {
  command: string
  description: string
  usage?: string
  category?: 'system' | 'workspace' | 'models' | 'tools'
}

interface ManifestEntry {
  token: string
  usage?: string
  description: string
  surfaces: string[]
  category?: string
}

export const SLASH_COMMANDS: SlashCommand[] = (manifest as ManifestEntry[])
  .filter((entry) => entry.surfaces.some((s) => s === 'web' || s === 'desktop'))
  .map((entry) => ({
    command: entry.token,
    description: entry.description,
    usage: entry.usage || undefined,
    category: entry.category as SlashCommand['category'],
  }))
