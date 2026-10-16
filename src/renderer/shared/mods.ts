export interface ModNode {
  color?: 'default' | 'yellow' | 'cyan' | 'blue' | 'magenta' | 'red'
  direction?: 'row' | 'column'
  type: 'text' | 'stack' | 'button' | 'input' | 'table' | 'badge' | 'progress'
  text?: string
  label?: string
  action?: string
  value?: string | number
  max?: number
  children?: ModNode[]
  columns?: string[]
  rows?: (string | number | null)[][]
}
export interface ModTestReport { passed: number; failed: number; tests: { file: string; name: string; passed: boolean; error?: string | null }[] }

export interface ModRecord {
  id: string
  name: string
  hash?: string
  activeHash?: string
  instanceId?: string
  enabled: boolean
  changed?: boolean
  error?: string | null
  reloadError?: string | null
  sourceError?: string
  source?: string
  readme?: string
  manifest?: { id: string; name: string; version: string; sdkVersion: number; entry: string; permissions: string[] }
  revisions: string[]
  snapshot?: { ui: { above_composer?: ModNode; side_panel?: ModNode }; commands: { name: string; description: string }[]; logs: string[] }
}
export interface ModOperation {
  workspacePath: string
  chatId: string
  action: 'list' | 'enable' | 'disable' | 'action' | 'command' | 'validate' | 'rollback' | 'export' | 'import' | 'close' | 'run_request' | 'inspect_package' | 'hot_reload' | 'viewport' | 'usage' | 'test'
  id?: string
  scope?: 'chat' | 'workspace' | 'global'
  grantPermissions?: boolean
  expectedHash?: string
  hash?: string
  instanceId?: string
  command?: string
  value?: string | boolean
  viewport?: ModViewport
  package?: string
}
export interface ModResponse {
  session?: { hotReload: boolean; viewport: ModViewport }
  testReport?: ModTestReport
  id?: string
  expectedHash?: string | null
  mods?: ModRecord[]
  toolRequests?: { id: string; modId: string; action: string; input: unknown }[]
  package?: string
  filename?: string
}
export interface ModViewport { surface: 'terminal' | 'desktop' | 'web' | null; columns?: number | null; rows?: number | null; widthPx?: number | null; heightPx?: number | null; sidePanelOpen?: boolean }
