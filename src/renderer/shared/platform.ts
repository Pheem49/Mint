/**
 * shared/platform.ts
 * Unified platform API interface for both Desktop and Web renderers.
 * Source of truth: Both tauri.ts files implement this API surface.
 */
import type {
  RuntimeStatus,
  ChatResponse,
  TtsUrl,
  InteractionMemory,
  ChatSession,
  PictureEntry,
  ImageGenRequest,
  ImageGenProviders,
  ImageGenResponse,
  WorkspaceTreeEntry,
  CodeEdit,
  CodeEditProposal,
  DetectedTools,
  LearnedSkill,
  SubagentDefinition,
  SubagentDraft,
  AgentProgress,
  AuthUser,
} from './types'

export function getLocalApiBase(): string {
  if (typeof window !== 'undefined') {
    const host = window.location.hostname || 'localhost'
    return `http://${host}:3000/api`
  }
  return 'http://localhost:3000/api'
}

// --- Shared slash-command engine (mint_core::slash) -------------------------
// Mirrors the serde-serialized Rust enums. `kind` is the internal tag.

export type SlashNavTarget = 'cron' | 'linked_folders' | 'skills' | 'plugins' | 'mcp' | 'veo'

export interface SlashChoice {
  label: string
  value: string
}

export type SlashEffect =
  | { kind: 'config_changed' }
  | { kind: 'provider_changed'; display: string }
  | { kind: 'workspace_changed'; path: string }
  | { kind: 'history_cleared' }
  | { kind: 'fast_mode_changed'; enabled: boolean }
  | { kind: 'multi_agent_changed'; enabled: boolean }

export type SlashResponse =
  | { kind: 'message'; markdown: string }
  | { kind: 'applied'; markdown: string; effects: SlashEffect[] }
  | { kind: 'needs_choice'; command: string; title: string; options: SlashChoice[] }
  | { kind: 'forward_to_agent'; prompt: string; agent_mode: boolean }
  | { kind: 'navigate'; target: SlashNavTarget; markdown: string }
  | { kind: 'exit' }
  | { kind: 'not_handled' }

export interface MintPlatformApi {
  authRegister(name: string | undefined, email: string, password: string): Promise<AuthUser>
  runSlashCommand(input: string, cwd?: string | null): Promise<SlashResponse>
  authLogin(email: string, password: string): Promise<AuthUser>
  authLogout(): Promise<void>
  authGetCurrentUser(): Promise<AuthUser | null>
  authUpdateProfile(name?: string, image?: string): Promise<AuthUser>
  authUploadAvatar(fileDataUri: string, fileName: string): Promise<AuthUser>
  getRuntimeStatus(): Promise<RuntimeStatus>
  detectSystemTools(): Promise<DetectedTools>
  sendChatMessage(
    message: string,
    imageDataUri?: string | null,
    audioDataUri?: string | null,
    videoDataUri?: string | null,
    documentAttachment?: any | null,
    workspacePath?: string | null,
    chatId?: string | null,
    agentId?: string | null,
  ): Promise<ChatResponse>
  streamChatMessage(
    message: string,
    onChunk: (chunk: string) => void,
    imageDataUri?: string | null,
    audioDataUri?: string | null,
    videoDataUri?: string | null,
    systemInstruction?: string,
    onProgress?: (progress: AgentProgress) => void,
    documentAttachment?: any | null,
    workspacePath?: string | null,
    chatId?: string | null,
    agentId?: string | null,
  ): Promise<ChatResponse>
  getTtsUrls(text: string): Promise<TtsUrl[]>
  cancelChatMessage(chatId: string): Promise<void>
  getRecentInteractions(limit?: number, chatId?: string | null): Promise<InteractionMemory[]>
  saveSystemInteraction(
    chatId: string,
    userText: string,
    aiText: string,
    provider: string,
    model: string,
  ): Promise<any>

  saveInteractionAgentActivity(interactionId: number, progress: any[]): Promise<void>
  listChatSessions(): Promise<ChatSession[]>
  deleteChatSession(chatId: string): Promise<number>
  renameChatSession(chatId: string, newTitle: string): Promise<number>
  getProfileValue(key: string): Promise<string>
  setProfileValue(key: string, value: string): Promise<boolean>
  listLearnedSkills(workspacePath?: string): Promise<LearnedSkill[]>
  addLearnedSkill(name: string, content: string): Promise<LearnedSkill>
  deleteLearnedSkill(name: string): Promise<number>
  listSubagents(workspacePath?: string): Promise<SubagentDefinition[]>
  saveSubagent(draft: SubagentDraft, workspacePath?: string): Promise<SubagentDefinition>
  deleteSubagent(sourcePath: string): Promise<void>
  clearChatHistory(chatId?: string | null): Promise<number>
  listSavedPictures(): Promise<PictureEntry[]>
  deleteSavedPicture(id: string): Promise<void>
  generateImages(req: ImageGenRequest): Promise<ImageGenResponse>
  getImageGenProviders(): Promise<ImageGenProviders>
  setDefaultImageProvider(provider: string): Promise<boolean>
  getWorkspaceTree(path?: string | null): Promise<WorkspaceTreeEntry>
  createWorkspaceFile(path: string): Promise<void>
  createWorkspaceFolder(path: string): Promise<void>
  deleteWorkspaceItem(path: string): Promise<void>
  selectWorkspaceDirectory(): Promise<string | null>
  submitToolApproval(token: string, approved: boolean, answer?: string): Promise<void>
  proposeCodeEdits(root: string, edits: CodeEdit[]): Promise<CodeEditProposal>
  applyCodeEdits(root: string, edits: CodeEdit[], approvalToken: string): Promise<any>
  listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void>
  readClipboardImage(): Promise<string | null>
}
