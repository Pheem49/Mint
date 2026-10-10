import { DEFAULT_NOTIFICATION_SETTINGS } from '../utils/notificationSettings'
/**
 * shared/constants/config.ts
 * Canonical DEFAULT_CONFIG — single source of truth for Desktop UI and Web UI settings.
 */

export const DEFAULT_CONFIG = {
  notificationSettings: { ...DEFAULT_NOTIFICATION_SETTINGS },
  theme: 'dark',
  tuiTheme: 'dark' as 'system' | 'dark' | 'light',
  accentColor: '#10b981',
  systemTextColor: '#f8fafc',
  chatTextColor: '#f8fafc',
  customBgStart: '#0f172a',
  customBgEnd: '#1e1b4b',
  customPanelBg: '#1e293b',
  surfaceStyle: 'opaque' as 'opaque' | 'glass',
  glassBlur: 'none',
  reducedEffects: false,
  fontFamily: "'Prompt', sans-serif",
  fontSize: '16px',
  typographyScaleVersion: 2,
  apiKey: '',
  aiProvider: 'gemini',
  geminiModel: 'gemini-2.5-flash' as string,
  openaiModel: 'gpt-5.6-luna' as string,
  openrouterModel: 'openai/gpt-5.6-terra' as string,
  deepseekModel: 'deepseek-v4-flash' as string,
  anthropicModel: 'claude-sonnet-5' as string,
  ollamaModel: 'llama3:latest' as string,
  temperature: null as number | null,
  modelTemperatures: {} as Record<string, number>,
  thinkingEnabled: true,
  thinkingEffort: 'medium' as 'low' | 'medium' | 'high' | 'extra_high',
  modelThinkingConfigs: {} as Record<string, { enabled: boolean; effort: 'low' | 'medium' | 'high' | 'extra_high' }>,
  language: 'th-TH',
  proactiveInterval: 60,
  proactiveCooldown: 120,
  enableVoiceReply: true,
  enableAgentCollaboration: false,
  autoSkillWriting: false,
  ttsProvider: 'google',
  ttsVolume: 1.0,
  ttsSpeed: 1.0,
  ttsPitch: 1.0,
  voiceMode: 'legacy',
  geminiLiveModel: 'gemini-2.5-flash-native-audio-preview-12-2025',
  geminiLiveVoice: 'Puck',
  pluginSpotifyEnabled: false,
  pluginCalendarEnabled: false,
  pluginGmailEnabled: false,
  pluginNotionEnabled: false,
  pluginDiscordEnabled: false,
  showDesktopWidget: true,
  mcpServers: {} as Record<string, any>,
  hfModel: 'Qwen/Qwen3.6-27B' as string,
  localApiBaseUrl: '',
  localModelName: 'local-model' as string,
  ollamaHost: '',
  anthropicApiKey: '',
  openaiApiKey: '',
  openrouterApiKey: '',
  deepseekApiKey: '',
  hfApiKey: '',
  bflApiKey: '',
  bflModel: 'flux-pro-1.1',
  automationBrowser: 'chromium',
  browserDebugUrl: 'http://127.0.0.1:9222/json/list',
  browserExtensionContextUrl: 'http://127.0.0.1:3212/context',
  enableHeadlessTaskQueue: false,
  enableAutoUpdate: false,
  updaterEndpoint: '',
  updaterPublicKey: '',
  telegramBotToken: '',
  enableTelegramBridge: false,
  discordBotToken: '',
  discordApplicationId: '',
  enableDiscordBridge: false,
  slackBotToken: '',
  slackAppToken: '',
  enableSlackBridge: false,
  lineChannelAccessToken: '',
  lineChannelSecret: '',
  enableLineBridge: false,
  lineWebhookHost: '127.0.0.1',
  lineWebhookPort: 3000,
  whatsappCloudAccessToken: '',
  whatsappPhoneNumberId: '',
  whatsappVerifyToken: '',
  whatsappAppSecret: '',
  enableWhatsappBridge: false,
  whatsappWebhookHost: '127.0.0.1',
  whatsappWebhookPort: 3001,
  enableBridgeAckNotification: true,
  bridgeAckMessage: '[Mint Agent] Remote command received, processing...',
  notionApiKey: '',
  notionDatabaseId: '',
  gmailClientId: '',
  gmailClientSecret: '',
  spotifyClientId: '',
  spotifyClientSecret: '',
  googleCalendarClientId: '',
  googleCalendarClientSecret: '',

  // Search Engine Settings
  searchProvider: 'google' as string,
  braveSearchApiKey: '',
  googleSearchApiKey: '',
  googleSearchCx: '',
  searxngBaseUrl: '',
  
  // Image Generation Settings
  imageGenProvider: 'gemini' as 'gemini' | 'dalle' | 'stability' | 'ideogram' | 'replicate' | 'bfl',
  stabilityApiKey: '',
  ideogramApiKey: '',
  replicateApiKey: '',
  // Default model per image provider (used when a generation request doesn't
  // override it). `bflModel` lives with the other bfl* fields above.
  nanobananaModel: 'gemini-3.1-flash-image',
  dalleModel: 'gpt-image-1',
  stabilityModel: 'ultra',
  ideogramModel: 'V_3',
  replicateModel: 'black-forest-labs/flux-1.1-pro',

  // Video Generation Settings
  videoGenProvider: 'veo' as 'veo',
  veoModel: 'veo-3.1-generate-preview' as string,

  // Multi-Agent Configuration
  agents: [
    {
      id: 'planner',
      name: 'Planner',
      provider: 'gemini',
      model: 'gemini-2.5-flash',
      apiKey: '',
      systemPrompt: 'You are the Lead Planning Agent. Break down user goals into clear executable steps.',
      systemInstruction: 'You are the Lead Planning Agent. Break down user goals into clear executable steps.',
      enabled: true,
      avatar: '🎯',
    },
    {
      id: 'coder',
      name: 'Coder',
      provider: 'openai',
      model: 'gpt-5.6-luna',
      apiKey: '',
      systemPrompt: 'You are the Senior Software Engineer. Implement features with clean code and tests.',
      systemInstruction: 'You are the Senior Software Engineer. Implement features with clean code and tests.',
      enabled: true,
      avatar: '💻',
    },
    {
      id: 'critic',
      name: 'Critic',
      provider: 'anthropic',
      model: 'claude-sonnet-5',
      apiKey: '',
      systemPrompt: 'You are the Security & Quality Auditor. Review code, find bugs, and suggest improvements.',
      systemInstruction: 'You are the Security & Quality Auditor. Review code, find bugs, and suggest improvements.',
      enabled: true,
      avatar: '🔍',
    },
  ],

  // Custom AI Providers
  customProviders: [] as any[],

  // Custom Model Selections per Provider
  customModelSelections: {} as Record<string, string>,
}

export const migrateTuiTheme = <T extends Record<string, any>>(config: T): T & { tuiTheme: 'system' | 'dark' | 'light' } => {
  const legacyTheme = config.tuiTheme || config.theme
  const tuiTheme = legacyTheme === 'system' || legacyTheme === 'light' ? legacyTheme : 'dark'
  return { ...config, tuiTheme }
}

/**
 * Version 1 used 18px as the implicit default. Move that old default to the
 * denser 16px scale once, while preserving any size chosen after this update.
 */
export function migrateTypographyScale<T extends Record<string, any> | null | undefined>(config: T): T {
  if (!config || config.typographyScaleVersion === 2) return config
  return {
    ...config,
    fontSize: config.fontSize === '18px' ? '16px' : config.fontSize,
    typographyScaleVersion: 2,
  } as T
}
