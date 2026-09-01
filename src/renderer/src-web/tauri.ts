export * from '../shared/types'
import type { MintPlatformApi } from '../shared/platform'
import type {

  RuntimeStatus,
  ChatResponse,
  TtsUrl,
  DocumentAttachment,
  AgentProgress,
  InteractionMemory,
  ChatSession,
  PictureEntry,
  ImageGenRequest,
  ImageGenProviders,
  ImageGenResponse,
  WorkspaceTreeEntry,
  CodeEdit,
  CodeEditProposal,
  LearnedSkill,
  AuthUser,
  CronJob,
  CronJobDraft,
  LinkedFolder,
  LinkedFolderDraft,
} from '../shared/types'


type DesktopStreamEvent =
  | { type: 'chunk'; chunk: string }
  | { type: 'progress'; progress: AgentProgress }

export type { GeminiLiveEvent } from '../shared/utils/useGeminiLiveVoice'
import type { GeminiLiveEvent } from '../shared/utils/useGeminiLiveVoice'


export const isTauriRuntime = () => (
  typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
)

/** The last browser Notification shown by notifyAiResponse's fallback
 * (non-Tauri) branch, so clearAiNotifications can dismiss it. */
let lastAiNotification: Notification | null = null

/**
 * Root-relative: the web build's SPA fallback serves index-web.html at deep
 * paths like /chat/<id>, and a page-relative "./assets/..." would resolve
 * against that path instead of the site root, 404ing on those routes.
 */
export const APP_ICON_PATH = '/assets/icon.png'

export const getLocalApiBase = () => {
  const host = typeof window !== 'undefined' ? window.location.hostname : 'localhost';
  return `http://${host}:3000/api`;
};

const getApiBase = getLocalApiBase

const AUTH_TOKEN_KEY = 'mint_auth_token'

function getStoredAuthToken(): string | null {
  if (typeof window === 'undefined') return null
  try {
    return window.localStorage.getItem(AUTH_TOKEN_KEY)
  } catch {
    return null
  }
}

function setStoredAuthToken(token: string | null) {
  if (typeof window === 'undefined') return
  try {
    if (token) window.localStorage.setItem(AUTH_TOKEN_KEY, token)
    else window.localStorage.removeItem(AUTH_TOKEN_KEY)
  } catch {
    // ignore storage errors (e.g. private browsing)
  }
}

/**
 * fetch() wrapper that attaches the signed-in user's session token to every
 * request to this app's own local API server. Without this, the server has
 * no way to know who's calling and logs every request as "auth:anonymous"
 * even while a user is signed in, since only a handful of /auth/* endpoints
 * used to send the header explicitly.
 */
function authFetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  const token = getStoredAuthToken()
  if (!token) return fetch(input, init)
  const headers = new Headers(init.headers)
  if (!headers.has('Authorization')) headers.set('Authorization', `Bearer ${token}`)
  return fetch(input, { ...init, headers })
}

export async function authRegister(
  name: string | undefined,
  email: string,
  password: string,
): Promise<AuthUser> {
  if (!isTauriRuntime()) {
    const res = await authFetch(`${getApiBase()}/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, email, password }),
    })
    const data = await res.json()
    if (!res.ok) throw new Error(data.message || 'Failed to register.')
    setStoredAuthToken(data.token)
    return data.user
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<AuthUser>('auth_register', { name, email, password })
}

export async function authLogin(email: string, password: string): Promise<AuthUser> {
  if (!isTauriRuntime()) {
    const res = await authFetch(`${getApiBase()}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, password }),
    })
    const data = await res.json()
    if (!res.ok) throw new Error(data.message || 'Invalid email or password.')
    setStoredAuthToken(data.token)
    return data.user
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<AuthUser>('auth_login', { email, password })
}

export async function authLogout(): Promise<void> {
  if (!isTauriRuntime()) {
    const token = getStoredAuthToken()
    setStoredAuthToken(null)
    if (token) {
      await authFetch(`${getApiBase()}/auth/logout`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
      }).catch(() => {})
    }
    return
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('auth_logout')
}

export async function authGetCurrentUser(): Promise<AuthUser | null> {
  if (!isTauriRuntime()) {
    const token = getStoredAuthToken()
    if (!token) return null
    try {
      const res = await authFetch(`${getApiBase()}/auth/session`, {
        headers: { Authorization: `Bearer ${token}` },
      })
      if (!res.ok) return null
      const data = await res.json()
      return data.user ?? null
    } catch {
      return null
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<AuthUser | null>('auth_current_user')
}

export async function authUpdateProfile(name?: string, image?: string): Promise<AuthUser> {
  if (!isTauriRuntime()) {
    const token = getStoredAuthToken()
    if (!token) throw new Error('Not logged in')
    const res = await authFetch(`${getApiBase()}/auth/profile`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ name, image }),
    })
    const data = await res.json()
    if (!res.ok) throw new Error(data.message || 'Failed to update profile.')
    return data.user
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<AuthUser>('auth_update_profile', { name, image })
}

export async function authUploadAvatar(fileDataUri: string, fileName: string): Promise<AuthUser> {
  const dataBase64 = fileDataUri.split(',')[1] ?? fileDataUri
  if (!isTauriRuntime()) {
    const token = getStoredAuthToken()
    if (!token) throw new Error('Not logged in')
    const res = await authFetch(`${getApiBase()}/auth/avatar`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ fileName, dataBase64 }),
    })
    const data = await res.json()
    if (!res.ok) throw new Error(data.message || 'Failed to upload avatar.')
    return data.user
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<AuthUser>('auth_upload_avatar', { fileName, dataBase64 })
}

/**
 * `AuthUser.image` is a relative `/api/avatar?key=...` path scoped to
 * whichever Mint app the account's avatar was uploaded from. Re-point it at
 * this app's own API server so it resolves regardless of origin (this app's
 * `/api/avatar` route reads from the same shared Pictures folder).
 */
export function resolveAvatarUrl(image?: string | null): string | null {
  if (!image) return null
  if (/^https?:\/\//i.test(image)) return image
  const match = image.match(/[?&]key=([^&]+)/)
  if (!match) return image
  return `${getApiBase()}/avatar?key=${match[1]}`
}

export async function getRuntimeStatus(): Promise<RuntimeStatus> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/status`);
      return await res.json();
    } catch (e) {
      console.error("Failed to fetch runtime status from local server:", e);
      return {
        backend: 'browser-fallback',
        configPath: '',
        activeProvider: '',
        availableProviders: [],
        integrations: {}
      };
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<RuntimeStatus>('get_runtime_status')
}

export async function setActiveModel(provider: string, model?: string): Promise<string> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase()
    try {
      const res = await authFetch(`${API_BASE}/active-model`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ provider, model }),
      })
      const data = await res.json()
      return data.displayName || `Changed model to ${provider}`
    } catch (e) {
      console.error('Failed to set active model via API:', e)
      return `Changed model to ${provider}`
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<string>('set_active_model', { provider, model })
}

export async function uploadFile(file: File): Promise<string> {
  const API_BASE = getApiBase();
  const res = await authFetch(`${API_BASE}/uploads?filename=${encodeURIComponent(file.name)}`, {
    method: 'POST',
    body: file,
  });
  if (!res.ok) {
    const data = await res.json().catch(() => null);
    throw new Error(data?.error || `HTTP ${res.status}`);
  }
  const data = await res.json();
  return data.url;
}

export interface DetectedTools {
  docker: boolean
  git: boolean
  gh: boolean
  node: boolean
}

export async function detectSystemTools(): Promise<DetectedTools> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/detect-tools`);
      return await res.json();
    } catch (e) {
      console.error("Failed to detect tools from local server:", e);
      return { docker: false, git: false, gh: false, node: false };
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<DetectedTools>('detect_system_tools')
}

