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
  ConversationSnapshot,
  ConversationChanges,
  ChatSession,
  PictureEntry,
  ImageGenRequest,
  ImageGenProviders,
  ImageGenResponse,
  CodeEdit,
  CodeEditProposal,
  DetectedTools,
  LearnedSkill,
  SubagentDefinition,
  SubagentDraft,
  AgentProgress,
  AuthUser,
  GitCheckpoint,
  GitBranchInfo,
  GitBranchChangeOutcome,
  WorkspaceOperation,
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
  | { kind: 'plan_mode_changed'; enabled: boolean }

export type SlashResponse =
  | { kind: 'message'; markdown: string }
  | { kind: 'applied'; markdown: string; effects: SlashEffect[] }
  | { kind: 'needs_choice'; command: string; title: string; options: SlashChoice[] }
  | { kind: 'forward_to_agent'; prompt: string; agent_mode: boolean; plan_mode?: boolean }
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
    planMode?: boolean,
    pinnedMcpServer?: string | null,
    onApprovalRequested?: (payload: { token: string; approval: any }) => void,
    onTurnStarted?: (interactionId: number) => void,
  ): Promise<ChatResponse>
  getTtsUrls(text: string): Promise<TtsUrl[]>
  cancelChatMessage(chatId: string): Promise<void>
  getRecentInteractions(limit?: number, chatId?: string | null, workspacePath?: string | null): Promise<InteractionMemory[]>
  getConversationSnapshot(chatId: string, beforeId?: number | null, limit?: number): Promise<ConversationSnapshot>
  getConversationChanges(chatId: string, after: number, limit?: number): Promise<ConversationChanges>
  saveSystemInteraction(
    chatId: string,
    userText: string,
    aiText: string,
    provider: string,
    model: string,
  ): Promise<any>

  saveInteractionAgentActivity(interactionId: number, progress: any[]): Promise<void>
  listChatSessions(): Promise<ChatSession[]>
  updateChatSessionWorkspace(chatId: string, workspacePath: string | null): Promise<void>
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
  getWorkspaceSnapshot(operation: WorkspaceOperation): Promise<import('./types').WorkspaceSnapshot>
  getWorkspaceGitDiff(workspacePath: string): Promise<import('./types').FileChange[]>
  getGitBranchInfo(workspacePath: string): Promise<GitBranchInfo>
  switchGitBranch(workspacePath: string, branch: string, confirmedDirtyWorkspace?: boolean): Promise<GitBranchChangeOutcome>
  createGitBranch(workspacePath: string, branch: string, confirmedDirtyWorkspace?: boolean): Promise<GitBranchChangeOutcome>
  checkoutRemoteGitBranch(workspacePath: string, remoteBranch: string, confirmedDirtyWorkspace?: boolean): Promise<GitBranchChangeOutcome>
  getGitGraph(workspacePath: string): Promise<string[]>
  createWorkspaceFile(operation: WorkspaceOperation): Promise<import('./types').WorkspaceSnapshot>
  createWorkspaceFolder(operation: WorkspaceOperation): Promise<import('./types').WorkspaceSnapshot>
  deleteWorkspaceItem(operation: WorkspaceOperation): Promise<import('./types').WorkspaceSnapshot>
  selectWorkspaceDirectory(): Promise<string | null>
  selectLinkedFolderPath(): Promise<string | null>
  submitToolApproval(token: string, approved: boolean, answer?: string): Promise<void>
  proposeCodeEdits(root: string, edits: CodeEdit[]): Promise<CodeEditProposal>
  applyCodeEdits(root: string, edits: CodeEdit[], approvalToken: string): Promise<any>
  listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void>
  readClipboardImage(): Promise<string | null>
  listGitCheckpoints(chatId: string): Promise<GitCheckpoint[]>
  rollbackGitCheckpoint(chatId: string, step: number, workspacePath?: string): Promise<{ status: string; message: string }>
  undoGitCheckpoint(workspacePath?: string): Promise<{ status: string; message: string }>
  readWorkspaceFile(path: string, workspacePath?: string): Promise<string>
  testMcpConnection(
    url: string,
    headers?: Record<string, string>,
  ): Promise<{ ok: boolean; error?: string; server_info?: any; tools_count?: number; tools?: any[] }>
}

