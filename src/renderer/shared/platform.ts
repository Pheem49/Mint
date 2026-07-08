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
  AgentProgress,
} from './types'

export interface MintPlatformApi {
  getRuntimeStatus(): Promise<RuntimeStatus>
  detectSystemTools(): Promise<DetectedTools>
  sendChatMessage(
    message: string,
    imageDataUri?: string | null,
    audioDataUri?: string | null,
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
  clearChatHistory(chatId?: string | null): Promise<number>
  listSavedPictures(): Promise<PictureEntry[]>
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