/**
 * Re-runs a configured MCP server's OAuth login in the foreground — fixes a
 * stale/expired refresh token (e.g. `invalid_grant` from a Gmail MCP
 * server). Opens the OAuth URL in the user's browser and blocks server-side
 * until the flow completes, so callers should keep the UI responsive
 * (disable just the triggering button) rather than blocking the whole view.
 */
export async function reauthMcpServer(serverName: string): Promise<boolean> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase()
    const res = await authFetch(`${API_BASE}/mcp/reauth`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ serverName }),
    })
    const data = await res.json().catch(() => null)
    if (!res.ok) {
      throw new Error(data?.error || `Re-authentication failed: HTTP ${res.status}`)
    }
    return Boolean(data?.success)
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<boolean>('reauth_mcp_server', { serverName })
}

/** Tool names a configured MCP server exposes — feeds the "Discover tools"
 *  picker in the MCP tool-allowlist UI. Can be slow or fail if the server is
 *  unreachable. */
export async function listMcpServerTools(name: string): Promise<string[]> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase()
    const res = await authFetch(`${API_BASE}/mcp/${encodeURIComponent(name)}/tools`)
    const data = await res.json().catch(() => null)
    if (!res.ok) {
      throw new Error(data?.error || `Could not list tools: HTTP ${res.status}`)
    }
    return Array.isArray(data?.tools) ? data.tools : []
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<string[]>('list_mcp_server_tools', { name })
}

export async function sendChatMessage(
  message: string,
  imageDataUri?: string | null,
  audioDataUri?: string | null,
  videoDataUri?: string | null,
  documentAttachment?: DocumentAttachment | null,
  workspacePath?: string | null,
  chatId?: string | null,
  agentId?: string | null,
): Promise<ChatResponse> {
  const outgoingMessage = withImagePlaceholder(message, imageDataUri, videoDataUri)
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/chat`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: outgoingMessage, systemInstruction: '', chatId, imageDataUri, audioDataUri, videoDataUri, documentAttachment, workspacePath, agentId })
      });
      const data = await res.json().catch(() => null);
      if (!res.ok) {
        return {
          provider: 'error',
          model: 'error',
          text: data?.text || data?.message || data?.status || `Local API returned HTTP ${res.status}`,
        };
      }
      if (!data || typeof data.text !== 'string') {
        return {
          provider: 'error',
          model: 'error',
          text: 'Local API returned an invalid chat response.',
        };
      }
      return data;
    } catch (e) {
      console.error("Failed to send chat message to local server:", e);
      return { provider: 'error', model: 'error', text: `Failed to connect to Local API Server: ${e}` };
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  const response = await invoke<ChatResponse>('send_chat_message', {
    request: { message: outgoingMessage, systemInstruction: '', chatId, imageDataUri, audioDataUri, videoDataUri, documentAttachment, workspacePath, agentId },
  })
  if (imageDataUri) {
    await invoke('save_pictures', {
      images: imageDataUri.split(' '),
      source: 'chat',
      message: outgoingMessage,
    })
  }
  if (videoDataUri) {
    await invoke('save_pictures', {
      images: videoDataUri.split(' '),
      source: 'chat',
      message: outgoingMessage,
    })
  }
  return response
}

export async function streamChatMessage(
  message: string,
  onChunk: (chunk: string) => void,
  imageDataUri?: string | null,
  audioDataUri?: string | null,
  videoDataUri?: string | null,
  systemInstruction = '',
  onProgress?: (progress: AgentProgress) => void,
  documentAttachment?: DocumentAttachment | null,
  workspacePath?: string | null,
  chatId?: string | null,
  agentId?: string | null,
  // Unused on web (no plan-mode-approval UI here) — kept only so this positional
  // arg list stays aligned with the desktop `tauri.ts`, since the shared
  // `MintDashboard.tsx` call site passes the same argument list to both builds.
  _planMode?: boolean,
  pinnedMcpServer?: string | null,
  // Web has no separate Tauri event bus for approvals (unlike desktop's
  // global `listen('tool-approval-requested', ...)`), so the server sends
  // an `approval-requested` event down this same ndjson stream instead —
  // this callback is how the caller learns about it.
  onApprovalRequested?: (payload: { token: string; approval: any }) => void,
): Promise<ChatResponse> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    const outgoingMessage = withImagePlaceholder(message, imageDataUri, videoDataUri);
    const res = await authFetch(`${API_BASE}/chat-stream`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: outgoingMessage, systemInstruction, chatId, imageDataUri, audioDataUri, videoDataUri, documentAttachment, workspacePath, agentId, pinnedMcpServer })
    });
    if (!res.ok) {
      const data = await res.json().catch(() => null);
      throw new Error(data?.text || data?.message || `HTTP ${res.status}`);
    }
    const reader = res.body?.getReader();
    if (!reader) {
      throw new Error("No response body reader");
    }
    const decoder = new TextDecoder();
    let buffer = '';
    let finalResponse: ChatResponse | null = null;
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';
      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const event = JSON.parse(line);
          if (event.type === 'chunk') {
            onChunk(event.chunk);
          } else if (event.type === 'progress') {
            onProgress?.(event.progress);
          } else if (event.type === 'done') {
            finalResponse = event.response;
          } else if (event.type === 'approval-requested') {
            onApprovalRequested?.({ token: event.token, approval: event.approval });
          }
        } catch (e) {
          console.error("Failed to parse stream line:", line, e);
        }
      }
    }
    if (finalResponse) return finalResponse;
    throw new Error("Stream closed without a final response");
  }
  const { invoke, Channel } = await import('@tauri-apps/api/core')
  const outgoingMessage = withImagePlaceholder(message, imageDataUri, videoDataUri)
  const onEvent = new Channel<DesktopStreamEvent>()
  onEvent.onmessage = (event) => {
    if (event.type === 'chunk') onChunk(event.chunk)
    else onProgress?.(event.progress)
  }
  const response = await invoke<ChatResponse>('stream_chat_message', {
    request: { message: outgoingMessage, systemInstruction, chatId, imageDataUri, audioDataUri, videoDataUri, documentAttachment, workspacePath, agentId, pinnedMcpServer },
    onEvent,
  })
  if (imageDataUri) {
    await invoke('save_pictures', {
      images: imageDataUri.split(' '),
      source: 'chat',
      message: outgoingMessage,
    })
  }
  if (videoDataUri) {
    await invoke('save_pictures', {
      images: videoDataUri.split(' '),
      source: 'chat',
      message: outgoingMessage,
    })
  }
  return response
}

function withImagePlaceholder(message: string, imageDataUri?: string | null, videoDataUri?: string | null) {
  let finalMessage = message
  if (imageDataUri && !finalMessage.includes('[Image #1]')) {
    const imageCount = imageDataUri.split(/\s+/).filter(Boolean).length
    const markers = Array.from({ length: imageCount }, (_, index) => `[Image #${index + 1}]`).join(' ')
    if (markers) finalMessage = `${finalMessage} ${markers}`
  }
  if (videoDataUri && !finalMessage.includes('[Video #1]')) {
    const videoCount = videoDataUri.split(/\s+/).filter(Boolean).length
    const markers = Array.from({ length: videoCount }, (_, index) => `[Video #${index + 1}]`).join(' ')
    if (markers) finalMessage = `${finalMessage} ${markers}`
  }
  return finalMessage
}

