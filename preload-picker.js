const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronPicker', {
    onScreenshot: (callback) => ipcRenderer.on('screenshot-data', (_, data) => callback(data)),
    sendSelection: (base64Image) => ipcRenderer.send('vision-selection', base64Image),
    startContinuousTranslation: (rect) => ipcRenderer.send('vision-translate-start', rect),
    stopContinuousTranslation: () => ipcRenderer.send('vision-translate-stop'),
    onTranslationResult: (callback) => ipcRenderer.on('vision-translate-result', (_, text) => callback(text)),
    closePicker: () => ipcRenderer.send('vision-cancel'),
    setOverlayInteractable: (isInteractable) => ipcRenderer.send('vision-overlay-interactable', isInteractable)
});
