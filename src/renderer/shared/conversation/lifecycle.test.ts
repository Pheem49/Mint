import { afterEach, describe, expect, it, vi } from 'vitest'
import { createConversationLifecycle, conversationPollIntervalsMs, type ConversationTransport, type ConversationEffects, type PendingApproval } from './lifecycle'
import type { AgentProgress, ChatResponse, ConversationChanges, ConversationSnapshot, InteractionMemory, ChatSession } from '../types'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
function turn(id: number, chatId = 'A'): InteractionMemory {
  return { id, chatId, userText: 'same prompt', aiText: `reply ${id}`, provider: 'system', model: 'test', createdAt: '2026-10-10T00:00:00Z', status: 'completed' }
}
function snapshot(id: number, cursor = id, chatId = 'A'): ConversationSnapshot {
  return { interactions: [turn(id, chatId)], cursor, hasOlder: false }
}
const reply: ChatResponse = { provider: 'system', model: 'test', text: 'completed' }
const activity: AgentProgress = { type: 'ToolStart', data: { action: 'read_file', input: { path: 'file.txt' } } }

function setup(overrides: Partial<ConversationTransport> = {}, callbacks: ConversationEffects = {}, drafts?: { read(id: string): string; write(id: string, text: string): void }) {
  const streams: Array<{ chatId: string; onProgress: (p: AgentProgress) => void; onApproval: (p: PendingApproval) => void; onStarted: (id: number) => void; result: ReturnType<typeof deferred<ChatResponse>> }> = []
  const saved: Array<{ id: number; progress: AgentProgress[] }> = []
  const submitted: Array<{ token: string; approved: boolean; answer?: string }> = []
  const transport: ConversationTransport = {
    getConversationSnapshot: async () => ({ interactions: [], cursor: 0, hasOlder: false }),
    getConversationChanges: async () => ({ changes: [], cursor: 0, hasMore: false }),
    listChatSessions: async () => [],
    saveInteractionAgentActivity: async (id: number, progress: AgentProgress[]) => { saved.push({ id, progress }) },
    cancelChatMessage: async () => {},
    clearChatHistory: async () => 1,
    submitToolApproval: async (token: string, approved: boolean, answer?: string) => { submitted.push({ token, approved, answer }) },
    streamChatMessage: (_message: string, _chunk: unknown, _image: unknown, _audio: unknown, _video: unknown, _instruction: unknown, onProgress: (progress: AgentProgress) => void, _document: unknown, _workspace: unknown, chatId: string, _agent: unknown, _plan: unknown, _mcp: unknown, onApproval: (approval: PendingApproval) => void, onStarted: (id: number) => void) => {
      const result = deferred<ChatResponse>()
      streams.push({ chatId, onProgress, onApproval, onStarted, result })
      return result.promise
    },
    ...overrides,
  }
  return { lifecycle: createConversationLifecycle('A', {}, transport, callbacks, drafts), streams, saved, submitted }
}

afterEach(() => vi.useRealTimers())

