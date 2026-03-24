const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('widgetAPI', {
    onStateChange: (callback) => ipcRenderer.on('widget-state', (event, state) => callback(state))
});