export async function getTtsUrls(text: string): Promise<TtsUrl[]> {
  if (typeof window === 'undefined') return []
  if (!isTauriRuntime()) return []
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<TtsUrl[]>('get_tts_urls', { text })
}

// Gemini Live has no Tauri IPC to ride on in the browser build, so it talks directly to a
// WebSocket route on the local API server (crates/mint-core/src/api_server.rs, "/api/gemini-live"),
// which bridges to the same gemini_live::start_session used by the desktop build.
const geminiLiveSockets = new Map<string, WebSocket>()

function geminiLiveWsUrl(params: URLSearchParams): string {
  const wsBase = getLocalApiBase().replace(/^http/, 'ws')
  return `${wsBase}/gemini-live?${params.toString()}`
}

/**
 * Starts a Gemini Live realtime voice session (beta). Returns a session id used by
 * `sendGeminiLiveAudioChunk`/`stopGeminiLiveSession`; `onEvent` receives audio replies,
 * transcripts, and tool-call status for the lifetime of the session.
 */
export async function startGeminiLiveSession(
  onEvent: (event: GeminiLiveEvent) => void,
  workspacePath?: string | null,
  chatId?: string | null,
): Promise<string> {
  const params = new URLSearchParams()
  const token = getStoredAuthToken()
  if (token) params.set('token', token)
  if (workspacePath) params.set('workspacePath', workspacePath)
  if (chatId) params.set('chatId', chatId)

  const sessionId = `gemini-live-${Date.now()}-${Math.random().toString(36).slice(2)}`
  const ws = new WebSocket(geminiLiveWsUrl(params))

  return new Promise((resolve, reject) => {
    let opened = false
    ws.onopen = () => {
      opened = true
      geminiLiveSockets.set(sessionId, ws)
      resolve(sessionId)
    }
    ws.onmessage = (event) => {
      try {
        onEvent(JSON.parse(event.data as string) as GeminiLiveEvent)
      } catch (error) {
        console.error('Failed to parse Gemini Live event', error)
      }
    }
    ws.onclose = () => {
      geminiLiveSockets.delete(sessionId)
      if (!opened) {
        reject(new Error('Failed to connect to Gemini Live (check your Gemini API key and sign-in status).'))
      } else {
        // Safety net for abnormal drops — the server also sends an explicit
        // {"type":"closed"} message on a clean shutdown, so this may fire twice;
        // the hook's handler is idempotent.
        onEvent({ type: 'closed' })
      }
    }
    ws.onerror = () => {
      // onclose always follows onerror for WebSocket; handled there.
    }
  })
}

/** Pushes a chunk of base64-encoded PCM16 (16kHz, mono) mic audio into a running session. */
export async function sendGeminiLiveAudioChunk(sessionId: string, chunkBase64: string): Promise<void> {
  const ws = geminiLiveSockets.get(sessionId)
  if (!ws || ws.readyState !== WebSocket.OPEN) return
  ws.send(JSON.stringify({ type: 'audio', data: chunkBase64 }))
}

export async function stopGeminiLiveSession(sessionId: string): Promise<void> {
  const ws = geminiLiveSockets.get(sessionId)
  if (!ws) return
  geminiLiveSockets.delete(sessionId)
  ws.close()
}

export async function cancelChatMessage(chatId: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    await authFetch(`${API_BASE}/cancel-chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ chatId })
    });
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('cancel_chat_message', { chatId })
}

export async function getRecentInteractions(limit = 50, chatId?: string | null, workspacePath?: string | null): Promise<InteractionMemory[]> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const params = new URLSearchParams({ limit: String(limit) });
      if (chatId) params.set('chatId', chatId);
      if (workspacePath) params.set('workspacePath', workspacePath);
      const res = await authFetch(`${API_BASE}/interactions?${params.toString()}`);
      return await res.json();
    } catch (e) {
      console.error("Failed to fetch chat history from local server:", e);
      return [];
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<InteractionMemory[]>('get_recent_interactions', { limit, chatId, workspacePath })
}

export async function saveSystemInteraction(
  chatId: string,
  userText: string,
  aiText: string,
  provider: string,
  model: string,
): Promise<any> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/interactions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ chatId, userText, aiText, provider, model }),
      });
      return await res.json();
    } catch (e) {
      console.error("Failed to save system interaction on local server:", e);
      return { success: false };
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke('save_system_interaction', { chatId, userText, aiText, provider, model })
}

export async function saveInteractionAgentActivity(
  interactionId: number,
  activity: AgentProgress[],
): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/interactions/agent-activity`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ interactionId, activity }),
      });
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
    } catch (error) {
      console.error('Failed to persist agent activity:', error);
    }
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('save_interaction_agent_activity', { interactionId, activity })
}

export async function listChatSessions(): Promise<ChatSession[]> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/chat-sessions`);
      const data = await res.json();
      return Array.isArray(data) ? data : [];
    } catch (e) {
      console.error("Failed to fetch chat sessions from local server:", e);
      return [];
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ChatSession[]>('list_chat_sessions')
}

export async function deleteChatSession(chatId: string): Promise<number> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const params = new URLSearchParams({ chatId });
      const res = await authFetch(`${API_BASE}/chat-sessions/delete?${params.toString()}`, { method: 'POST' });
      const data = await res.json();
      return typeof data?.deleted === 'number' ? data.deleted : 0;
    } catch (e) {
      console.error("Failed to delete chat session on local server:", e);
      return 0;
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<number>('delete_chat_session', { chatId })
}

export async function renameChatSession(chatId: string, newTitle: string): Promise<number> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/chat-sessions/rename`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ chatId, newTitle })
      });
      const data = await res.json();
      return typeof data?.updated === 'number' ? data.updated : 0;
    } catch (e) {
      console.error("Failed to rename chat session on local server:", e);
      return 0;
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<number>('rename_chat_session', { chatId, newTitle })
}

export async function getProfileValue(key: string): Promise<string> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const params = new URLSearchParams({ key });
      const res = await authFetch(`${API_BASE}/profile?${params.toString()}`);
      const data = await res.json();
      return data.value || '';
    } catch (e) {
      console.error("Failed to get profile key from local server:", e);
      return '';
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<string | null>('get_profile_value', { key }).then(res => res || '')
}

