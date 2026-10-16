import type { ConversationSources, SourceAttachment } from '../types'

export function createSourcesTransport(environment: {
  isNative(): boolean; apiBase(): string; fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>
}) {
  return {
    async getConversationSources(chatId: string): Promise<ConversationSources> {
      if (environment.isNative()) {
        const { invoke } = await import('@tauri-apps/api/core')
        return invoke('get_conversation_sources', { chatId })
      }
      const response = await environment.fetch(`${environment.apiBase()}/conversation-sources?${new URLSearchParams({ chatId })}`)
      if (!response.ok) throw new Error(`Session sources unavailable (${response.status})`)
      return response.json()
    },
    async saveSourceAttachments(chatId: string, interactionId: number, attachments: SourceAttachment[]): Promise<void> {
      if (environment.isNative()) {
        const { invoke } = await import('@tauri-apps/api/core')
        return invoke('save_source_attachments', { chatId, interactionId, attachments })
      }
      const response = await environment.fetch(`${environment.apiBase()}/conversation-sources`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ chatId, interactionId, attachments }),
      })
      if (!response.ok) throw new Error(`Could not save session sources (${response.status})`)
    },
  }
}
