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
  DetectedTools,
  LearnedSkill,
} from '../shared/types'


type DesktopStreamEvent =
  | { type: 'chunk'; chunk: string }
  | { type: 'progress'; progress: AgentProgress }


export const isTauriRuntime = () => (
  typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
)

export const getLocalApiBase = () => {
  const host = typeof window !== 'undefined' ? window.location.hostname : 'localhost';
  return `http://${host}:3000/api`;
};

const getApiBase = getLocalApiBase

export async function getRuntimeStatus(): Promise<RuntimeStatus> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await fetch(`${API_BASE}/status`);
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
      const res = await fetch(`${API_BASE}/detect-tools`);
      return await res.json();
    } catch (e) {
      console.error("Failed to detect tools from local server:", e);
      return { docker: false, git: false, gh: false, node: false };
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<DetectedTools>('detect_system_tools')
}

export async function sendChatMessage(
  message: string,
  imageDataUri?: string | null,
  audioDataUri?: string | null,
  documentAttachment?: DocumentAttachment | null,
  workspacePath?: string | null,
  chatId?: string | null,
  agentId?: string | null,
): Promise<ChatResponse> {
  const outgoingMessage = withImagePlaceholder(message, imageDataUri)
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await fetch(`${API_BASE}/chat`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: outgoingMessage, systemInstruction: '', chatId, imageDataUri, audioDataUri, documentAttachment, agentId })
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
    request: { message: outgoingMessage, systemInstruction: '', chatId, imageDataUri, audioDataUri, documentAttachment, workspacePath, agentId },
  })
  if (imageDataUri) {
    await invoke('save_pictures', {
      images: imageDataUri.split(' '),
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
  systemInstruction = '',
  onProgress?: (progress: AgentProgress) => void,
  documentAttachment?: DocumentAttachment | null,
  workspacePath?: string | null,
  chatId?: string | null,
  agentId?: string | null,
): Promise<ChatResponse> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    const outgoingMessage = withImagePlaceholder(message, imageDataUri);
    const res = await fetch(`${API_BASE}/chat-stream`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: outgoingMessage, systemInstruction, chatId, imageDataUri, audioDataUri, documentAttachment, agentId })
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
  const outgoingMessage = withImagePlaceholder(message, imageDataUri)
  const onEvent = new Channel<DesktopStreamEvent>()
  onEvent.onmessage = (event) => {
    if (event.type === 'chunk') onChunk(event.chunk)
    else onProgress?.(event.progress)
  }
  const response = await invoke<ChatResponse>('stream_chat_message', {
    request: { message: outgoingMessage, systemInstruction, chatId, imageDataUri, audioDataUri, documentAttachment, workspacePath, agentId },
    onEvent,
  })
  if (imageDataUri) {
    await invoke('save_pictures', {
      images: imageDataUri.split(' '),
      source: 'chat',
      message: outgoingMessage,
    })
  }
  return response
}

function withImagePlaceholder(message: string, imageDataUri?: string | null) {
  if (!imageDataUri || message.includes('[Image #1]')) return message
  const imageCount = imageDataUri.split(/\s+/).filter(Boolean).length
  const markers = Array.from({ length: imageCount }, (_, index) => `[Image #${index + 1}]`).join(' ')
  return markers ? `${message} ${markers}` : message
}

export async function getTtsUrls(text: string): Promise<TtsUrl[]> {
  if (typeof window === 'undefined') return []
  if (!isTauriRuntime()) return []
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<TtsUrl[]>('get_tts_urls', { text })
}

export async function cancelChatMessage(chatId: string): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    await fetch(`${API_BASE}/cancel-chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ chatId })
    });
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('cancel_chat_message', { chatId })
}

export async function getRecentInteractions(limit = 50, chatId?: string | null): Promise<InteractionMemory[]> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const params = new URLSearchParams({ limit: String(limit) });
      if (chatId) params.set('chatId', chatId);
      const res = await fetch(`${API_BASE}/interactions?${params.toString()}`);
      return await res.json();
    } catch (e) {
      console.error("Failed to fetch chat history from local server:", e);
      return [];
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<InteractionMemory[]>('get_recent_interactions', { limit, chatId })
}

export async function saveSystemInteraction(
  chatId: string,
  userText: string,
  provider: string,
  model: string,
): Promise<any> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await fetch(`${API_BASE}/interactions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ chatId, userText, provider, model }),
      });
      return await res.json();
    } catch (e) {
      console.error("Failed to save system interaction on local server:", e);
      return { success: false };
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke('save_system_interaction', { chatId, userText, provider, model })
}

