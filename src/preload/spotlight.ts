import { contextBridge, ipcRenderer } from 'electron'

contextBridge.exposeInMainWorld('spotlightAPI', {
  submit: (query: string) => ipcRenderer.send('spotlight-submit', query),
  executeAction: (action: any) => ipcRenderer.invoke('spotlight-action', action),
  close: () => ipcRenderer.send('spotlight-close'),
  hide: () => ipcRenderer.send('spotlight-hide'),
  resize: (width: number, height: number) => ipcRenderer.send('spotlight-resize', width, height),
  getSettings: () => ipcRenderer.invoke('get-settings'),
  onSettingsChanged: (callback: (config: any) => void) =>
    ipcRenderer.on('settings-changed', (_, config) => callback(config))
})