/** The small workspace interface shared renderer modules cross. */
export type WorkspacePlatform = Pick<
  MintPlatformApi,
  | 'getWorkspaceSnapshot'
  | 'getWorkspaceGitDiff'
  | 'getGitBranchInfo'
  | 'switchGitBranch'
  | 'createGitBranch'
  | 'checkoutRemoteGitBranch'
  | 'getGitGraph'
  | 'createWorkspaceFile'
  | 'createWorkspaceFolder'
  | 'deleteWorkspaceItem'
>

let workspaceAdapter: WorkspacePlatform | undefined
type RuntimeOperation = (...args: any[]) => any
export type RendererAdapter = MintPlatformApi & {
  APP_ICON_PATH: string
  resolveAvatarUrl: RuntimeOperation
  fetchProviderModels: RuntimeOperation
  fetchImageProviderModels: RuntimeOperation
  fetchVideoProviderModels: RuntimeOperation
  reauthMcpServer: RuntimeOperation
  listMcpServerTools: RuntimeOperation
  listCronJobs: RuntimeOperation
  addCronJob: RuntimeOperation
  removeCronJob: RuntimeOperation
  setCronJobEnabled: RuntimeOperation
  listLinkedFolders: RuntimeOperation
  addLinkedFolder: RuntimeOperation
  removeLinkedFolder: RuntimeOperation
  linkedFolderStatus: RuntimeOperation
  refreshLinkedFolder: RuntimeOperation
  listLinkedFolderNotes: RuntimeOperation
  readLinkedFolderNote: RuntimeOperation
  openLinkedFolderNote: RuntimeOperation
  startGeminiLiveSession: RuntimeOperation
  sendGeminiLiveAudioChunk: RuntimeOperation
  stopGeminiLiveSession: RuntimeOperation
  generateVideo: RuntimeOperation
  getVideoGenProviders: RuntimeOperation
  convertFileSrc: RuntimeOperation
  setActiveModel: RuntimeOperation
  selectWorkspaceDirectory: RuntimeOperation
  selectLinkedFolderPath: RuntimeOperation
  isTauriRuntime: RuntimeOperation
}
let rendererAdapter: RendererAdapter | undefined

/** Install the runtime implementation used by shared renderer modules. */
export function installRendererPlatform(adapter: RendererAdapter): void {
  rendererAdapter = adapter
}

function requireRendererPlatform(): RendererAdapter {
  if (!rendererAdapter) throw new Error('Renderer platform adapter has not been installed.')
  return rendererAdapter
}

function operation<Key extends keyof RendererAdapter>(name: Key): RendererAdapter[Key] {
  return ((...args: any[]) => {
    const implementation = requireRendererPlatform()[name]
    if (typeof implementation !== 'function') throw new Error(`Platform operation is unavailable: ${name}`)
    return (implementation as RuntimeOperation)(...args)
  }) as RendererAdapter[Key]
}

/** Authentication and profile seam. */
export const authPlatform = {
  authRegister: operation('authRegister'), authLogin: operation('authLogin'),
  authLogout: operation('authLogout'), authGetCurrentUser: operation('authGetCurrentUser'),
  authUploadAvatar: operation('authUploadAvatar'), resolveAvatarUrl: operation('resolveAvatarUrl'),
}

/** Conversation transport, persistence, live voice, and checkpoint seam. */
export const conversationPlatform = {
  clearChatHistory: operation('clearChatHistory'), deleteChatSession: operation('deleteChatSession'),
  renameChatSession: operation('renameChatSession'), getRecentInteractions: operation('getRecentInteractions'),
  getConversationSnapshot: operation('getConversationSnapshot'), getConversationChanges: operation('getConversationChanges'),
  saveSystemInteraction: operation('saveSystemInteraction'), listChatSessions: operation('listChatSessions'),
  updateChatSessionWorkspace: operation('updateChatSessionWorkspace'),
  saveInteractionAgentActivity: operation('saveInteractionAgentActivity'), streamChatMessage: operation('streamChatMessage'),
  cancelChatMessage: operation('cancelChatMessage'), submitToolApproval: operation('submitToolApproval'),
  listen: operation('listen'), readClipboardImage: operation('readClipboardImage'), getTtsUrls: operation('getTtsUrls'),
  startGeminiLiveSession: operation('startGeminiLiveSession'), sendGeminiLiveAudioChunk: operation('sendGeminiLiveAudioChunk'),
  stopGeminiLiveSession: operation('stopGeminiLiveSession'), listGitCheckpoints: operation('listGitCheckpoints'),
  rollbackGitCheckpoint: operation('rollbackGitCheckpoint'), undoGitCheckpoint: operation('undoGitCheckpoint'),
}