export async function setProfileValue(key: string, value: string): Promise<boolean> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/profile`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ key, value })
      });
      const data = await res.json();
      return data.status === 'ok';
    } catch (e) {
      console.error("Failed to set profile key on local server:", e);
      return false;
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<void>('set_profile_value', { key, value }).then(() => true).catch(() => false)
}

export async function clearChatHistory(chatId?: string | null): Promise<number> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const params = new URLSearchParams();
      if (chatId) params.set('chatId', chatId);
      const suffix = params.toString() ? `?${params.toString()}` : '';
      const res = await authFetch(`${API_BASE}/interactions/clear${suffix}`, { method: 'POST' });
      const data = await res.json();
      return data.status === 'ok' ? 1 : 0;
    } catch (e) {
      console.error("Failed to clear chat history on local server:", e);
      return 0;
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<number>('clear_chat_history', { chatId })
}

export async function listSavedPictures(): Promise<PictureEntry[]> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await authFetch(`${API_BASE}/pictures?_t=${Date.now()}`);
      const pictures = await res.json();
      const timestamp = Date.now();
      return Array.isArray(pictures)
        ? pictures.map((picture) => {
            const pictureUrl = picture.url ? `${API_BASE.replace('/api', '')}${picture.url}?_t=${timestamp}` : undefined
            return {
              ...picture,
              path: pictureUrl || picture.path,
              thumbnailPath: undefined,
              thumbnailUrl: pictureUrl,
              url: pictureUrl || picture.url,
            }
          })
        : [];
    } catch (e) {
      console.error("Failed to fetch saved pictures from local server:", e);
      return [];
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<PictureEntry[]>('list_pictures')
}

export async function deleteSavedPicture(id: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    await authFetch(`${API_BASE}/pictures/${encodeURIComponent(id)}`, { method: 'DELETE' });
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<void>('delete_picture', { id })
}

export async function generateImages(
  request: ImageGenRequest
): Promise<ImageGenResponse> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase()
    const res = await authFetch(`${API_BASE}/image-generate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        prompt: request.prompt,
        negativePrompt: request.negativePrompt,
        aspectRatio: request.aspectRatio ?? '1:1',
        numImages: request.numImages ?? 1,
        model: request.model,
        provider: request.provider,
      }),
    })
    if (!res.ok) {
      const err = await res.json().catch(() => null)
      throw new Error(err?.error || `Image generation failed: HTTP ${res.status}`)
    }
    const data = await res.json()
    // Normalize image URLs for web mode
    const images: PictureEntry[] = Array.isArray(data.images)
      ? data.images.map((pic: PictureEntry) => {
          const pictureUrl = pic.url ? `${API_BASE.replace('/api', '')}${pic.url}` : pic.url
          return { ...pic, url: pictureUrl, thumbnailUrl: pictureUrl }
        })
      : []
    return { ...data, images }
  }
  // Desktop / Tauri mode
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ImageGenResponse>('generate_images', { request })
}

/** Fetch which image-generation providers are currently configured on the backend. */
export async function getImageGenProviders(): Promise<ImageGenProviders> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase()
    try {
      const res = await authFetch(`${API_BASE}/image-gen/providers`)
      if (res.ok) return await res.json()
    } catch (_) { /* ignore */ }
    return { active: 'nanobanana', available: ['nanobanana'] }
  }
  // Desktop / Tauri: read config to know which keys are set
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const config = await invoke<Record<string, string>>('get_config')
    const available: string[] = []
    if (config.api_key)            available.push('nanobanana')
    if (config.openai_api_key)     available.push('dalle')
    if (config.stability_api_key)  available.push('stability')
    if (config.ideogram_api_key)   available.push('ideogram')
    if (config.replicate_api_key)  available.push('replicate')
    if (available.length === 0)    available.push('nanobanana')
    const active = available.includes(config.image_gen_provider)
      ? config.image_gen_provider
      : available[0]
    return { active, available }
  } catch (_) {
    return { active: 'nanobanana', available: ['nanobanana'] }
  }
}

/** Updates the default image generation provider in the configuration. */
export async function setDefaultImageProvider(provider: string): Promise<boolean> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase()
    try {
      const getRes = await authFetch(`${API_BASE}/config`)
      if (!getRes.ok) return false
      const config = await getRes.json()
      config.image_gen_provider = provider
      const saveRes = await authFetch(`${API_BASE}/config`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(config)
      })
      return saveRes.ok
    } catch (_) {
      return false
    }
  }

  // Desktop / Tauri
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const config = await invoke<any>('get_config')
    config.image_gen_provider = provider
    await invoke('update_config', { config })
    return true
  } catch (_) {
    return false
  }
}



export async function submitToolApproval(token: string, approved: boolean, answer?: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    await authFetch(`${API_BASE}/submit-approval`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token, approved, answer }),
    });
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke('submit_tool_approval', { token, approved, answer })
}

export async function proposeCodeEdits(root: string, edits: CodeEdit[]): Promise<CodeEditProposal> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    return { approvalRequired: false, approvalToken: '', edits: [] };
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<CodeEditProposal>('propose_desktop_code_edits', { root, edits })
}

export async function applyCodeEdits(root: string, edits: CodeEdit[], approvalToken: string) {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke('apply_desktop_code_edits', { root, edits, approvalToken })
}

export async function listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    return () => {};
  }
  const { listen: tauriListen } = await import('@tauri-apps/api/event');
  return tauriListen<T>(event, handler);
}

export function convertFileSrc(filePath: string, protocol = 'asset'): string {
  if (!filePath) return ''
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    if (filePath.startsWith('http://') || filePath.startsWith('https://') || filePath.startsWith('blob:')) {
      return filePath
    }
    const API_BASE = getApiBase()
    return `${API_BASE}/media?path=${encodeURIComponent(filePath)}`
  }
  const internals = (window as any).__TAURI_INTERNALS__;
  if (internals && typeof internals.convertFileSrc === 'function') {
    return internals.convertFileSrc(filePath, protocol);
  }
  const path = filePath.startsWith('\\\\?\\') ? filePath.substring(4) : filePath;
  return `https://asset.localhost/${encodeURIComponent(path)}`;
}

