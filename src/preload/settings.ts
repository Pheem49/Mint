import { contextBridge, ipcRenderer } from 'electron'

contextBridge.exposeInMainWorld('settingsApi', {
  getSettings: () => ipcRenderer.invoke('get-settings'),
  saveSettings: (config: any) => ipcRenderer.invoke('save-settings', config),
  closeSettings: () => ipcRenderer.send('close-settings'),
  quitApp: () => ipcRenderer.send('quit-app'),
  openExternal: (url: string) => ipcRenderer.invoke('open-external', url),
  openCustomWorkflows: () => ipcRenderer.invoke('open-custom-workflows'),
  reloadCustomWorkflows: () => ipcRenderer.invoke('reload-custom-workflows')
})