describe('Conversation lifecycle through the renderer interface', () => {
  it('keeps attachments until backend acceptance and reuses a failed submission ID', async () => {
    const received: any[] = []
    const results = [deferred<ChatResponse>(), deferred<ChatResponse>()]
    let started!: (id: number) => void
    const { lifecycle } = setup({ streamChatMessage: (...args) => {
      received.push(args[15]); started = args[14]!
      return results[received.length - 1].promise
    } })
    const document = { filename: 'notes.md', dataUri: 'data:text/markdown;base64,aGVsbG8=' }
    lifecycle.compose('Read this'); lifecycle.attachDocument(document)
    const options = { documentAttachment: document, clearComposer: true }
    const failed = lifecycle.sendPrompt('Read this', options)
    expect(lifecycle.getSnapshot().documentAttachment).toEqual(document)
    results[0].reject(new Error('Offline')); await failed
    expect(lifecycle.getSnapshot().message).toBe('Read this')
    const retried = lifecycle.sendPrompt('Read this', options)
    expect(received[0]?.requestId).toBeTruthy()
    expect(received[1]?.requestId).toBe(received[0]?.requestId)
    started(12)
    expect(lifecycle.getSnapshot().documentAttachment).toBeNull()
    expect(lifecycle.getSnapshot().message).toBe('')
    results[1].resolve(reply); await retried
  })

  it('fingerprints attachments for retry deduplication without calling JSON.stringify on attachment payloads', async () => {
    const received: any[] = []
    const results = [deferred<ChatResponse>(), deferred<ChatResponse>(), deferred<ChatResponse>()]
    const { lifecycle } = setup({ streamChatMessage: (...args) => {
      received.push(args[15])
      return results[received.length - 1].promise
    } })
    const stringifySpy = vi.spyOn(JSON, 'stringify')
    const img1 = { name: 'photo.png', dataUri: 'data:image/png;base64,AAAA1111' }
    const img2 = { name: 'photo.png', dataUri: 'data:image/png;base64,BBBB2222' }

    const attempt1 = lifecycle.sendPrompt('Analyze', { imageAttachments: [img1] })
    results[0].reject(new Error('Network error')); await attempt1

    const attempt2 = lifecycle.sendPrompt('Analyze', { imageAttachments: [img1] })
    results[1].reject(new Error('Network error')); await attempt2

    const attempt3 = lifecycle.sendPrompt('Analyze', { imageAttachments: [img2] })
    results[2].resolve(reply); await attempt3

    expect(stringifySpy).not.toHaveBeenCalled()
    expect(received[0]?.requestId).toBeTruthy()
    expect(received[1]?.requestId).toBe(received[0]?.requestId)
    expect(received[2]?.requestId).not.toBe(received[0]?.requestId)
  })

  it('rejects old A snapshots after A → B → A, including an old failure', async () => {
    const reads = [deferred<ConversationSnapshot>(), deferred<ConversationSnapshot>(), deferred<ConversationSnapshot>()]
    let next = 0
    const { lifecycle } = setup({ getConversationSnapshot: () => reads[next++].promise })
    const oldA = lifecycle.refreshHistory()
    lifecycle.activate('B', 'draft B')
    const b = lifecycle.refreshHistory()
    lifecycle.activate('A', 'draft A')
    const newA = lifecycle.refreshHistory()
    reads[2].resolve(snapshot(30))
    await newA
    reads[1].resolve(snapshot(20, 20, 'B'))
    reads[0].reject(new Error('old failure'))
    await Promise.all([oldA, b])
    expect(lifecycle.getSnapshot()).toMatchObject({ conversationId: 'A', message: 'draft A', interactions: [{ id: 30 }] })
  })

  it('keeps a newer poll when an older full snapshot finishes last', async () => {
    const late = deferred<ConversationSnapshot>()
    let first = true
    const { lifecycle } = setup({
      getConversationSnapshot: () => first ? (first = false, Promise.resolve(snapshot(1))) : late.promise,
      getConversationChanges: async () => ({ changes: [{ sequence: 3, interactionId: 3, interaction: turn(3) }], cursor: 3, hasMore: false }),
    })
    await lifecycle.refreshHistory()
    const refresh = lifecycle.refreshHistory()
    await lifecycle.syncHistory()
    late.resolve(snapshot(2))
    await refresh
    expect(lifecycle.getSnapshot().interactions.map((row: InteractionMemory) => row.id)).toEqual([1, 3])
  })

  it('does not overlap polls and reconciles deletion across change pages', async () => {
    const page = deferred<ConversationChanges>()
    let reads = 0
    const { lifecycle } = setup({
      getConversationSnapshot: async () => snapshot(1),
      getConversationChanges: async () => {
        reads++
        return reads === 1 ? page.promise : { changes: [{ sequence: 3, interactionId: 2, interaction: turn(2) }], cursor: 3, hasMore: false }
      },
    })
    await lifecycle.refreshHistory()
    const poll = lifecycle.syncHistory()
    await lifecycle.syncHistory()
    page.resolve({ changes: [{ sequence: 2, interactionId: 1, interaction: null }], cursor: 2, hasMore: true })
    await poll
    expect(reads).toBe(2)
    expect(lifecycle.getSnapshot().interactions.map((row: InteractionMemory) => row.id)).toEqual([2])
  })

  it('rejects older-history pages after another conversation is selected', async () => {
    const page = deferred<ConversationSnapshot>()
    const { lifecycle } = setup({ getConversationSnapshot: (_id: string, before: number | null) => before ? page.promise : Promise.resolve(snapshot(10)) })
    await lifecycle.refreshHistory()
    const older = lifecycle.loadOlder()
    lifecycle.activate('B')
    page.resolve({ interactions: [turn(1)], cursor: 10, hasOlder: true })
    await older
    expect(lifecycle.getSnapshot()).toMatchObject({ conversationId: 'B', interactions: [], hasOlderInteractions: false })
  })

  it('persists late activity to the initiating turn without touching the new run', async () => {
    const { lifecycle, streams, saved } = setup()
    const a = lifecycle.sendPrompt('same prompt', { agentMode: true })
    streams[0].onStarted(10)
    streams[0].onProgress(activity)
    lifecycle.activate('B')
    const b = lifecycle.sendPrompt('same prompt', { agentMode: true })
    streams[1].onStarted(20)
    streams[0].result.resolve(reply)
    await a
    expect(saved).toEqual([{ id: 10, progress: [activity] }])
    expect(lifecycle.getSnapshot()).toMatchObject({ conversationId: 'B', sending: true, activeTurnId: 20, agentActivitySnapshots: {} })
    streams[1].result.resolve(reply)
    await b
  })

  it('does not let delayed cancellation or an old run finish clear a newer run in the same chat', async () => {
    const cancel = deferred<void>()
    const { lifecycle, streams } = setup({ cancelChatMessage: () => cancel.promise })
    const old = lifecycle.sendPrompt('old')
    const cancellation = lifecycle.cancel()
    lifecycle.activate('B')
    lifecycle.activate('A')
    const current = lifecycle.sendPrompt('new')
    streams[1].onStarted(22)
    streams[1].onApproval({ token: 'new-approval', approval: { RunShell: { command: 'pwd' } } })
    cancel.resolve()
    await cancellation
    streams[0].onProgress(activity)
    streams[0].onApproval({ token: 'old-approval', approval: { RunShell: { command: 'old' } } })
    streams[0].result.resolve(reply)
    await old
    expect(lifecycle.getSnapshot()).toMatchObject({ sending: true, sendingMessage: 'new', activeTurnId: 22, agentProgress: [], pendingApprovals: { A: { token: 'new-approval' } } })
    streams[1].result.resolve(reply)
    await current
  })

  it('keeps approvals for a background conversation and does not erase a replacement token', async () => {
    const answer = deferred<void>()
    const { lifecycle } = setup({ submitToolApproval: () => answer.promise })
    lifecycle.receiveApproval('A', { token: 'first', approval: { RunShell: {} } })
    const resolve = lifecycle.resolveApproval(true)
    lifecycle.receiveApproval('A', { token: 'second', approval: { RunShell: {} } })
    lifecycle.activate('B')
    answer.resolve()
    await resolve
    expect(lifecycle.getSnapshot().pendingApprovals).toMatchObject({ A: { token: 'second' } })
    lifecycle.activate('A')
    expect(lifecycle.getSnapshot().pendingApprovals.A.token).toBe('second')
  })

  it('never auto-approves persistent skill edits during a session allowance', async () => {
    const { lifecycle, streams, submitted } = setup()
    const run = lifecycle.sendPrompt('work', { agentMode: true })
    streams[0].onApproval({ token: 'shell', approval: { RunShell: {} } })
    await lifecycle.resolveApproval(true, true)
    const skill = { token: 'skill', approval: { SkillWrite: { path: '.agents/skills/example/SKILL.md' } } }
    streams[0].onApproval(skill)
    expect(lifecycle.getSnapshot().pendingApprovals.A).toEqual(skill)
    expect(submitted).toEqual([{ token: 'shell', approved: true, answer: undefined }])
    streams[0].result.resolve(reply)
    await run
  })

  it('retains a failed approval submission for retry', async () => {
    const errors: unknown[] = []
    const { lifecycle } = setup({ submitToolApproval: async () => { throw new Error('offline') } }, { onError: (error: unknown) => errors.push(error) })
    lifecycle.receiveApproval('A', { token: 'pending', approval: { RunShell: {} } })
    await lifecycle.resolveApproval(true)
    expect(lifecycle.getSnapshot().pendingApprovals.A.token).toBe('pending')
    expect(errors).toHaveLength(1)
  })

  it('does not erase a new conversation when clearing old history completes late', async () => {
    const cleared = deferred<number>()
    const { lifecycle } = setup({ clearChatHistory: () => cleared.promise, getConversationSnapshot: async () => snapshot(20, 20, 'B') })
    const clear = lifecycle.clearHistory()
    lifecycle.activate('B', 'keep this draft')
    await lifecycle.refreshHistory()
    cleared.resolve(1)
    await clear
    expect(lifecycle.getSnapshot()).toMatchObject({ message: 'keep this draft', interactions: [{ id: 20 }] })
  })

  it('batches thinking updates and drops queued updates when switching sessions', async () => {
    vi.useFakeTimers()
    const { lifecycle, streams } = setup()
    const run = lifecycle.sendPrompt('work')
    streams[0].onProgress({ type: 'ThinkingDelta', data: { id: 'think-1', delta: 'thinking', elapsed_ms: 1 } })
    expect(lifecycle.getSnapshot().agentProgress).toEqual([])
    lifecycle.activate('B')
    await vi.advanceTimersByTimeAsync(100)
    expect(lifecycle.getSnapshot().agentProgress).toEqual([])
    streams[0].result.resolve(reply)
    await run
    expect(vi.getTimerCount()).toBe(0)
  })
})

