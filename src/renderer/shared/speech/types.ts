export type SpeechRequest =
  | { operation: 'prefer'; owner: string; chatId: string; enabled: boolean }
  | { operation: 'prepare'; owner: string; chatId: string; turnId?: number; text?: string; automatic?: boolean; audioOnly?: boolean; routeOwner?: string }
  | { operation: 'audio'; owner: string; index: number }
  | { operation: 'renew' | 'release'; owner: string }
export type SpeechReply =
  | { kind: 'prepared'; text: string; provider: string; volume: number; speed: number; pitch: number; chunks: number }
  | { kind: 'audio'; data: string }
  | { kind: 'lease'; active: boolean }
  | { kind: 'released' }
export type SpeechTransport = (request: SpeechRequest) => Promise<SpeechReply>
