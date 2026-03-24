const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('api', {
    sendMessage: (message, base64Image, base64Audio) => ipcRenderer.invoke('chat-message', message, base64Image, base64Audio),
    closeWindow: () => ipcRenderer.send('close-window'),
    maximizeWindow: () => ipcRenderer.send('maximize-window'),
    resetChat: () => ipcRenderer.invoke('reset-chat'),
    getChatHistory: () => ipcRenderer.invoke('get-chat-history'),
    openSettings: () => ipcRenderer.invoke('open-settings'),
    // Clipboard
    readClipboard: () => ipcRenderer.invoke('clipboard-read'),
    writeClipboard: (text) => ipcRenderer.invoke('clipboard-write', text),
    // System Info
    getSystemInfo: () => ipcRenderer.invoke('get-system-info'),
    getWeather: (city) => ipcRenderer.invoke('get-weather', city),
    // Settings
    getSettings: () => ipcRenderer.invoke('get-settings'),
    // Listen for settings changes from other window
    onSettingsChanged: (callback) => ipcRenderer.on('settings-changed', (event, data) => callback(data)),
    // Vision
    startVision: () => ipcRenderer.invoke('start-screen-capture'),
    onVisionReady: (callback) => ipcRenderer.on('vision-ready', (event, data) => callback(data)),
    captureSilentScreen: () => ipcRenderer.invoke('capture-silent-screen'),
    // Proactive Assistant
    onProactiveSuggestion: (callback) => ipcRenderer.on('proactive-suggestion', (event, data) => callback(data)),
    onProactiveNotification: (callback) => ipcRenderer.on('proactive-notification', (event, data) => callback(data)),
    toggleProactive: (isOn) => ipcRenderer.send('toggle-proactive', isOn),
    recordBehavior: (context) => ipcRenderer.send('record-behavior', context),
    executeProactiveAction: (action) => ipcRenderer.invoke('execute-proactive-action', action),
    onSpotlightToChat: (callback) => ipcRenderer.on('spotlight-to-chat', (_event, query) => callback(query)),
    notifyAiResponse: () => ipcRenderer.send('ai-notify'),
    clearAiNotifications: () => ipcRenderer.send('ai-notify-clear'),
    getTtsUrls: (text) => ipcRenderer.invoke('get-tts-urls', text),
    setAiState: (state) => ipcRenderer.send('set-ai-state', state)
});