it('does not resurrect deleted history from a slow older page', async () => {
  const older = deferred<ConversationSnapshot>()
  const { lifecycle } = setup({
    getConversationSnapshot: (_id: string, before: number | null) => before ? older.promise : Promise.resolve(snapshot(10)),
    getConversationChanges: async () => ({ changes: [{ sequence: 11, interactionId: 1, interaction: null }], cursor: 11, hasMore: false }),
  })
  await lifecycle.refreshHistory()
  const load = lifecycle.loadOlder()
  await lifecycle.syncHistory()
  older.resolve({ interactions: [turn(1)], cursor: 10, hasOlder: false })
  await load
  expect(lifecycle.getSnapshot().interactions.map((row: InteractionMemory) => row.id)).toEqual([10])
})

it('allows the next ordinary approval even if it arrives before the first submission returns', async () => {
  const submission = deferred<void>()
  const { lifecycle, streams, submitted } = setup({ submitToolApproval: async (token: string, approved: boolean, answer?: string) => {
    submitted.push({ token, approved, answer })
    if (token === 'first') {
      streams[0].onApproval({ token: 'next', approval: { RunShell: {} } })
      await submission.promise
    }
  } })
  const run = lifecycle.sendPrompt('work')
  streams[0].onApproval({ token: 'first', approval: { RunShell: {} } })
  const approval = lifecycle.resolveApproval(true, true)
  submission.resolve()
  await approval
  expect(submitted.map(row => row.token)).toEqual(['first', 'next'])
  expect(lifecycle.getSnapshot().pendingApprovals).toEqual({})
  streams[0].result.resolve(reply)
  await run
})

