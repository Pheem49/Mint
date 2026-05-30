export {}

declare global {
  interface Window {
    api: {
      sendMessage: (message: string, base64Image?: string | null, base64Audio?: string | null) => Promise<any>
      closeWindow: () => void
      minimizeWindow: () => void
      quitApp: () => void
      maximizeWindow: () => void
      resetChat: () => Promise<any>
      getChatHistory: () => Promise<any[]>
      listSavedPictures: () => Promise<any[]>
      openSettings: () => Promise<any>
      readClipboard: () => Promise<string>
      writeClipboard: (text: string) => Promise<any>
      getSystemInfo: () => Promise<any>
      getWeather: (city: string) => Promise<any>
      getSettings: () => Promise<any>
      saveSettings: (config: any) => Promise<any>
      onSettingsChanged: (callback: (data: any) => void) => void
      startVision: () => Promise<any>
      onVisionReady: (callback: (data: string) => void) => void
      captureSilentScreen: () => Promise<string | null>
      getSmartContext: () => Promise<any>
      onProactiveSuggestion: (callback: (data: any) => void) => void
      onProactiveNotification: (callback: (data: any) => void) => void
      toggleProactive: (isOn: boolean) => void
      recordBehavior: (context: any) => void
      executeProactiveAction: (action: any) => Promise<any>
      executeApprovedAction: (action: any) => Promise<any>
      onSpotlightToChat: (callback: (query: string) => void) => void
      notifyAiResponse: () => void
      clearAiNotifications: () => void
      getTtsUrls: (text: string) => Promise<any[]>
      setAiState: (state: string) => void
    }
    settingsApi: {
      getSettings: () => Promise<any>
      saveSettings: (config: any) => Promise<any>
      closeSettings: () => void
      quitApp: () => void
      openExternal: (url: string) => Promise<any>
      openCustomWorkflows: () => Promise<any>
      reloadCustomWorkflows: () => Promise<any>
    }
    electronPicker: {
      onScreenshot: (callback: (data: string) => void) => void
      sendSelection: (base64Image: string) => void
      startContinuousTranslation: (rect: any) => void
      stopContinuousTranslation: () => void
      onTranslationResult: (callback: (text: string) => void) => void
      closePicker: () => void
      setOverlayInteractable: (isInteractable: boolean) => void
    }
    spotlightAPI: {
      submit: (query: string) => void
      executeAction: (action: any) => Promise<any>
      close: () => void
      hide: () => void
      resize: (width: number, height: number) => void
      getSettings: () => Promise<any>
      onSettingsChanged: (callback: (config: any) => void) => void
    }
    widgetAPI: {
      onStateChange: (callback: (state: any) => void) => void
    }
    Live2DManager?: any
    PIXI?: any
  }
}
