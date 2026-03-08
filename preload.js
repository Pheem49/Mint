const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('api', {
    sendMessage: (message, base64Image) => ipcRenderer.invoke('chat-message', message, base64Image),
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
    onSettingsChanged: (callback) => ipcRenderer.on('settings-changed', (_event, config) => callback(config)),
    // Vision
    startVision: () => ipcRenderer.invoke('start-screen-capture'),
    onVisionReady: (callback) => ipcRenderer.on('vision-ready', (_event, image) => callback(image)),
    captureSilentScreen: () => ipcRenderer.invoke('capture-silent-screen'),
    // Proactive Assistant
    onProactiveSuggestion: (callback) => ipcRenderer.on('proactive-suggestion', (_event, data) => callback(data)),
    toggleProactive: (isOn) => ipcRenderer.send('toggle-proactive', isOn),
    recordBehavior: (context) => ipcRenderer.send('record-behavior', context),
    executeProactiveAction: (action) => ipcRenderer.invoke('execute-proactive-action', action)
});