it('retries history after an invisible period and removes browser timers on detach', async () => {
  vi.useFakeTimers()
  const focus = new EventTarget()
  const visibility = new EventTarget()
  let visible: DocumentVisibilityState = 'hidden'
  let reads = 0
  const { lifecycle } = setup({ getConversationSnapshot: async () => { reads++; return snapshot(1) } })
  const browser = {
    window: { addEventListener: focus.addEventListener.bind(focus), removeEventListener: focus.removeEventListener.bind(focus), setInterval, clearInterval },
    document: { addEventListener: visibility.addEventListener.bind(visibility), removeEventListener: visibility.removeEventListener.bind(visibility), get visibilityState() { return visible } },
  } as unknown as import('./lifecycle').ConversationBrowser
  const stop = lifecycle.watch(browser, true)
  await vi.advanceTimersByTimeAsync(5000)
  expect(reads).toBe(0)
  visible = 'visible'
  focus.dispatchEvent(new Event('focus'))
  await vi.advanceTimersByTimeAsync(0)
  expect(lifecycle.getSnapshot().interactions).toEqual([turn(1)])
  stop()
  expect(vi.getTimerCount()).toBe(0)
  focus.dispatchEvent(new Event('focus'))
  await vi.advanceTimersByTimeAsync(10000)
  expect(reads).toBe(1)
})

