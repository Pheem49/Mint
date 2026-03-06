const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('api', {
    sendMessage: (message) => ipcRenderer.invoke('chat-message', message),
    closeWindow: () => ipcRenderer.send('close-window'),
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
});