export function installTauriAdapters() {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    console.warn(`Not running inside Tauri. Connecting to local API server fallback at ${getApiBase()}.`);
    const API_BASE = getApiBase();

    (window as any).settingsApi = {
      getSettings: async () => {
        try {
          const res = await authFetch(`${API_BASE}/config`);
          return await res.json();
        } catch (e) {
          console.error("Failed to fetch settings from local server:", e);
          return {};
        }
      },
      getUpdaterStatus: async () => ({ supported: false, message: 'Desktop updates are only available in the Tauri app.' }),
      checkForUpdates: async () => ({ available: false, supported: false, message: 'Desktop updates are only available in the Tauri app.' }),
      installAvailableUpdate: async () => 'Desktop updates are only available in the Tauri app.',
      saveSettings: async (config: any) => {
        try {
          const res = await authFetch(`${API_BASE}/config`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(config)
          });
          return await res.json();
        } catch (e) {
          console.error("Failed to save settings to local server:", e);
          return {};
        }
      },
      closeSettings: () => {
        window.location.hash = '#/';
      },
      quitApp: () => undefined,
      openExternal: async (url: string) => {
        window.open(url, '_blank', 'noopener,noreferrer')
      },
      openFolder: async () => ({ success: false, message: 'Opening local folders is only available in the desktop app.' }),
    };

    (window as any).spotlightAPI = {
      submit: () => {},
      executeAction: async (action: any) => {
        try {
          const res = await authFetch(`${API_BASE}/action`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(action)
          });
          return await res.json();
        } catch (e) {
          return { success: false, message: String(e) };
        }
      },
      close: () => {},
      hide: () => {},
      resize: () => {},
      getSettings: async () => {
        try {
          const res = await authFetch(`${API_BASE}/config`);
          return await res.json();
        } catch (e) {
          return {};
        }
      },
      onSettingsChanged: () => {},
    };

    (window as any).widgetAPI = {
      onStateChange: () => {},
    };

    (window as any).screenPickerApi = {
      onScreenshot: () => {},
      sendSelection: () => {},
      startContinuousTranslation: () => {},
      stopContinuousTranslation: () => {},
      onTranslationResult: () => {},
      closePicker: () => {},
      setOverlayInteractable: () => {},
    };

    (window as any).api = {
      sendMessage: async (message: string, imageDataUri?: string | null, audioDataUri?: string | null, documentAttachment?: DocumentAttachment | null) => {
        try {
          const res = await authFetch(`${API_BASE}/chat`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ message, imageDataUri, audioDataUri, documentAttachment })
          });
          const data = await res.json().catch(() => null);
          if (!res.ok) {
            return {
              provider: 'error',
              model: 'error',
              text: data?.text || data?.message || data?.status || `Local API returned HTTP ${res.status}`,
            };
          }
          if (!data || typeof data.text !== 'string') {
            return { provider: 'error', model: 'error', text: 'Local API returned an invalid chat response.' };
          }
          return data;
        } catch (e) {
          console.error("Failed to send message to local server:", e);
          return { provider: 'error', model: 'error', text: `Failed to connect to Local API Server: ${e}` };
        }
      },
      closeWindow: () => {},
      minimizeWindow: () => {},
      quitApp: () => undefined,
      maximizeWindow: () => {},
      resetChat: async () => {
        try {
          const res = await authFetch(`${API_BASE}/interactions/clear`, { method: 'POST' });
          const data = await res.json();
          return data.status === 'ok' ? 1 : 0;
        } catch (e) {
          return 0;
        }
      },
      getChatHistory: async () => {
        try {
          const res = await authFetch(`${API_BASE}/interactions`);
          return await res.json();
        } catch (e) {
          console.error("Failed to fetch chat history from local server:", e);
          return [];
        }
      },
      listSavedPictures,
      deleteSavedPicture,
      openSettings: () => {
        window.location.hash = '#/settings';
      },
      readClipboard: async () => '',
      writeClipboard: async () => {},
      getSystemInfo: async () => {
        try {
          const res = await authFetch(`${API_BASE}/status`);
          return await res.json();
        } catch (e) {
          return { backend: 'browser-fallback' };
        }
      },
      getWeather: async (city: string) => {
        try {
          const res = await authFetch(`${API_BASE}/weather?city=${encodeURIComponent(city)}`);
          return await res.json();
        } catch (e) {
          return { error: String(e) };
        }
      },
      getSettings: async () => {
        try {
          const res = await authFetch(`${API_BASE}/config`);
          return await res.json();
        } catch (e) {
          return {};
        }
      },
      saveSettings: async (config: any) => {
        try {
          const res = await authFetch(`${API_BASE}/config`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(config)
          });
          return await res.json();
        } catch (e) {
          return {};
        }
      },
      closeSettings: async () => {
        if (window.location.hash.includes('settings')) {
          window.history.replaceState(null, '', window.location.pathname + window.location.search)
        }
      },
      onSettingsChanged: () => {},
      startVision: () => {},
      onVisionReady: async () => () => {},
      captureSilentScreen: async () => '',
      getSmartContext: async () => {
        try {
          const res = await authFetch(`${API_BASE}/smart-context`);
          return await res.json();
        } catch (e) {
          return {};
        }
      },
      onProactiveSuggestion: async () => () => {},
      onProactiveNotification: async () => () => {},
      toggleProactive: () => {},
      recordBehavior: () => {},
      executeProactiveAction: async (action: any) => {
        try {
          const res = await authFetch(`${API_BASE}/action`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(action)
          });
          return await res.json();
        } catch (e) {
          return { success: false, message: String(e) };
        }
      },
      executeApprovedAction: async (action: any) => {
        try {
          const res = await authFetch(`${API_BASE}/action`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(action)
          });
          return await res.json();
        } catch (e) {
          return { success: false, message: String(e) };
        }
      },
      onSpotlightToChat: async () => () => {},
      notifyAiResponse: async (preview: string) => {
        if (typeof Notification === 'undefined') return
        let permission = Notification.permission
        if (permission === 'default') {
          permission = await Notification.requestPermission()
        }
        if (permission === 'granted') {
          lastAiNotification = new Notification('Mint Agent', { body: preview })
        }
      },
      clearAiNotifications: () => {
        lastAiNotification?.close()
        lastAiNotification = null
      },
      getTtsUrls: async () => [],
      setAiState: () => {},
    };
    return;
  }

  const settingsChanged = async (callback: (config: any) => void) => {
    const { listen } = await import('@tauri-apps/api/event')
    void listen<any>('settings-changed', (event) => callback(event.payload))
  }
  const executeAction = async (action: any, approved = false) => {
    const { invoke } = await import('@tauri-apps/api/core')
    return action.type === 'plugin'
      ? invoke('run_native_plugin', { name: action.pluginName, instruction: action.target || '' })
      : invoke('run_desktop_action', { action: { ...action, approved } })
  }

  window.settingsApi = {
    getSettings: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('get_config')
    },
    getUpdaterStatus: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('get_updater_status')
    },
    checkForUpdates: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('check_for_updates')
    },
    installAvailableUpdate: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('install_available_update', { approved: true })
    },
    saveSettings: async (config) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('update_config', { config })
    },
    closeSettings: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      return invoke('close_desktop_window', { label: getCurrentWindow().label })
    },
    quitApp: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return void invoke('exit_app')
    },
    openExternal: async (url) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('run_desktop_action', { action: { type: 'open_url', target: url } })
    },
    openFolder: async (path) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('open_folder', { path })
    },
  }

  window.spotlightAPI = {
    submit: async (query) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return void invoke('submit_spotlight', { query })
    },
    executeAction: async (action) => {
      const { invoke } = await import('@tauri-apps/api/core')
      if (action.type === 'clipboard_write') {
        await navigator.clipboard.writeText(action.target)
        return { success: true }
      }
      return invoke('run_desktop_action', { action })
    },
    close: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      return invoke('close_desktop_window', { label: getCurrentWindow().label })
    },
    hide: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      return invoke('hide_desktop_window', { label: getCurrentWindow().label })
    },
    resize: async (width, height) => {
      const { invoke } = await import('@tauri-apps/api/core')
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      return void invoke('resize_desktop_window', {
        label: getCurrentWindow().label,
        width,
        height,
      })
    },
    getSettings: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('get_config')
    },
    onSettingsChanged: settingsChanged,
  }

  window.widgetAPI = {
    onStateChange: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event')
      void listen<string>('widget-state', (event) => callback(event.payload))
    },
  }

  window.screenPickerApi = {
    onScreenshot: async (callback) => {
      const { invoke } = await import('@tauri-apps/api/core')
      try {
        const image = await captureSharedScreen()
        callback(image)
      } catch (reason) {
        console.warn('Screen share capture failed, falling back to native capture:', reason)
        void invoke<string>('capture_silent_screen').then(callback)
      }
    },
    sendSelection: async (image) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return void invoke('submit_screen_selection', { image })
    },
    startContinuousTranslation: (rect) => {
      let translationTimer: ReturnType<typeof setInterval> | null = null
      const translate = async () => {
        const { invoke } = await import('@tauri-apps/api/core')
        void invoke<string>('translate_capture_region', { rect })
          .then((text) => window.dispatchEvent(new CustomEvent('mint-translation', { detail: text })))
          .catch((reason) => {
            window.dispatchEvent(new CustomEvent('mint-translation', { detail: String(reason) }))
          })
      }
      translate()
      translationTimer = setInterval(translate, 3000)
      
      // Clean up helper attached to window if needed
      if ((window as any)._stopTranslate) (window as any)._stopTranslate()
      ;(window as any)._stopTranslate = () => {
        if (translationTimer) clearInterval(translationTimer)
      }
    },
    stopContinuousTranslation: () => {
      if ((window as any)._stopTranslate) {
        (window as any)._stopTranslate()
      }
    },
    onTranslationResult: (callback) => {
      window.addEventListener('mint-translation', ((event: CustomEvent<string>) => {
        callback(event.detail)
      }) as EventListener)
    },
    closePicker: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      return invoke('close_desktop_window', { label: getCurrentWindow().label })
    },
    setOverlayInteractable: () => {},
  }

  window.api = {
    sendMessage: (message, imageDataUri, audioDataUri, documentAttachment) => sendChatMessage(message, imageDataUri, audioDataUri, undefined, documentAttachment),
    closeWindow: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      return invoke('hide_desktop_window', { label: getCurrentWindow().label })
    },
    minimizeWindow: async () => {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      return void getCurrentWindow().minimize()
    },
    quitApp: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return void invoke('exit_app')
    },
    maximizeWindow: async () => {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      return void getCurrentWindow().toggleMaximize()
    },
    resetChat: clearChatHistory,
    getChatHistory: () => getRecentInteractions(50),
    listSavedPictures,
    deleteSavedPicture,
    openSettings: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('open_window', { kind: 'settings' })
    },
    readClipboard: () => navigator.clipboard.readText(),
    writeClipboard: (text) => navigator.clipboard.writeText(text),
    getSystemInfo: async () => ({ backend: 'rust' }),
    getWeather: async (city) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('get_weather', { city })
    },
    getSettings: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('get_config')
    },
    saveSettings: async (config) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('update_config', { config })
    },
    onSettingsChanged: settingsChanged,
    startVision: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      try {
        const image = await captureSharedScreen()
        window.localStorage.setItem('mint:pending-screen-capture', image)
      } catch (reason) {
        console.warn('Screen share capture failed before opening picker:', reason)
        const image = await invoke<string>('capture_silent_screen')
        window.localStorage.setItem('mint:pending-screen-capture', image)
      }
      return invoke('start_screen_capture')
    },
    onVisionReady: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event')
      return listen<string>('vision-ready', (event) => callback(event.payload))
    },
    captureSilentScreen: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('capture_silent_screen')
    },
    getSmartContext: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('get_smart_context')
    },
    onProactiveSuggestion: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event')
      return listen<any>('proactive-suggestion', (event) => callback(event.payload))
    },
    onProactiveNotification: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event')
      return listen<any>('proactive-notification', (event) => callback(event.payload))
    },
    toggleProactive: async (enabled) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return void invoke('toggle_proactive', { enabled })
    },
    recordBehavior: async (context) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return void invoke('save_behavior_context', { context })
    },
    executeProactiveAction: (action) => executeAction(action),
    executeApprovedAction: (action) => executeAction(action, true),
    onSpotlightToChat: async (callback) => {
      const { listen } = await import('@tauri-apps/api/event')
      return listen<string>('spotlight-to-chat', (event) => callback(event.payload))
    },
    notifyAiResponse: async (preview) => {
      const { isPermissionGranted, requestPermission, sendNotification } = await import('@tauri-apps/plugin-notification')
      let granted = await isPermissionGranted()
      if (!granted) {
        granted = (await requestPermission()) === 'granted'
      }
      if (granted) {
        sendNotification({ title: 'Mint Agent', body: preview })
      }
    },
    // The plugin has no reliable cross-platform way to dismiss an
    // already-shown OS notification from JS — best-effort no-op; there's
    // simply nothing stale left for the next unfocused reply to build on.
    clearAiNotifications: () => {},
    getTtsUrls: async (text) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('get_tts_urls', { text })
    },
    setAiState: async (state) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return void invoke('set_ai_state', { state })
    },
  }
}

