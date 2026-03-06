const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('api', {
    sendMessage: (message) => ipcRenderer.invoke('chat-message', message),
    closeWindow: () => ipcRenderer.send('close-window')
});
