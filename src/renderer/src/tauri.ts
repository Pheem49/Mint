import { Channel, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

export interface RuntimeStatus {
  backend: string
  configPath: string
  activeProvider: string
  availableProviders: string[]
  integrations: Record<string, unknown>
}

export interface ChatResponse {
  provider: string
  model: string
  text: string
}

export type AgentProgress =
  | { type: 'Thinking'; data: { elapsed_secs: number } }
  | { type: 'Thought'; data: { thought: string } }
  | { type: 'ToolStart'; data: { action: string; input: Record<string, unknown> } }
  | { type: 'ToolEnd'; data: { action: string; input: Record<string, unknown>; result: string } }

type DesktopStreamEvent =
  | { type: 'chunk'; chunk: string }
  | { type: 'progress'; progress: AgentProgress }

export interface InteractionMemory {
  id: number
  userText: string
  aiText: string
  provider: string
  model: string
  createdAt: string
}

export interface PictureEntry {
  id: string
  filename: string
  path: string
  mimeType: string
  createdAt: string
  source: string
  message: string
  url?: string
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

export async function getRuntimeStatus(): Promise<RuntimeStatus> {
  return invoke<RuntimeStatus>('get_runtime_status')
}

export async function sendChatMessage(
  message: string,
  imageDataUri?: string | null,
  audioDataUri?: string | null,
): Promise<ChatResponse> {
  const outgoingMessage = withImagePlaceholder(message, imageDataUri)
  const response = await invoke<ChatResponse>('send_chat_message', {
    request: { message: outgoingMessage, systemInstruction: '', imageDataUri, audioDataUri },
  })
  if (imageDataUri) {
    await invoke('save_pictures', {
      images: [imageDataUri],
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
): Promise<ChatResponse> {
  const outgoingMessage = withImagePlaceholder(message, imageDataUri)
  const onEvent = new Channel<DesktopStreamEvent>()
  onEvent.onmessage = (event) => {
    if (event.type === 'chunk') onChunk(event.chunk)
    else onProgress?.(event.progress)
  }
  const response = await invoke<ChatResponse>('stream_chat_message', {
    request: { message: outgoingMessage, systemInstruction, imageDataUri, audioDataUri },
    onEvent,
  })
  if (imageDataUri) {
    await invoke('save_pictures', {
      images: [imageDataUri],
      source: 'chat',
      message: outgoingMessage,
    })
  }
  return response
}

function withImagePlaceholder(message: string, imageDataUri?: string | null) {
  return imageDataUri && !message.includes('[Image #1]') ? `${message} [Image #1]` : message
}

export async function getRecentInteractions(limit = 50): Promise<InteractionMemory[]> {
  return invoke<InteractionMemory[]>('get_recent_interactions', { limit })
}

export async function clearChatHistory(): Promise<number> {
  return invoke<number>('clear_chat_history')
}

export async function listSavedPictures(): Promise<PictureEntry[]> {
  return invoke<PictureEntry[]>('list_pictures')
}

export async function submitToolApproval(token: string, approved: boolean): Promise<void> {
  return invoke('submit_tool_approval', { token, approved })
}

export async function proposeCodeEdits(root: string, edits: CodeEdit[]): Promise<CodeEditProposal> {
  return invoke<CodeEditProposal>('propose_desktop_code_edits', { root, edits })
}

export async function applyCodeEdits(root: string, edits: CodeEdit[], approvalToken: string) {
  return invoke('apply_desktop_code_edits', { root, edits, approvalToken })
}

export function installTauriAdapters() {
  if (typeof window === 'undefined' || !(window as any).__TAURI_INTERNALS__) {
    console.warn("Not running inside Tauri. Connecting to local API server fallback at http://localhost:3000/api.");
    const API_BASE = "http://localhost:3000/api";

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
      getUpdaterStatus: async () => ({}),
      checkForUpdates: async () => ({}),
      installAvailableUpdate: async () => {},
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
      closeSettings: () => {},
      quitApp: () => {},
      openExternal: () => {},
      openCustomWorkflows: () => {},
      reloadCustomWorkflows: () => {},
    };

    (window as any).spotlightAPI = {
      submit: () => {},
      executeAction: async () => ({ success: true }),
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
      sendMessage: async (message: string, imageDataUri?: string | null, audioDataUri?: string | null) => {
        try {
          const res = await fetch(`${API_BASE}/chat`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ message, imageDataUri, audioDataUri })
          });
          return await res.json();
        } catch (e) {
          console.error("Failed to send message to local server:", e);
          return { provider: 'error', model: 'error', text: `Failed to connect to Local API Server: ${e}` };
        }
      },
      closeWindow: () => {},
      minimizeWindow: () => {},
      quitApp: () => {},
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
      listSavedPictures: async () => [],
      openSettings: () => {},
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
      getWeather: async () => ({}),
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
      onVisionReady: () => {},
      captureSilentScreen: async () => '',
      getSmartContext: async () => ({}),
      onProactiveSuggestion: () => {},
      onProactiveNotification: () => {},
      toggleProactive: () => {},
      recordBehavior: () => {},
      executeProactiveAction: () => {},
      executeApprovedAction: () => {},
      onSpotlightToChat: () => {},
      notifyAiResponse: () => {},
      clearAiNotifications: () => {},
      getTtsUrls: async () => [],
      setAiState: () => {},
    };
    return;
  }

  const currentWindow = getCurrentWindow()
  let translationTimer: ReturnType<typeof setInterval> | null = null
  const close = () => invoke('close_desktop_window', { label: currentWindow.label })
  const hide = () => invoke('hide_desktop_window', { label: currentWindow.label })
  const settingsChanged = (callback: (config: any) => void) => {
    void listen<any>('settings-changed', (event) => callback(event.payload))
  }
  const executeAction = (action: any, approved = false) => action.type === 'plugin'
    ? invoke('run_native_plugin', { name: action.pluginName, instruction: action.target || '' })
    : invoke('run_desktop_action', { action: { ...action, approved } })

  window.settingsApi = {
    getSettings: () => invoke('get_config'),
    getUpdaterStatus: () => invoke('get_updater_status'),
    checkForUpdates: () => invoke('check_for_updates'),
    installAvailableUpdate: () => invoke('install_available_update', { approved: true }),
    saveSettings: (config) => invoke('update_config', { config }),
    closeSettings: close,
    quitApp: () => void invoke('exit_app'),
    openExternal: (url) => invoke('run_desktop_action', { action: { type: 'open_url', target: url } }),
    openCustomWorkflows: () => invoke('open_workflows_file'),
    reloadCustomWorkflows: () => invoke('reload_custom_workflows'),
  }

  window.spotlightAPI = {
    submit: (query) => void invoke('submit_spotlight', { query }),
    executeAction: async (action) => {
      if (action.type === 'clipboard_write') {
        await navigator.clipboard.writeText(action.target)
        return { success: true }
      }
      return invoke('run_desktop_action', { action })
    },
    close,
    hide,
    resize: (width, height) => void invoke('resize_desktop_window', {
      label: currentWindow.label,
      width,
      height,
    }),
    getSettings: () => invoke('get_config'),
    onSettingsChanged: settingsChanged,
  }

  window.widgetAPI = {
    onStateChange: (callback) => {
      void listen<string>('widget-state', (event) => callback(event.payload))
    },
  }

  window.screenPickerApi = {
    onScreenshot: (callback) => {
      void invoke<string>('capture_silent_screen').then(callback)
    },
    sendSelection: (image) => void invoke('submit_screen_selection', { image }),
    startContinuousTranslation: (rect) => {
      if (translationTimer) clearInterval(translationTimer)
      const translate = () => {
        void invoke<string>('translate_capture_region', { rect })
          .then((text) => window.dispatchEvent(new CustomEvent('mint-translation', { detail: text })))
          .catch((reason) => {
            window.dispatchEvent(new CustomEvent('mint-translation', { detail: String(reason) }))
          })
      }
      translate()
      translationTimer = setInterval(translate, 3000)
    },
    stopContinuousTranslation: () => {
      if (translationTimer) clearInterval(translationTimer)
      translationTimer = null
    },
    onTranslationResult: (callback) => {
      window.addEventListener('mint-translation', ((event: CustomEvent<string>) => {
        callback(event.detail)
      }) as EventListener)
    },
    closePicker: close,
    setOverlayInteractable: () => {},
  }

  window.api = {
    sendMessage: (message, imageDataUri, audioDataUri) => sendChatMessage(message, imageDataUri, audioDataUri),
    closeWindow: hide,
    minimizeWindow: () => void currentWindow.minimize(),
    quitApp: () => void invoke('exit_app'),
    maximizeWindow: () => void currentWindow.toggleMaximize(),
    resetChat: clearChatHistory,
    getChatHistory: () => getRecentInteractions(50),
    listSavedPictures,
    openSettings: () => invoke('open_window', { kind: 'settings' }),
    readClipboard: () => navigator.clipboard.readText(),
    writeClipboard: (text) => navigator.clipboard.writeText(text),
    getSystemInfo: async () => ({ backend: 'rust' }),
    getWeather: (city) => invoke('get_weather', { city }),
    getSettings: () => invoke('get_config'),
    saveSettings: (config) => invoke('update_config', { config }),
    onSettingsChanged: settingsChanged,
    startVision: () => invoke('start_screen_capture'),
    onVisionReady: (callback) => {
      void listen<string>('vision-ready', (event) => callback(event.payload))
    },
    captureSilentScreen: () => invoke('capture_silent_screen'),
    getSmartContext: () => invoke('get_smart_context'),
    onProactiveSuggestion: (callback) => {
      void listen<any>('proactive-suggestion', (event) => callback(event.payload))
    },
    onProactiveNotification: (callback) => {
      void listen<any>('proactive-notification', (event) => callback(event.payload))
    },
    toggleProactive: (enabled) => void invoke('toggle_proactive', { enabled }),
    recordBehavior: (context) => void invoke('save_behavior_context', { context }),
    executeProactiveAction: (action) => executeAction(action),
    executeApprovedAction: (action) => executeAction(action, true),
    onSpotlightToChat: (callback) => {
      void listen<string>('spotlight-to-chat', (event) => callback(event.payload))
    },
    notifyAiResponse: () => {},
    clearAiNotifications: () => {},
    getTtsUrls: (text) => invoke('get_tts_urls', { text }),
    setAiState: (state) => void invoke('set_ai_state', { state }),
  }
}