async function captureSharedScreen(): Promise<string> {
  // On Linux (especially under Wayland/WebKitGTK), getDisplayMedia often returns a black screen
  // or fails silently. Bypass it to force fallback to native screenshot commands.
  if (navigator.userAgent.toLowerCase().includes('linux')) {
    throw new Error('Linux detected, bypassing getDisplayMedia to use native screenshot tools')
  }

  if (!navigator.mediaDevices?.getDisplayMedia) {
    throw new Error('getDisplayMedia is not available')
  }

  const stream = await navigator.mediaDevices.getDisplayMedia({
    video: true,
    audio: false,
  })
  try {
    const video = document.createElement('video')
    video.srcObject = stream
    video.muted = true
    await video.play()
    await new Promise<void>((resolve) => {
      if (video.videoWidth > 0 && video.videoHeight > 0) {
        resolve()
      } else {
        video.onloadedmetadata = () => resolve()
      }
    })

    const canvas = document.createElement('canvas')
    canvas.width = video.videoWidth || window.screen.width
    canvas.height = video.videoHeight || window.screen.height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('Unable to create screen capture canvas')
    context.drawImage(video, 0, 0, canvas.width, canvas.height)
    return canvas.toDataURL('image/png')
  } finally {
    stream.getTracks().forEach((track) => track.stop())
  }
}

export async function listLearnedSkills(workspacePath?: string): Promise<LearnedSkill[]> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    try {
      const API_BASE = getLocalApiBase()
      const res = await authFetch(`${API_BASE}/learned-skills`)
      if (res.ok) {
        return await res.json()
      }
      const val = await getProfileValue('learned-skills-web-mock')
      if (val) {
        return JSON.parse(val)
      }
      return []
    } catch (e) {
      console.error("Failed to load web skills:", e)
      return []
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<LearnedSkill[]>('list_learned_skills', { workspacePath })
}

export async function addLearnedSkill(name: string, content: string): Promise<LearnedSkill> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    try {
      const list = await listLearnedSkills()
      const newSkill: LearnedSkill = {
        id: Date.now(),
        name,
        sourcePath: 'ui_manual',
        content,
        updatedAt: new Date().toISOString()
      }
      list.push(newSkill)
      await setProfileValue('learned-skills-web-mock', JSON.stringify(list))
      return newSkill
    } catch (e) {
      console.error("Failed to add web mock skill:", e)
      throw e
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<LearnedSkill>('add_learned_skill', { name, content })
}

export async function deleteLearnedSkill(name: string): Promise<number> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    try {
      const list = await listLearnedSkills()
      const filtered = list.filter(s => s.name !== name)
      const deletedCount = list.length - filtered.length
      await setProfileValue('learned-skills-web-mock', JSON.stringify(filtered))
      return deletedCount
    } catch (e) {
      console.error("Failed to delete web mock skill:", e)
      return 0
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<number>('delete_learned_skill', { name })
}

