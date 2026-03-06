const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('settingsApi', {
    getSettings: () => ipcRenderer.invoke('get-settings'),
    saveSettings: (config) => ipcRenderer.invoke('save-settings', config),
    closeSettings: () => ipcRenderer.send('close-settings'),
    openExternal: (url) => ipcRenderer.invoke('open-external', url),
});