export async function saveInteractionAgentActivity(
  interactionId: number,
  activity: AgentProgress[],
): Promise<void> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase();
    try {
      const res = await fetch(`${API_BASE}/interactions/agent-activity`, {
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
      const res = await fetch(`${API_BASE}/chat-sessions`);
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
      const res = await fetch(`${API_BASE}/chat-sessions/delete?${params.toString()}`, { method: 'POST' });
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
      const res = await fetch(`${API_BASE}/chat-sessions/rename`, {
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
      const res = await fetch(`${API_BASE}/profile?${params.toString()}`);
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
      const res = await fetch(`${API_BASE}/profile`, {
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
      const res = await fetch(`${API_BASE}/interactions/clear${suffix}`, { method: 'POST' });
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
      const res = await fetch(`${API_BASE}/pictures`);
      const pictures = await res.json();
      return Array.isArray(pictures)
        ? pictures.map((picture) => {
            const pictureUrl = picture.url ? `${API_BASE.replace('/api', '')}${picture.url}` : undefined
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

export async function generateImages(
  request: ImageGenRequest
): Promise<ImageGenResponse> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    const API_BASE = getApiBase()
    const res = await fetch(`${API_BASE}/image-generate`, {
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
      const res = await fetch(`${API_BASE}/image-gen/providers`)
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
      const getRes = await fetch(`${API_BASE}/config`)
      if (!getRes.ok) return false
      const config = await getRes.json()
      config.image_gen_provider = provider
      const saveRes = await fetch(`${API_BASE}/config`, {
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
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    return filePath;
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
          const res = await fetch(`${API_BASE}/config`);
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
          const res = await fetch(`${API_BASE}/config`, {
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
      openCustomWorkflows: async () => ({ success: false, message: 'Opening workflow files is only available in the desktop app.' }),
      reloadCustomWorkflows: async () => ({ success: false, message: 'Reloading workflow files is only available in the desktop app.' }),
      saveCustomWorkflows: async () => ({ success: false, message: 'Saving workflow files is only available in the desktop app.' }),
    };

    (window as any).spotlightAPI = {
      submit: () => {},
      executeAction: async (action: any) => {
        try {
          const res = await fetch(`${API_BASE}/action`, {
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
          const res = await fetch(`${API_BASE}/config`);
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
          const res = await fetch(`${API_BASE}/chat`, {
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
          const res = await fetch(`${API_BASE}/interactions/clear`, { method: 'POST' });
          const data = await res.json();
          return data.status === 'ok' ? 1 : 0;
        } catch (e) {
          return 0;
        }
      },
      getChatHistory: async () => {
        try {
          const res = await fetch(`${API_BASE}/interactions`);
          return await res.json();
        } catch (e) {
          console.error("Failed to fetch chat history from local server:", e);
          return [];
        }
      },
      listSavedPictures,
      openSettings: () => {
        window.location.hash = '#/settings';
      },
      openWorkflows: () => {
        window.location.hash = '#/workflows';
      },
      readClipboard: async () => '',
      writeClipboard: async () => {},
      getSystemInfo: async () => {
        try {
          const res = await fetch(`${API_BASE}/status`);
          return await res.json();
        } catch (e) {
          return { backend: 'browser-fallback' };
        }
      },
      getWeather: async (city: string) => {
        try {
          const res = await fetch(`${API_BASE}/weather?city=${encodeURIComponent(city)}`);
          return await res.json();
        } catch (e) {
          return { error: String(e) };
        }
      },
      getSettings: async () => {
        try {
          const res = await fetch(`${API_BASE}/config`);
          return await res.json();
        } catch (e) {
          return {};
        }
      },
      saveSettings: async (config: any) => {
        try {
          const res = await fetch(`${API_BASE}/config`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(config)
          });
          return await res.json();
        } catch (e) {
          return {};
        }
      },
      onSettingsChanged: () => {},
      startVision: () => {},
      onVisionReady: async () => () => {},
      captureSilentScreen: async () => '',
      getSmartContext: async () => {
        try {
          const res = await fetch(`${API_BASE}/smart-context`);
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
          const res = await fetch(`${API_BASE}/action`, {
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
          const res = await fetch(`${API_BASE}/action`, {
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
      notifyAiResponse: () => {},
      clearAiNotifications: () => {},
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
    openCustomWorkflows: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('open_workflows_file')
    },
    reloadCustomWorkflows: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('reload_custom_workflows')
    },
    saveCustomWorkflows: async (workflows) => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('save_custom_workflows', { workflows })
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
    sendMessage: (message, imageDataUri, audioDataUri, documentAttachment) => sendChatMessage(message, imageDataUri, audioDataUri, documentAttachment),
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
    openSettings: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('open_window', { kind: 'settings' })
    },
    openWorkflows: async () => {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('open_window', { kind: 'workflows' })
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
    notifyAiResponse: () => {},
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
      const val = await getProfileValue('learned-skills-web-mock')
      if (val) {
        return JSON.parse(val)
      }
      return []
    } catch (e) {
      console.error("Failed to load web mock skills:", e)
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

// Enforce compile-time check against the shared platform interface
const _apiCheck: MintPlatformApi = {
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
  clearChatHistory,
  listSavedPictures,
  generateImages,
  getImageGenProviders,
  setDefaultImageProvider,
  getWorkspaceTree,
  createWorkspaceFile,
  createWorkspaceFolder,
  deleteWorkspaceItem,
  selectWorkspaceDirectory,
  submitToolApproval,
  proposeCodeEdits,
  applyCodeEdits,
  listen,
  readClipboardImage,
}

