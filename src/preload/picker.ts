import { contextBridge, ipcRenderer } from 'electron'

contextBridge.exposeInMainWorld('electronPicker', {
  onScreenshot: (callback: (data: string) => void) =>
    ipcRenderer.on('screenshot-data', (_, data) => callback(data)),
  sendSelection: (base64Image: string) => ipcRenderer.send('vision-selection', base64Image),
  startContinuousTranslation: (rect: any) => ipcRenderer.send('vision-translate-start', rect),
  stopContinuousTranslation: () => ipcRenderer.send('vision-translate-stop'),
  onTranslationResult: (callback: (text: string) => void) =>
    ipcRenderer.on('vision-translate-result', (_, text) => callback(text)),
  closePicker: () => ipcRenderer.send('vision-cancel'),
  setOverlayInteractable: (isInteractable: boolean) =>
    ipcRenderer.send('vision-overlay-interactable', isInteractable)
})