it('rejects a slow project picker after another project selection', async () => {
  const slowPath = deferred<string | null>()
  const sessions = [{ id: 'project-B', kind: 'chat', title: 'B', workspacePath: '/B', createdAt: '2026-10-10', updatedAt: '2026-10-10' }]
  const { lifecycle } = setup({ listChatSessions: async () => sessions })
  let selected = ''
  const navigation = {
    openSession: async (s: ChatSession) => { selected = s.id; lifecycle.activate(s.id) },
    newChat: async () => {}, showCurrentView: () => {},
  }
  const old = lifecycle.selectProject(() => slowPath.promise, navigation, 'chat')
  await lifecycle.selectProject(async () => '/B', navigation, 'workspace')
  slowPath.resolve('/A')
  await old
  expect(selected).toBe('project-B')
  expect(lifecycle.getSnapshot().conversationId).toBe('project-B')
})

it('does not replace a fresh session list with a slow earlier refresh', async () => {
  const old = deferred<ChatSession[]>()
  const latest = [{ id: 'B', kind: 'chat', title: 'B', workspacePath: '/B', createdAt: '2026-10-10', updatedAt: '2026-10-10' }]
  let count = 0
  const { lifecycle } = setup({ listChatSessions: () => count++ === 0 ? old.promise : Promise.resolve(latest) })
  const pending = lifecycle.refreshSessions()
  await lifecycle.refreshSessions()
  old.resolve([{ ...latest[0], id: 'A', title: 'old' }])
  await pending
  expect(lifecycle.getSnapshot().chatSessions).toEqual(latest)
})

it('preserves an outgoing draft even when switching before the persistence debounce', () => {
  const drafts = new Map<string, string>([['B', 'saved B']])
  const { lifecycle } = setup({}, {}, { read: id => drafts.get(id) ?? '', write: (id, text) => { drafts.set(id, text) } })
  lifecycle.compose('new A draft')
  lifecycle.activate('B')
  expect(lifecycle.getSnapshot().message).toBe('saved B')
  lifecycle.compose('new B draft')
  lifecycle.activate('A')
  expect(lifecycle.getSnapshot().message).toBe('new A draft')
  expect(drafts.get('B')).toBe('new B draft')
})

it('revokes ordinary session allowances when leaving the initiating conversation', async () => {
  const { lifecycle, streams, submitted } = setup()
  const run = lifecycle.sendPrompt('work')
  streams[0].onApproval({ token: 'first', approval: { RunShell: {} } })
  await lifecycle.resolveApproval(true, true)
  lifecycle.activate('B')
  streams[0].onApproval({ token: 'background', approval: { RunShell: {} } })
  expect(lifecycle.getSnapshot().pendingApprovals).toMatchObject({ A: { token: 'background' } })
  expect(submitted.map(row => row.token)).toEqual(['first'])
  streams[0].result.resolve(reply)
  await run
})

