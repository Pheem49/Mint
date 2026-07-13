/**
 * shared/types.ts
 * Shared TypeScript interfaces and types for both Desktop and Web renderers.
 * Source of truth: Import from here, not from local declarations in tauri.ts.
 */

export interface RuntimeStatus {
  backend: string
  configPath: string
  activeProvider: string
  availableProviders: string[]
  integrations: Record<string, unknown>
  localIp?: string
}

export interface ChatResponse {
  provider: string
  model: string
  text: string
  fallbackProvider?: string | null
}

export interface TtsUrl {
  shortText: string
  url: string
}

export interface DocumentAttachment {
  filename: string
  dataUri: string
}

export type AgentProgress =
  | { type: 'Thinking'; data: { elapsed_secs: number; agent_name?: string; model_name?: string } }
  | { type: 'Thought'; data: { thought: string } }
  | { type: 'ToolStart'; data: { action: string; input: Record<string, unknown> } }
  | { type: 'ToolEnd'; data: { action: string; input: Record<string, unknown>; result: string } }

export interface InteractionMemory {
  id: number
  chatId: string
  userText: string
  aiText: string
  provider: string
  model: string
  fallbackProvider?: string | null
  createdAt: string
  agentActivity?: AgentProgress[] | null
}

export interface ChatSession {
  id: string
  title: string
  kind: string
  createdAt: string
  updatedAt: string
}

export interface PictureEntry {
  id: string
  filename: string
  path: string
  mimeType: string
  createdAt: string
  source: string
  message: string
  thumbnailPath?: string
  url?: string
  thumbnailUrl?: string
}

export interface ImageGenRequest {
  prompt: string
  negativePrompt?: string
  aspectRatio?: '1:1' | '16:9' | '9:16' | '4:3'
  numImages?: number
  model?: string
  provider?: string
}

export interface ImageGenProviders {
  active: string
  available: string[]
}

export interface ImageGenResponse {
  images: PictureEntry[]
  model: string
  provider: string
  prompt: string
  description?: string | null
}

export interface CodeEdit {
  path: string
  content: string
}

export interface CodeEditProposal {
  approvalRequired: boolean
  approvalToken: string
  edits: Array<{ path: string; existed: boolean; diff: string }>
}

export interface WorkspaceTreeEntry {
  name: string
  path: string
  kind: 'file' | 'directory'
  children: WorkspaceTreeEntry[]
}

export interface DetectedTools {
  docker: boolean
  git: boolean
  gh: boolean
  node: boolean
}

export interface LearnedSkill {
  id: number
  name: string
  sourcePath: string
  content: string
  updatedAt: string
  location?: string
}

export interface DiffHunk {
  oldText: string
  newText: string
}

export interface FileChange {
  path: string
  created: boolean
  additions: number
  deletions: number
  hunks: DiffHunk[]
}

