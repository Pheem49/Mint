// The MCP server catalog is authored once in `mcp-registry.json` at the repo
// root and shared with the Rust side (`mint_core::mcp::mcp_registry`). It's a
// presets layer over the normal "Add MCP Server" form — picking an entry
// pre-fills command / args / env; nothing here is required to add a server.
import registry from '../../../../mcp-registry.json'

export interface McpRegistryArgInput {
  label: string
  placeholder?: string
}

export interface McpRegistryEnvVar {
  key: string
  label: string
  help?: string
}

export interface McpRegistryEntry {
  key: string
  name: string
  desc?: string
  icon?: string
  command: string
  args?: string[]
  argInputs?: McpRegistryArgInput[]
  requiredEnv?: McpRegistryEnvVar[]
  docs?: string
}

export const MCP_REGISTRY: McpRegistryEntry[] = registry as McpRegistryEntry[]