it('flushes queued progress when the transport fails before the debounce', async () => {
  vi.useFakeTimers()
  const { lifecycle, streams } = setup()
  const run = lifecycle.sendPrompt('work')
  streams[0].onProgress({ type: 'ThinkingDelta', data: { id: 'thinking', delta: 'working', elapsed_ms: 1 } })
  streams[0].result.reject(new Error('transport failed'))
  await run
  expect(lifecycle.getSnapshot().agentProgress).toEqual([{ type: 'ThinkingDelta', data: { id: 'thinking', delta: 'working', elapsed_ms: 1 } }])
  expect(vi.getTimerCount()).toBe(0)
})

it('can submit from a LAN HTTP browser without crypto.randomUUID', async()=>{
 vi.stubGlobal('crypto', {getRandomValues: globalThis.crypto.getRandomValues.bind(globalThis.crypto)})
 let receipt: any
 const {lifecycle}=setup({streamChatMessage:async(...args)=>{receipt=args[15];return reply}})
 await lifecycle.sendPrompt('Read',{documentAttachment:{filename:'notes.md',dataUri:'data:text/plain;base64,aGk='}})
 expect(receipt.requestId).toBeTruthy()
 expect(lifecycle.getSnapshot().sending).toBe(false)
 vi.unstubAllGlobals()
})

it('exports adaptive conversation polling intervals for active and idle runs', () => {
  expect(conversationPollIntervalsMs(true)).toEqual({ historyMs: 1500, sessionsMs: 5000 })
  expect(conversationPollIntervalsMs(false)).toEqual({ historyMs: 5000, sessionsMs: 15000 })
})

it('adapts polling intervals between idle and active runs in watch', async () => {
  vi.useFakeTimers()
  let historySyncCount = 0
  let sessionRefreshCount = 0
  const { lifecycle, streams } = setup({
    getConversationChanges: async () => { historySyncCount++; return { changes: [], cursor: 0, hasMore: false } },
    listChatSessions: async () => { sessionRefreshCount++; return [] },
  })
  await lifecycle.refreshHistory()

  const browser = {
    window: { addEventListener: vi.fn(), removeEventListener: vi.fn(), setInterval, clearInterval },
    document: { addEventListener: vi.fn(), removeEventListener: vi.fn(), visibilityState: 'visible' },
  } as unknown as import('./lifecycle').ConversationBrowser

  const stop = lifecycle.watch(browser, true)
  // Initial synchronous invocation at attach
  expect(historySyncCount).toBe(1)
  expect(sessionRefreshCount).toBe(0)

  // Idle state: history syncs every 5s, sessions every 15s
  await vi.advanceTimersByTimeAsync(4900)
  expect(historySyncCount).toBe(1)
  await vi.advanceTimersByTimeAsync(200)
  expect(historySyncCount).toBe(2)

  await vi.advanceTimersByTimeAsync(9700)
  expect(sessionRefreshCount).toBe(0)
  await vi.advanceTimersByTimeAsync(300)
  expect(sessionRefreshCount).toBe(1)

  // Start an active prompt -> switches to active polling (1.5s history, 5s sessions)
  const run = lifecycle.sendPrompt('test active run')
  const initialHistory = historySyncCount
  await vi.advanceTimersByTimeAsync(1500)
  expect(historySyncCount).toBe(initialHistory + 1)

  // Finish run -> returns to idle polling
  streams[0].result.resolve(reply)
  await run
  const afterRunHistory = historySyncCount
  await vi.advanceTimersByTimeAsync(1500)
  // Since it returned to idle (5000ms), 1500ms shouldn't trigger another sync
  expect(historySyncCount).toBe(afterRunHistory)

  stop()
  expect(vi.getTimerCount()).toBe(0)
})
