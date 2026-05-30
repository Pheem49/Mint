import { contextBridge, ipcRenderer } from 'electron'

contextBridge.exposeInMainWorld('widgetAPI', {
  onStateChange: (callback: (state: any) => void) =>
    ipcRenderer.on('widget-state', (_, state) => callback(state))
})
