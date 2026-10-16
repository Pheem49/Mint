import type { MintPlatformApi } from '../platform'
import type { AgentProgress, ChatResponse, DocumentAttachment, AttachmentMetadata } from '../types'

type ConversationOperations = Pick<MintPlatformApi, 'sendChatMessage' | 'streamChatMessage'>
type TransportEnvironment = {
  isNative(): boolean
  apiBase(): string
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>
  ownerToken(): string | null
}

type StreamEvent =
  | { type: 'started'; interactionId: number }
  | { type: 'chunk'; chunk: string }
  | { type: 'progress'; progress: AgentProgress }
  | { type: 'approval-requested'; token: string; approval: any }
  | { type: 'done'; response: ChatResponse }

type StreamCallbacks = {
  onChunk(chunk: string): void
  onProgress?: (progress: AgentProgress) => void
  onApprovalRequested?: (payload: { token: string; approval: any }) => void
  onTurnStarted?: (interactionId: number) => void
}

function deliverEvent(event: StreamEvent, callbacks: StreamCallbacks): ChatResponse | undefined {
  switch (event?.type) {
    case 'chunk': callbacks.onChunk(event.chunk); break
    case 'started': callbacks.onTurnStarted?.(event.interactionId); break
    case 'progress': callbacks.onProgress?.(event.progress); break
    case 'approval-requested': callbacks.onApprovalRequested?.({ token: event.token, approval: event.approval }); break
    case 'done': return event.response
  }
}

async function readStream(response: Response, callbacks: StreamCallbacks): Promise<ChatResponse> {
  const reader = response.body?.getReader()
  if (!reader) throw new Error('No response body reader')
  const decoder = new TextDecoder()
  let buffer = ''
  let finalResponse: ChatResponse | undefined
  const consumeLine = (line: string) => {
    if (!line.trim()) return
    let event: StreamEvent
    try {
      event = JSON.parse(line)
    } catch (error) {
      console.error('Failed to parse stream line:', line, error)
      return
    }
    // Consumer failures are transport failures, not malformed JSON.
    const result = deliverEvent(event, callbacks)
    if (result) finalResponse = result
  }
  try {
    while (true) {
      const { value, done } = await reader.read()
      buffer += done ? decoder.decode() : decoder.decode(value, { stream: true })
      const lines = buffer.split('\n')
      buffer = lines.pop() || ''
      for (const line of lines) consumeLine(line)
      if (done) {
        consumeLine(buffer)
        break
      }
    }
    if (finalResponse) return finalResponse
    throw new Error('Stream closed without a final response')
  } catch (error) {
    await reader.cancel(error).catch(() => {})
    throw error
  } finally {
    reader.releaseLock()
  }
}

function withMediaPlaceholders(message: string, imageDataUri?: string | null, videoDataUri?: string | null) {
  for (const [kind, uris] of [['Image', imageDataUri], ['Video', videoDataUri]] as const) {
    if (uris && !message.includes(`[${kind} #1]`)) {
      const markers = uris.split(/\s+/).filter(Boolean).map((_, index) => `[${kind} #${index + 1}]`).join(' ')
      if (markers) message = `${message} ${markers}`
    }
  }
  return message
}

/** Shared wire contract; adapters supply runtime detection, address and authentication. */
export function createConversationTransport(environment: TransportEnvironment): ConversationOperations {
  const requestBody = (message: string, imageDataUri?: string | null, audioDataUri?: string | null,
    videoDataUri?: string | null, systemInstruction = '', documentAttachment?: DocumentAttachment | null,
    workspacePath?: string | null, chatId?: string | null, agentId?: string | null, planMode = false,
    pinnedMcpServer?: string | null, attachmentMetadata?: AttachmentMetadata) => ({
    ...(attachmentMetadata ? { attachmentMetadata } : {}),
    message: withMediaPlaceholders(message, imageDataUri, videoDataUri), systemInstruction,
    imageDataUri, audioDataUri, videoDataUri, documentAttachment, workspacePath, chatId, agentId, planMode, pinnedMcpServer,
  })

  const validate = (request: ReturnType<typeof requestBody>) => {
    const body = JSON.stringify(request)
    if (new TextEncoder().encode(body).byteLength > 32 * 1024 * 1024) throw new Error('Attachments exceed the 32 MiB request limit. Send fewer files at once.')
    return body
  }
  const post = (route: string, request: ReturnType<typeof requestBody>) => environment.fetch(`${environment.apiBase()}/${route}`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: validate(request),
  })


  return {
    async sendChatMessage(message, imageDataUri, audioDataUri, videoDataUri, documentAttachment,
      workspacePath, chatId, agentId, planMode, attachmentMetadata) {
      const request = requestBody(message, imageDataUri, audioDataUri, videoDataUri, '', documentAttachment,
        workspacePath, chatId, agentId, planMode, undefined, attachmentMetadata)
      if (!environment.isNative()) {
        try {
          const response = await post('chat', request)
          const data = await response.json().catch(() => null)
          if (!response.ok) return { provider: 'error', model: 'error',
            text: data?.text || data?.message || data?.status || `Local API returned HTTP ${response.status}` }
          if (!data || typeof data.text !== 'string') return {
            provider: 'error', model: 'error', text: 'Local API returned an invalid chat response.',
          }
          return data
        } catch (error) {
          console.error('Failed to send chat message to local server:', error)
          return { provider: 'error', model: 'error', text: `Failed to connect to Local API Server: ${error}` }
        }
      }
      validate(request)
      const { invoke } = await import('@tauri-apps/api/core')
      const response = await invoke<ChatResponse>('send_chat_message', { ownerToken: environment.ownerToken(), request, ...(attachmentMetadata ? { attachmentMetadata } : {}) })
      return response
    },

    async streamChatMessage(message, onChunk, imageDataUri, audioDataUri, videoDataUri, systemInstruction,
      onProgress, documentAttachment, workspacePath, chatId, agentId, planMode, pinnedMcpServer,
      onApprovalRequested, onTurnStarted, attachmentMetadata) {
      const request = requestBody(message, imageDataUri, audioDataUri, videoDataUri, systemInstruction,
        documentAttachment, workspacePath, chatId, agentId, planMode, pinnedMcpServer, attachmentMetadata)
      const callbacks = { onChunk, onProgress, onApprovalRequested, onTurnStarted }
      if (!environment.isNative()) {
        const response = await post('chat-stream', request)
        if (!response.ok) {
          const data = await response.json().catch(() => null)
          throw new Error(data?.text || data?.message || `HTTP ${response.status}`)
        }
        return readStream(response, callbacks)
      }
      validate(request)
      const { invoke, Channel } = await import('@tauri-apps/api/core')
      const onEvent = new Channel<StreamEvent>()
      onEvent.onmessage = event => { deliverEvent(event, callbacks) }
      // Native approvals arrive via the global tool-approval-requested event bus.
      const response = await invoke<ChatResponse>('stream_chat_message', {
        ownerToken: environment.ownerToken(), request, onEvent, ...(attachmentMetadata ? { attachmentMetadata } : {}),
      })
      return response
    },
  }
}
