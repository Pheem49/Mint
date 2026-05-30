interface Window {
  api: {
    sendMessage: (message: string, base64Image?: string | null, base64Audio?: string | null) => Promise<any>;
    closeWindow: () => void;
    minimizeWindow: () => void;
    quitApp: () => void;
    maximizeWindow: () => void;
    resetChat: () => Promise<any>;
    getChatHistory: () => Promise<any>;
    listSavedPictures: () => Promise<any>;
    openSettings: () => Promise<any>;
    readClipboard: () => Promise<string>;
    writeClipboard: (text: string) => Promise<void>;
    getSystemInfo: () => Promise<any>;
    getWeather: (city: string) => Promise<any>;
    getSettings: () => Promise<any>;
    saveSettings: (config: any) => Promise<any>;
    onSettingsChanged: (callback: (data: any) => void) => void;
    startVision: () => Promise<any>;
    onVisionReady: (callback: (data: string) => void) => void;
    captureSilentScreen: () => Promise<string | null>;
    getSmartContext: () => Promise<any>;
    onProactiveSuggestion: (callback: (data: any) => void) => void;
    onProactiveNotification: (callback: (data: any) => void) => void;
    toggleProactive: (isOn: boolean) => void;
    recordBehavior: (context: any) => void;
    executeProactiveAction: (action: any) => Promise<any>;
    executeApprovedAction: (action: any) => Promise<any>;
    onSpotlightToChat: (callback: (query: string) => void) => void;
    notifyAiResponse: () => void;
    clearAiNotifications: () => void;
    getTtsUrls: (text: string) => Promise<string[]>;
    setAiState: (state: string) => void;
  };
  settingsApi: {
    getSettings: () => Promise<any>;
    saveSettings: (config: any) => Promise<any>;
    closeSettings: () => void;
    quitApp: () => void;
    openExternal: (url: string) => Promise<void>;
    openCustomWorkflows: () => Promise<any>;
    reloadCustomWorkflows: () => Promise<any>;
  };
  spotlightAPI: {
    submit: (query: string) => void;
    executeAction: (action: any) => Promise<any>;
    close: () => void;
    hide: () => void;
    resize: (width: number, height: number) => void;
    getSettings: () => Promise<any>;
    onSettingsChanged: (callback: (config: any) => void) => void;
  };
  electronPicker: {
    onScreenshot: (callback: (data: string) => void) => void;
    sendSelection: (base64Image: string) => void;
    startContinuousTranslation: (rect: any) => void;
    stopContinuousTranslation: () => void;
    onTranslationResult: (callback: (text: string) => void) => void;
    closePicker: () => void;
    setOverlayInteractable: (isInteractable: boolean) => void;
  };
  widgetAPI: {
    onStateChange: (callback: (state: any) => void) => void;
  };
  PIXI: any;
  Live2DManager: any;
}

interface SpeechRecognition extends EventTarget {
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  onstart: ((this: SpeechRecognition, ev: Event) => any) | null;
  onresult: ((this: SpeechRecognition, ev: SpeechRecognitionEvent) => any) | null;
  onerror: ((this: SpeechRecognition, ev: Event) => any) | null;
  onend: ((this: SpeechRecognition, ev: Event) => any) | null;
  start(): void;
  stop(): void;
  abort(): void;
}

interface SpeechRecognitionEvent extends Event {
  readonly resultIndex: number;
  readonly results: SpeechRecognitionResultList;
}

interface SpeechRecognitionResultList {
  readonly length: number;
  item(index: number): SpeechRecognitionResult;
  [index: number]: SpeechRecognitionResult;
}

interface SpeechRecognitionResult {
  readonly length: number;
  item(index: number): SpeechRecognitionAlternative;
  [index: number]: SpeechRecognitionAlternative;
  readonly isFinal: boolean;
}

interface SpeechRecognitionAlternative {
  readonly transcript: string;
  readonly confidence: number;
}


