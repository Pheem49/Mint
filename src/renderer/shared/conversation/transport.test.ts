import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as desktop from '../../src/tauri'
import * as web from '../../src-web/tauri'
import type { AgentProgress } from '../types'

const native = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: native.invoke,
  Channel: class { onmessage = (_event: unknown) => {} },
}))

const response = { provider: 'test', model: 'test-model', text: 'สวัสดี' }
const encoder = new TextEncoder()
function streamResponse(text: string, byteByByte = false) {
  const bytes = encoder.encode(text)
  const body = new ReadableStream<Uint8Array>({ start(controller) {
    if (byteByByte) for (const byte of bytes) controller.enqueue(Uint8Array.of(byte))
    else controller.enqueue(bytes)
    controller.close()
  } })
  return new Response(body)
}
const done = JSON.stringify({ type: 'done', response })

beforeEach(() => {
  vi.stubGlobal('window', {
    location: { hostname: 'mint.test', port: '9000', protocol: 'http:' },
    localStorage: { getItem: () => 'owner-token' },
  })
  native.invoke.mockReset()
})
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })

describe.each([['Desktop', desktop], ['Web', web]] as const)('%s Conversation transport', (_name, adapter) => {
  it('sends plan mode and authenticated conversation context over HTTP', async () => {
    let request: any
    let authorization: string | null = null
    let url = ''
    vi.stubGlobal('fetch', async (input: string, init: RequestInit) => {
      url = input
      request = JSON.parse(init.body as string)
      authorization = new Headers(init.headers).get('Authorization')
      return streamResponse(done + '\n')
    })
    await expect(adapter.streamChatMessage('inspect', () => {}, 'image1 image2', 'audio', 'video',
      'instructions', undefined, null, '/project', 'chat-1', 'agent-1', true, 'mcp-1')).resolves.toEqual(response)
    expect(request).toEqual({ message: 'inspect [Image #1] [Image #2] [Video #1]',
      systemInstruction: 'instructions', imageDataUri: 'image1 image2', audioDataUri: 'audio', videoDataUri: 'video',
      documentAttachment: null, workspacePath: '/project', chatId: 'chat-1', agentId: 'agent-1', planMode: true, pinnedMcpServer: 'mcp-1' })
    expect(authorization).toBe('Bearer owner-token')
    expect(url).toBe(_name === 'Desktop' ? 'http://mint.test:3000/api/chat-stream' : '/api/chat-stream')
  })

  it('reads ordered events across split UTF-8 bytes and a final line without newline', async () => {
    const progress: AgentProgress = { type: 'ThinkingDelta', data: { id: 'thought-1', delta: 'คิด', elapsed_ms: 10 } }
    const approval = { RunShell: { command: 'pwd', mode: 'normal', background: false } }
    const res = streamResponse([
      JSON.stringify({ type: 'started', interactionId: 42 }),
      JSON.stringify({ type: 'chunk', chunk: 'สวัสดี' }),
      JSON.stringify({ type: 'progress', progress }),
      JSON.stringify({ type: 'approval-requested', token: 'approval-1', approval }), done,
    ].join('\r\n'), true)
    vi.stubGlobal('fetch', async () => res)
    const events: unknown[] = []
    await expect(adapter.streamChatMessage('hello', chunk => events.push(chunk), undefined, undefined, undefined,
      undefined, event => events.push(event), undefined, undefined, undefined, undefined, undefined, undefined,
      event => events.push(event), id => events.push(id))).resolves.toEqual(response)
    expect(events).toEqual([42, 'สวัสดี', progress, { token: 'approval-1', approval }])
    expect(res.body!.locked).toBe(false)
  })

  it('propagates callback failures and releases the response stream', async () => {
    const failure = new Error('consumer failed')
    const res = streamResponse(JSON.stringify({ type: 'chunk', chunk: 'hello' }) + '\n' + done + '\n')
    vi.stubGlobal('fetch', async () => res)
    await expect(adapter.streamChatMessage('hello', () => { throw failure })).rejects.toBe(failure)
    expect(res.body!.locked).toBe(false)
  })

  it('rejects incomplete streams and releases the reader', async () => {
    const res = streamResponse('{"type":"chunk","chunk":"partial"}\n')
    vi.stubGlobal('fetch', async () => res)
    await expect(adapter.streamChatMessage('hello', () => {})).rejects.toThrow('without a final response')
    expect(res.body!.locked).toBe(false)
  })

  it('preserves HTTP errors returned before streaming starts', async () => {
    vi.stubGlobal('fetch', async () => new Response('{"text":"approval unavailable"}', { status: 403 }))
    await expect(adapter.streamChatMessage('hello', () => {})).rejects.toThrow('approval unavailable')
  })

  it('keeps valid events after malformed and unknown lines', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.stubGlobal('fetch', async () => streamResponse('broken json\n{"type":"future"}\n' + done + '\n'))
    await expect(adapter.streamChatMessage('hello', () => {})).resolves.toEqual(response)
  })

  it('sends plan mode for non-streaming HTTP requests too', async () => {
    let request: any
    vi.stubGlobal('fetch', async (_url: string, init: RequestInit) => {
      request = JSON.parse(init.body as string)
      return Response.json(response)
    })
    // The platform contract should support the same option on both operations.
    await adapter.sendChatMessage('hello', undefined, undefined, undefined, undefined, undefined, 'chat-1', undefined, true)
    expect(request).toMatchObject({ message: 'hello', chatId: 'chat-1', planMode: true })
  })

  it('preserves IPC ownership, events and the backend attachment receipt', async () => {
    ;(window as any).__TAURI_INTERNALS__ = {}
    let request: any
    let owner: unknown
    const pictures: unknown[] = []
    let attachmentMetadata: unknown
    native.invoke.mockImplementation(async (command: string, args: any) => {
      if (command === 'save_pictures') { pictures.push(args); return }
      request = args.request; owner = args.ownerToken; attachmentMetadata = args.attachmentMetadata
      args.onEvent.onmessage({ type: 'started', interactionId: 7 })
      args.onEvent.onmessage({ type: 'chunk', chunk: 'hello' })
      args.onEvent.onmessage({ type: 'progress', progress: { type: 'ThinkingDelta', data: { id: 'thought-1', delta: 'working', elapsed_ms: 10 } } })
      return response
    })
    const events: unknown[] = []
    await expect(adapter.streamChatMessage('hello', value => events.push(value), 'image1 image2', undefined, 'video1',
      'system', value => events.push(value), undefined, '/project', 'chat-1', 'agent-1', true, 'mcp-1',
      undefined, id => events.push(id), { requestId: 'submission-1', imageNames: ['one.png', 'two.png'], videoNames: ['clip.mp4'] })).resolves.toEqual(response)
    expect(request).toMatchObject({ planMode: true, chatId: 'chat-1', pinnedMcpServer: 'mcp-1' })
    expect(owner).toBe('owner-token')
    expect(events).toEqual([7, 'hello', { type: 'ThinkingDelta', data: { id: 'thought-1', delta: 'working', elapsed_ms: 10 } }])
    expect(pictures).toEqual([])
    expect(attachmentMetadata).toEqual({ requestId: 'submission-1', imageNames: ['one.png', 'two.png'], videoNames: ['clip.mp4'] })
  })

  it('defaults to ordinary agent mode and preserves existing media markers', async () => {
    let request: any
    vi.stubGlobal('fetch', async (_url: string, init: RequestInit) => {
      request = JSON.parse(init.body as string)
      return streamResponse(done + '\n')
    })
    await adapter.streamChatMessage('hello [Image #1] [Video #1]', () => {}, 'image', undefined, 'video')
    expect(request).toMatchObject({ message: 'hello [Image #1] [Video #1]', systemInstruction: '', planMode: false })
  })

  it('preserves non-streaming error responses', async () => {
    vi.stubGlobal('fetch', async () => Response.json({ message: 'access denied' }, { status: 403 }))
    await expect(adapter.sendChatMessage('hello')).resolves.toEqual({ provider: 'error', model: 'error', text: 'access denied' })
    vi.stubGlobal('fetch', async () => Response.json({ unrelated: true }))
    await expect(adapter.sendChatMessage('hello')).resolves.toMatchObject({ provider: 'error', text: expect.stringContaining('invalid chat response') })
  })

  it('sends authenticated plan mode in non-streaming IPC', async () => {
    ;(window as any).__TAURI_INTERNALS__ = {}
    let captured: any
    native.invoke.mockImplementation(async (command: string, args: any) => {
      expect(command).toBe('send_chat_message')
      captured = args
      return response
    })
    await expect(adapter.sendChatMessage('hello', undefined, undefined, undefined, undefined, undefined,
      'chat-1', undefined, true)).resolves.toEqual(response)
    expect(captured).toMatchObject({ ownerToken: 'owner-token', request: { chatId: 'chat-1', planMode: true } })
  })

  it('validates the 32 MiB request limit without TextEncoder allocations or double JSON.stringify on native IPC', async () => {
    const encodeSpy = vi.spyOn(TextEncoder.prototype, 'encode')
    const stringifySpy = vi.spyOn(JSON, 'stringify')
    ;(window as any).__TAURI_INTERNALS__ = {}
    native.invoke.mockResolvedValue(response)

    await expect(adapter.streamChatMessage('hello', () => {}, 'data:image/png;base64,AAAA')).resolves.toEqual(response)
    expect(encodeSpy).not.toHaveBeenCalled()
    expect(stringifySpy).not.toHaveBeenCalled()

    const oversized = 'A'.repeat(32 * 1024 * 1024 + 64)
    await expect(adapter.streamChatMessage('hello', () => {}, oversized)).rejects.toThrow(
      'Attachments exceed the 32 MiB request limit. Send fewer files at once.',
    )
    delete (window as any).__TAURI_INTERNALS__
    await expect(adapter.streamChatMessage('hello', () => {}, oversized)).rejects.toThrow(
      'Attachments exceed the 32 MiB request limit. Send fewer files at once.',
    )
  })
})