export interface SubagentDefinition {
  name: string
  description: string
  tools: string[] | null
  model: string | null
  provider: string | null
  systemPrompt: string
  sourcePath: string
}

export interface SubagentDraft {
  name: string
  description: string
  tools: string[] | null
  model: string | null
  provider: string | null
  systemPrompt: string
  scope: 'global' | 'workspace'
  previousSourcePath?: string | null
}

export async function listSubagents(workspacePath?: string): Promise<SubagentDefinition[]> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/subagents`)
    if (!res.ok) return []
    return res.json()
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<SubagentDefinition[]>('list_subagents', { workspacePath })
}

export async function saveSubagent(draft: SubagentDraft, workspacePath?: string): Promise<SubagentDefinition> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/subagents`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(draft)
    })
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: 'Failed to save subagent.' }))
      throw new Error(err.error || 'Failed to save subagent.')
    }
    return res.json()
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<SubagentDefinition>('save_subagent', { draft, workspacePath })
}

export async function deleteSubagent(sourcePath: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/subagents/${encodeURIComponent(sourcePath)}`, {
      method: 'DELETE'
    })
    if (!res.ok) throw new Error('Failed to delete subagent.')
    return
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('delete_subagent', { sourcePath })
}

export async function runSlashCommand(
  input: string,
  cwd?: string | null
): Promise<import('../shared/platform').SlashResponse> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/slash`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ input, cwd: cwd ?? null })
    })
    if (!res.ok) return { kind: 'not_handled' }
    return res.json()
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke('run_slash_command', { input, cwd: cwd ?? null })
}

export async function listCronJobs(): Promise<CronJob[]> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/cron`)
    if (!res.ok) return []
    return res.json()
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<CronJob[]>('list_cron_jobs')
}

export async function addCronJob(draft: CronJobDraft): Promise<CronJob> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/cron`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(draft)
    })
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: 'Failed to create scheduled task.' }))
      throw new Error(err.error || 'Failed to create scheduled task.')
    }
    return res.json()
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<CronJob>('add_cron_job', { draft })
}

export async function removeCronJob(id: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/cron/${encodeURIComponent(id)}`, {
      method: 'DELETE'
    })
    if (!res.ok) throw new Error('Failed to remove scheduled task.')
    return
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('remove_cron_job', { id })
}

export async function setCronJobEnabled(id: string, enabled: boolean): Promise<CronJob | null> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/cron/${encodeURIComponent(id)}/${enabled ? 'enable' : 'disable'}`, {
      method: 'POST'
    })
    if (!res.ok) throw new Error('Failed to update scheduled task.')
    return res.json()
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<CronJob | null>('set_cron_job_enabled', { id, enabled })
}

export async function listLinkedFolders(): Promise<Record<string, LinkedFolder>> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/linked-folders`)
    if (!res.ok) return {}
    return res.json()
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<Record<string, LinkedFolder>>('list_linked_folders')
}

export async function addLinkedFolder(draft: LinkedFolderDraft): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/linked-folders`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(draft)
    })
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: 'Failed to link folder.' }))
      throw new Error(err.error || 'Failed to link folder.')
    }
    return
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('add_linked_folder', { draft })
}

export async function removeLinkedFolder(name: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const res = await authFetch(`${getLocalApiBase()}/linked-folders/${encodeURIComponent(name)}`, {
      method: 'DELETE'
    })
    if (!res.ok) throw new Error('Failed to unlink folder.')
    return
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('remove_linked_folder', { name })
}

export async function getWorkspaceTree(path?: string | null): Promise<WorkspaceTreeEntry> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    return {
      name: 'Workspace',
      path: '.',
      kind: 'directory',
      children: [
        { name: 'src', path: 'src', kind: 'directory', children: [] },
        { name: 'package.json', path: 'package.json', kind: 'file', children: [] },
      ],
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<WorkspaceTreeEntry>('get_workspace_tree', { path })
}

export async function createWorkspaceFile(path: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) return
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke('create_workspace_file', { path })
}

export async function createWorkspaceFolder(path: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) return
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke('create_workspace_folder', { path })
}

export async function deleteWorkspaceItem(path: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) return
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke('delete_workspace_item', { path })
}

export async function selectWorkspaceDirectory(): Promise<string | null> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    return null
  }
  const { invoke } = await import('@tauri-apps/api/core')
  const selected = await invoke<string | null>('select_workspace_directory')
  return selected
}

/** Folder picker for the Linked Folders "Browse…" button. The browser can't
 * hand back a real filesystem path, but `mint web` runs on this same machine,
 * so ask the server to open its own native dialog (that route is gated to
 * loopback callers server-side). Returns null on cancel or if no picker /
 * display is available. */
export async function selectLinkedFolderPath(): Promise<string | null> {
  if (typeof window !== 'undefined' && isTauriRuntime()) {
    const { invoke } = await import('@tauri-apps/api/core')
    const selected = await invoke<string | null>('select_workspace_directory')
    return selected?.trim() || null
  }
  try {
    const res = await authFetch(`${getLocalApiBase()}/select-folder`, { method: 'POST' })
    if (!res.ok) return null
    const data = await res.json().catch(() => null)
    const path = data && typeof data.path === 'string' ? data.path.trim() : ''
    return path || null
  } catch {
    return null
  }
}

export async function readClipboardImage(): Promise<string | null> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {

    return null
  }
  const { invoke } = await import('@tauri-apps/api/core')
  try {
    return await invoke<string>('read_clipboard_image')
  } catch (err) {
    console.warn('Failed to read clipboard image via Tauri command:', err)
    return null
  }
}

// ─── Veo Studio — Video Generation (stub; real Tauri command in follow-up) ───

export interface VideoGenRequest {
  prompt: string
  negativePrompt?: string
  aspectRatio: '16:9' | '9:16' | '1:1'
  duration: 5 | 8
  model?: string
  provider: string
}

export interface VideoGenEntry {
  id: string
  url: string
  path: string
  message?: string
  createdAt?: string
}

export interface VideoGenResponse {
  videos: VideoGenEntry[]
  provider: string
  model: string
  description?: string
}

export interface VideoGenProviders {
  active: string
  available: string[]
}

/**
 * Generate a video using the configured provider.
 * NOTE: This is a frontend stub — the real Tauri backend command will be
 * added in a follow-up PR once the Veo REST API integration is ready.
 */
export async function generateVideo(request: VideoGenRequest): Promise<VideoGenResponse> {
  const apiBase = getLocalApiBase()
  const response = await authFetch(`${apiBase}/video-generate`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request)
  })
  if (!response.ok) {
    const errorData = await response.json().catch(() => ({}))
    throw new Error(errorData.error || `HTTP ${response.status} from API`)
  }
  return response.json()
}

export async function getVideoGenProviders(): Promise<VideoGenProviders> {
  return { active: 'veo', available: ['veo'] }
}

// ─── AI Video Editor — Edit API ────────────────────────────────────────────

export interface VideoInfo {
  path: string
  duration: number
  fps: number
  width: number
  height: number
  hasAudio: boolean
  audioStreams: number
  sizeBytes: number
  format: string
}

export interface VideoEditResult {
  outputPath: string
  duration?: number
  sizeBytes?: number
}