/** Models, skills, agents, MCP, schedules, linked folders, and slash commands. */
export const catalogPlatform = {
  fetchProviderModels: operation('fetchProviderModels'), listLearnedSkills: operation('listLearnedSkills'),
  addLearnedSkill: operation('addLearnedSkill'), deleteLearnedSkill: operation('deleteLearnedSkill'),
  detectSystemTools: operation('detectSystemTools'), listMcpServerTools: operation('listMcpServerTools'),
  testMcpConnection: operation('testMcpConnection'), reauthMcpServer: operation('reauthMcpServer'),
  listSubagents: operation('listSubagents'), saveSubagent: operation('saveSubagent'),
  deleteSubagent: operation('deleteSubagent'), getProfileValue: operation('getProfileValue'),
  setProfileValue: operation('setProfileValue'), listCronJobs: operation('listCronJobs'),
  addCronJob: operation('addCronJob'), removeCronJob: operation('removeCronJob'),
  setCronJobEnabled: operation('setCronJobEnabled'), listLinkedFolders: operation('listLinkedFolders'),
  addLinkedFolder: operation('addLinkedFolder'), removeLinkedFolder: operation('removeLinkedFolder'),
  linkedFolderStatus: operation('linkedFolderStatus'), refreshLinkedFolder: operation('refreshLinkedFolder'),
  listLinkedFolderNotes: operation('listLinkedFolderNotes'), readLinkedFolderNote: operation('readLinkedFolderNote'),
  openLinkedFolderNote: operation('openLinkedFolderNote'),
  runSlashCommand: operation('runSlashCommand'),
}

/** Image and video generation and saved media seam. */
export const mediaPlatform = {
  generateImages: operation('generateImages'), getImageGenProviders: operation('getImageGenProviders'),
  fetchImageProviderModels: operation('fetchImageProviderModels'), listSavedPictures: operation('listSavedPictures'),
  generateVideo: operation('generateVideo'), getVideoGenProviders: operation('getVideoGenProviders'),
  fetchVideoProviderModels: operation('fetchVideoProviderModels'), convertFileSrc: operation('convertFileSrc'),
}

/** Runtime capabilities and native picker seam. */
export const runtimePlatform = {
  appIconPath: () => String(requireRendererPlatform().APP_ICON_PATH ?? './assets/icon.png'),
  getRuntimeStatus: operation('getRuntimeStatus'), setActiveModel: operation('setActiveModel'),
  selectWorkspaceDirectory: operation('selectWorkspaceDirectory'), selectLinkedFolderPath: operation('selectLinkedFolderPath'),
  isTauriRuntime: operation('isTauriRuntime'), readWorkspaceFile: operation('readWorkspaceFile'),
}

/** Install the Desktop or Web adapter before rendering the application. */
export function installWorkspacePlatform(adapter: WorkspacePlatform): void {
  workspaceAdapter = adapter
}

function requireWorkspacePlatform(): WorkspacePlatform {
  if (!workspaceAdapter) {
    throw new Error('Workspace platform adapter has not been installed.')
  }
  return workspaceAdapter
}

/** One seam for workspace and Git behavior used by shared renderer modules. */
export const workspacePlatform: WorkspacePlatform = {
  getWorkspaceSnapshot: (...args) => requireWorkspacePlatform().getWorkspaceSnapshot(...args),
  getWorkspaceGitDiff: (...args) => requireWorkspacePlatform().getWorkspaceGitDiff(...args),
  getGitBranchInfo: (...args) => requireWorkspacePlatform().getGitBranchInfo(...args),
  switchGitBranch: (...args) => requireWorkspacePlatform().switchGitBranch(...args),
  createGitBranch: (...args) => requireWorkspacePlatform().createGitBranch(...args),
  checkoutRemoteGitBranch: (...args) => requireWorkspacePlatform().checkoutRemoteGitBranch(...args),
  getGitGraph: (...args) => requireWorkspacePlatform().getGitGraph(...args),
  createWorkspaceFile: (...args) => requireWorkspacePlatform().createWorkspaceFile(...args),
  createWorkspaceFolder: (...args) => requireWorkspacePlatform().createWorkspaceFolder(...args),
  deleteWorkspaceItem: (...args) => requireWorkspacePlatform().deleteWorkspaceItem(...args),
}
