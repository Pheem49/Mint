import { contextBridge, ipcRenderer } from 'electron'

contextBridge.exposeInMainWorld('api', {
  sendMessage: (message: string, base64Image?: string | null, base64Audio?: string | null) =>
    ipcRenderer.invoke('chat-message', message, base64Image, base64Audio),
  closeWindow: () => ipcRenderer.send('close-window'),
  minimizeWindow: () => ipcRenderer.send('minimize-window'),
  quitApp: () => ipcRenderer.send('quit-app'),
  maximizeWindow: () => ipcRenderer.send('maximize-window'),
  resetChat: () => ipcRenderer.invoke('reset-chat'),
  getChatHistory: () => ipcRenderer.invoke('get-chat-history'),
  listSavedPictures: () => ipcRenderer.invoke('list-saved-pictures'),
  openSettings: () => ipcRenderer.invoke('open-settings'),
  // Clipboard
  readClipboard: () => ipcRenderer.invoke('clipboard-read'),
  writeClipboard: (text: string) => ipcRenderer.invoke('clipboard-write', text),
  // System Info
  getSystemInfo: () => ipcRenderer.invoke('get-system-info'),
  getWeather: (city: string) => ipcRenderer.invoke('get-weather', city),
  // Settings
  getSettings: () => ipcRenderer.invoke('get-settings'),
  saveSettings: (config: any) => ipcRenderer.invoke('save-settings', config),
  // Listen for settings changes from other window
  onSettingsChanged: (callback: (data: any) => void) =>
    ipcRenderer.on('settings-changed', (_, data) => callback(data)),
  // Vision
  startVision: () => ipcRenderer.invoke('start-screen-capture'),
  onVisionReady: (callback: (data: string) => void) =>
    ipcRenderer.on('vision-ready', (_, data) => callback(data)),
  captureSilentScreen: () => ipcRenderer.invoke('capture-silent-screen'),
  getSmartContext: () => ipcRenderer.invoke('get-smart-context'),
  // Proactive Assistant
  onProactiveSuggestion: (callback: (data: any) => void) =>
    ipcRenderer.on('proactive-suggestion', (_, data) => callback(data)),
  onProactiveNotification: (callback: (data: any) => void) =>
    ipcRenderer.on('proactive-notification', (_, data) => callback(data)),
  toggleProactive: (isOn: boolean) => ipcRenderer.send('toggle-proactive', isOn),
  recordBehavior: (context: any) => ipcRenderer.send('record-behavior', context),
  executeProactiveAction: (action: any) => ipcRenderer.invoke('execute-proactive-action', action),
  executeApprovedAction: (action: any) => ipcRenderer.invoke('execute-approved-action', action),
  onSpotlightToChat: (callback: (query: string) => void) =>
    ipcRenderer.on('spotlight-to-chat', (_, query) => callback(query)),
  notifyAiResponse: () => ipcRenderer.send('ai-notify'),
  clearAiNotifications: () => ipcRenderer.send('ai-notify-clear'),
  getTtsUrls: (text: string) => ipcRenderer.invoke('get-tts-urls', text),
  setAiState: (state: string) => ipcRenderer.send('set-ai-state', state)
})