export interface VideoTrimRequest {
  input: string
  output: string
  start: number
  end: number
}

export interface VideoResizeRequest {
  input: string
  output: string
  width: number
  height: number
}

export interface VideoMergeRequest {
  inputs: string[]
  output: string
}

export interface VideoExtractAudioRequest {
  input: string
  output: string
}

export interface VideoRemoveSilenceRequest {
  input: string
  output: string
  thresholdDb?: number
  minSilenceSecs?: number
}

export interface VideoExportRequest {
  input: string
  output: string
  resolution?: string
  fps?: number
  codec?: string
  crf?: number
}

export interface TimelineClip {
  source: string
  trimStart?: number
  trimEnd?: number
  order?: number
  scale?: { width: number; height: number }
}

export interface TimelineSubtitle {
  start: number
  end: number
  text: string
}

export interface TimelineAudio {
  music?: string
  duck?: boolean
  musicVolume?: number
  duckVolume?: number
}

export interface TimelineOutput {
  path: string
  resolution?: string
  fps?: number
  codec?: string
  crf?: number
}

export interface VideoTimeline {
  clips: TimelineClip[]
  subtitles?: TimelineSubtitle[]
  audio?: TimelineAudio
  output: TimelineOutput
}

export interface RenderTimelineResult {
  outputPath: string
  clipsRendered: number
  duration?: number
  sizeBytes?: number
}

async function videoEditPost<T>(route: string, body: unknown): Promise<T> {
  const apiBase = getLocalApiBase()
  const res = await authFetch(`${apiBase}${route}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    const data = await res.json().catch(() => ({}))
    throw new Error((data as any).error || `HTTP ${res.status}`)
  }
  return res.json()
}

export async function videoLoad(path: string): Promise<VideoInfo> {
  return videoEditPost<VideoInfo>('/video/load', { path })
}

export async function videoTrim(req: VideoTrimRequest): Promise<VideoEditResult> {
  return videoEditPost<VideoEditResult>('/video/trim', req)
}

export async function videoResize(req: VideoResizeRequest): Promise<VideoEditResult> {
  return videoEditPost<VideoEditResult>('/video/resize', req)
}

export async function videoMerge(req: VideoMergeRequest): Promise<VideoEditResult> {
  return videoEditPost<VideoEditResult>('/video/merge', req)
}

export async function videoExtractAudio(req: VideoExtractAudioRequest): Promise<VideoEditResult> {
  return videoEditPost<VideoEditResult>('/video/extract-audio', req)
}

export async function videoRemoveSilence(req: VideoRemoveSilenceRequest): Promise<VideoEditResult> {
  return videoEditPost<VideoEditResult>('/video/remove-silence', req)
}

export async function videoExport(req: VideoExportRequest): Promise<VideoEditResult> {
  return videoEditPost<VideoEditResult>('/video/export', req)
}

export async function videoRenderTimeline(timeline: VideoTimeline): Promise<RenderTimelineResult> {
  return videoEditPost<RenderTimelineResult>('/video/render-timeline', { timeline })
}

// ─── Speech & Subtitle API ──────────────────────────────────────────────────

export interface TranscriptSegment {
  id: number
  start: number
  end: number
  text: string
  speaker?: string
}

export interface TranscriptionResult {
  text: string
  language: string
  duration: number
  segments: TranscriptSegment[]
}

export interface TranscribeRequest {
  input: string
  language?: string
  prompt?: string
}

export interface DetectSilenceRequest {
  input: string
  thresholdDb?: number
  minDurationSecs?: number
}

export interface SilenceRange {
  start: number
  end: number
  duration: number
}

export interface SubtitleStyle {
  fontName?: string
  fontSize?: number
  primaryColor?: string
  outlineColor?: string
  outline?: number
  alignment?: number
  marginV?: number
}

export interface BurnSubtitleRequest {
  inputVideo: string
  srtInput: string
  outputVideo: string
  style?: SubtitleStyle
  preset?: string
}

export interface TranslateSubtitleRequest {
  srtContent: string
  targetLanguage: string
}

export async function speechTranscribe(req: TranscribeRequest): Promise<TranscriptionResult> {
  return videoEditPost<TranscriptionResult>('/speech/transcribe', req)
}

export async function speechDetectSilence(req: DetectSilenceRequest): Promise<SilenceRange[]> {
  return videoEditPost<SilenceRange[]>('/speech/detect-silence', req)
}

export async function subtitleGenerate(segments: TranscriptSegment[]): Promise<{ srt: string }> {
  return videoEditPost<{ srt: string }>('/subtitle/generate', { segments })
}

export async function subtitleTranslate(req: TranslateSubtitleRequest): Promise<{ srt: string }> {
  return videoEditPost<{ srt: string }>('/subtitle/translate', req)
}

export async function subtitleBurn(req: BurnSubtitleRequest): Promise<VideoEditResult> {
  return videoEditPost<VideoEditResult>('/subtitle/burn', req)
}

// ─── Auto Shorts API ────────────────────────────────────────────────────────

export interface MakeShortsRequest {
  input: string
  outputDir?: string
  maxClips?: number
  targetDuration?: number
  burnSubtitles?: boolean
  width?: number
  height?: number
}

export interface ShortClipInfo {
  id: number
  path: string
  start: number
  end: number
  duration: number
  title: string
}

export interface MakeShortsResult {
  clips: ShortClipInfo[]
}

export async function videoMakeShorts(req: MakeShortsRequest): Promise<MakeShortsResult> {
  return videoEditPost<MakeShortsResult>('/video/make-shorts', req)
}

export interface VideoAiEditRequest {
  input: string
  output?: string
  instruction: string
}

export interface AiEditStepResult {
  step: number
  operation: string
  description: string
  outputPath: string
}

export interface AiEditVideoResult {
  outputPath: string
  stepsPerformed: AiEditStepResult[]
  summary: string
}

export async function videoAiEdit(req: VideoAiEditRequest): Promise<AiEditVideoResult> {
  return videoEditPost<AiEditVideoResult>('/video/ai-edit', req)
}



// Enforce compile-time check against the shared platform interface
const _apiCheck: MintPlatformApi = {
  runSlashCommand,
  authRegister,
  authLogin,
  authLogout,
  authGetCurrentUser,
  authUpdateProfile,
  authUploadAvatar,
  getRuntimeStatus,
  detectSystemTools,
  sendChatMessage,
  streamChatMessage,
  getTtsUrls,
  cancelChatMessage,
  getRecentInteractions,
  saveSystemInteraction,
  saveInteractionAgentActivity,
  listChatSessions,
  deleteChatSession,
  renameChatSession,
  getProfileValue,
  setProfileValue,
  listLearnedSkills,
  addLearnedSkill,
  deleteLearnedSkill,
  listSubagents,
  saveSubagent,
  deleteSubagent,
  clearChatHistory,
  listSavedPictures,
  deleteSavedPicture,
  generateImages,
  getImageGenProviders,
  setDefaultImageProvider,
  getWorkspaceTree,
  createWorkspaceFile,
  createWorkspaceFolder,
  deleteWorkspaceItem,
  selectWorkspaceDirectory,
  selectLinkedFolderPath,
  submitToolApproval,
  proposeCodeEdits,
  applyCodeEdits,
  listen,
  readClipboardImage,
}

