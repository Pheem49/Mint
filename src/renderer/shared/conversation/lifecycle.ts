import { initialConversationState, transitionConversation, type ConversationEvent, type ConversationState } from './coordinator'
import { compactAgentProgressForPersistence, mergeActivitySnapshots } from '../agentProgress'
import { requiresExplicitApproval } from '../utils/approval'
import { navigateToProject } from './projectNavigation'
import type { MintPlatformApi } from '../platform'
import type { AgentProgress, ChatResponse, ChatSession, InteractionMemory } from '../types'

export type ConversationTransport = Pick<MintPlatformApi,
  | 'getConversationSnapshot' | 'getConversationChanges' | 'listChatSessions'
  | 'streamChatMessage' | 'cancelChatMessage' | 'submitToolApproval'
  | 'saveInteractionAgentActivity' | 'clearChatHistory'>
export interface PendingApproval { token: string; approval: Record<string, unknown> }
export interface ConversationView extends ConversationState {
  conversationId: string
  interactions: InteractionMemory[]
  hasOlderInteractions: boolean
  agentActivitySnapshots: Record<string, AgentProgress[]>
  chatSessions: ChatSession[]
  streamingConversationId: string | null
  activeTurnId: number | null
  pendingApprovals: Record<string, PendingApproval>
  sessionAutoApproved: boolean
}
export interface ConversationEffects {
  onError?(error: unknown): void
  onApproval?(approval: PendingApproval, chatId: string): void
  onReply?(response: ChatResponse, chatId: string): void | Promise<void>
  onActivityPersisted?(turnId: number): void
  onRunCompleted?(): void | Promise<void>
  onVisibleRunCompleted?(chatId: string): void
}
export interface PromptOptions {
  imageAttachments?: ConversationState['imageAttachments']
  videoAttachments?: ConversationState['videoAttachments']
  documentAttachment?: ConversationState['documentAttachment']
  audioDataUri?: string | null
  systemInstruction?: string
  clearComposer?: boolean
  pinnedMcpServer?: string
  agentMode?: boolean
  forceAgentMode?: boolean
  planMode?: boolean
}
export interface ConversationDrafts {
  read(id: string): string
  write(id: string, text: string): void
}
interface Session { chatId: string }
interface Run {
  session: Session
  turnId: number | null
  autoApprove: boolean
  progress: AgentProgress[]
  queuedProgress: AgentProgress[]
  timer?: ReturnType<typeof setTimeout>
}
export interface ConversationBrowser {
  window: Pick<Window, 'addEventListener' | 'removeEventListener' | 'setInterval' | 'clearInterval'>
  document: Pick<Document, 'visibilityState' | 'addEventListener' | 'removeEventListener'>
}

/** Owns ordering and lifecycle; callers never handle generations or cursors. */
export function createConversationLifecycle(
  conversationId: string,
  initial: Partial<ConversationState>,
  transport: ConversationTransport,
  effects: ConversationEffects = {},
  drafts?: ConversationDrafts,
) {
  let state: ConversationView = {
    ...initialConversationState(initial), conversationId, interactions: [], hasOlderInteractions: false,
    agentActivitySnapshots: {}, chatSessions: [], streamingConversationId: null, activeTurnId: null,
    pendingApprovals: {}, sessionAutoApproved: false,
  }
  let session: Session = { chatId: conversationId }
  let cursor: number | null = null
  let historySerial = 0
  let sessionsSerial = 0
  let projectSerial = 0
  let poll: Session | null = null
  const runs = new Map<string, Run>()
  const subscribers = new Set<() => void>()
  const update = (patch: Partial<ConversationView>) => {
    state = { ...state, ...patch }
    subscribers.forEach(notify => notify())
  }
  const dispatch = (event: ConversationEvent) => update(transitionConversation(state, event))
  const currentRun = (run: Run) => run.session === session && runs.get(session.chatId) === run
  const clearProgressTimer = (run: Run) => {
    if (run.timer !== undefined) clearTimeout(run.timer)
    run.timer = undefined
  }
  const flushProgress = (run: Run) => {
    clearProgressTimer(run)
    const batch = run.queuedProgress
    run.queuedProgress = []
    if (currentRun(run) && batch.length) dispatch({ type: 'progress_received', progress: batch })
  }
  const removeApproval = (chatId: string, token?: string) => {
    if (!state.pendingApprovals[chatId] || (token && state.pendingApprovals[chatId].token !== token)) return
    const pendingApprovals = { ...state.pendingApprovals }
    delete pendingApprovals[chatId]
    update({ pendingApprovals })
  }

  function activate(id: string, draft?: string) {
    if (session.chatId === id) return
    // Flush the outgoing draft before the debounce is cancelled by navigation.
    try { drafts?.write(session.chatId, state.message) } catch { /* Storage is best-effort. */ }
    if (draft === undefined) {
      try { draft = drafts?.read(id) ?? '' } catch { draft = '' }
    }
    const oldRun = runs.get(session.chatId)
    if (oldRun) {
      clearProgressTimer(oldRun)
      oldRun.queuedProgress = []
      oldRun.autoApprove = false
    }
    session = { chatId: id }
    cursor = null
    projectSerial++
    update({
      ...transitionConversation(state, { type: 'session_switched', draft }), conversationId: id,
      interactions: [], hasOlderInteractions: false, agentActivitySnapshots: {},
      streamingConversationId: null, activeTurnId: null, sessionAutoApproved: false,
    })
  }

  async function refreshHistory(): Promise<void> {
    const origin = session
    const serial = ++historySerial
    try {
      const snapshot = await transport.getConversationSnapshot(origin.chatId, null, 50)
      if (origin !== session || serial !== historySerial || (cursor !== null && cursor > snapshot.cursor)) return
      cursor = snapshot.cursor
      update({ interactions: snapshot.interactions, hasOlderInteractions: snapshot.interactions.length > 0 && snapshot.hasOlder,
        agentActivitySnapshots: mergeActivitySnapshots(state.agentActivitySnapshots, snapshot.interactions) })
    } catch (error) {
      if (origin === session && serial === historySerial) throw error
    }
  }

  async function refreshSessions(): Promise<ChatSession[]> {
    const serial = ++sessionsSerial
    const sessions = await transport.listChatSessions()
    if (serial === sessionsSerial) {
      const unchanged = state.chatSessions.length === sessions.length && state.chatSessions.every((s, i) =>
        s.id === sessions[i]?.id && s.updatedAt === sessions[i]?.updatedAt && s.title === sessions[i]?.title
        && s.workspacePath === sessions[i]?.workspacePath && s.messageCount === sessions[i]?.messageCount
        && s.totalBytes === sessions[i]?.totalBytes)
      if (!unchanged) update({ chatSessions: sessions })
    }
    return sessions
  }

  async function syncHistory(isActive: () => boolean = () => true) {
    const origin = session
    if (poll === origin || !isActive()) return
    poll = origin
    try {
      if (cursor === null) { await refreshHistory(); return }
      let changed = false
      for (let page = 0; page < 10; page++) {
        const batch = await transport.getConversationChanges(origin.chatId, cursor!, 100)
        if (origin !== session || !isActive() || (cursor !== null && cursor > batch.cursor)) return
        cursor = batch.cursor
        if (batch.changes.length) {
          changed = true
          const byId = new Map(state.interactions.map(row => [row.id, row]))
          const snapshots = { ...state.agentActivitySnapshots }
          for (const change of batch.changes) {
            if (change.interaction) {
              byId.set(change.interactionId, change.interaction)
              if (Array.isArray(change.interaction.agentActivity)) snapshots[String(change.interactionId)] = change.interaction.agentActivity
            } else {
              byId.delete(change.interactionId)
              delete snapshots[String(change.interactionId)]
            }
          }
          const interactions = Array.from(byId.values()).sort((a, b) => a.id - b.id)
          update({ interactions, agentActivitySnapshots: snapshots,
            hasOlderInteractions: interactions.length > 0 && state.hasOlderInteractions })
        }
        if (!batch.hasMore) break
      }
      if (changed && origin === session && isActive()) await refreshSessions()
    } catch {
      // Cross-process SQLite synchronization retries on the next visible tick.
    } finally {
      if (poll === origin) poll = null
    }
  }

  async function loadOlder() {
    const origin = session
    const before = state.interactions[0]?.id
    if (before == null) return
    const revision = cursor
    let page
    try { page = await transport.getConversationSnapshot(origin.chatId, before, 50) }
    catch (error) { if (origin === session) throw error; return }
    if (origin !== session || revision !== cursor) return
    const rows = new Map([...page.interactions, ...state.interactions].map(row => [row.id, row]))
    update({ interactions: Array.from(rows.values()).sort((a, b) => a.id - b.id), hasOlderInteractions: page.hasOlder })
  }

  function receiveApproval(chatId: string, approval: PendingApproval) {
    const run = runs.get(chatId)
    if (run?.autoApprove && !requiresExplicitApproval(approval.approval)) {
      void transport.submitToolApproval(approval.token, true).catch(error => effects.onError?.(error))
    } else {
      update({ pendingApprovals: { ...state.pendingApprovals, [chatId]: approval } })
      effects.onApproval?.(approval, chatId)
    }
  }

  async function resolveApproval(approved: boolean, autoApprove = false, answer?: string) {
    const origin = session
    const approval = state.pendingApprovals[origin.chatId]
    if (!approval) return
    const run = runs.get(origin.chatId)
    const previousAllowance = run?.autoApprove ?? false
    const allow = autoApprove && run && !requiresExplicitApproval(approval.approval)
    if (allow) {
      run.autoApprove = true
      update({ sessionAutoApproved: true })
    }
    try {
      await transport.submitToolApproval(approval.token, approved, answer)
      removeApproval(origin.chatId, approval.token)
    } catch (error) {
      if (allow && runs.get(origin.chatId) === run) {
        run.autoApprove = previousAllowance
        if (origin === session) update({ sessionAutoApproved: previousAllowance })
      }
      if (origin === session) effects.onError?.(error)
    }
  }

  async function cancel() {
    const origin = session
    const run = runs.get(origin.chatId)
    if (!state.sending || !run || !currentRun(run)) return
    runs.delete(origin.chatId)
    clearProgressTimer(run)
    const approvalToken = state.pendingApprovals[origin.chatId]?.token
    try {
      await transport.cancelChatMessage(origin.chatId)
    } catch (error) {
      if (origin === session) effects.onError?.(error)
    } finally {
      if (approvalToken) removeApproval(origin.chatId, approvalToken)
      if (origin === session && !runs.has(origin.chatId)) {
        update({ ...transitionConversation(state, { type: 'run_cancelled' }),
          streamingConversationId: null, activeTurnId: null, sessionAutoApproved: false })
      }
    }
  }

  async function sendPrompt(prompt: string, options: PromptOptions = {}) {
    if (state.sending) return
    const origin = session
    const run: Run = { session: origin, turnId: null, autoApprove: false, progress: [], queuedProgress: [] }
    runs.set(origin.chatId, run)
    const images = options.imageAttachments ?? []
    const videos = options.videoAttachments ?? []
    const agent = options.agentMode || options.forceAgentMode || prompt.toLowerCase().startsWith('search web:')
    const workspace = state.workspacePath || null
    update({ ...transitionConversation(state, { type: 'run_started', message: prompt, imageCount: images.length, videoCount: videos.length }),
      streamingConversationId: origin.chatId, activeTurnId: null, sessionAutoApproved: false })
    if (options.clearComposer) dispatch({ type: 'composer_cleared' })
    try {
      const response = await transport.streamChatMessage(
        agent ? prompt : `/chat ${prompt}`, () => {}, images.map(img => img.dataUri).join(' '),
        options.audioDataUri ?? null, videos.map(video => video.dataUri).join(' '), options.systemInstruction ?? '',
        progress => {
          if (runs.get(origin.chatId) !== run) return
          run.progress.push(progress)
          if (!currentRun(run)) return
          run.queuedProgress.push(progress)
          if (progress.type !== 'ThinkingDelta') flushProgress(run)
          else if (run.timer === undefined) run.timer = setTimeout(() => flushProgress(run), 75)
        }, options.documentAttachment ?? null, workspace, origin.chatId, undefined,
        agent ? options.planMode : false, options.pinnedMcpServer ?? null,
        approval => { if (runs.get(origin.chatId) === run) receiveApproval(origin.chatId, approval) },
        id => { run.turnId = id; if (currentRun(run)) update({ activeTurnId: id }) },
      )
      flushProgress(run)
      // Reveal only the authoritative completed reply, as on CLI/TUI.
      if (currentRun(run)) update({ streamedReply: response.text, streamedResponse: response })
      await effects.onReply?.(response, origin.chatId)
      const persisted = compactAgentProgressForPersistence(run.progress)
      if (persisted.length && run.turnId !== null) {
        await transport.saveInteractionAgentActivity(run.turnId, persisted)
        if (currentRun(run)) {
          update({ agentActivitySnapshots: { ...state.agentActivitySnapshots, [String(run.turnId)]: persisted.slice() } })
          effects.onActivityPersisted?.(run.turnId)
        }
      }
      if (currentRun(run)) {
        try { await refreshHistory() } catch (error) { effects.onError?.(error) }
      }
      await refreshSessions()
      await effects.onRunCompleted?.()
      if (currentRun(run)) {
        effects.onVisibleRunCompleted?.(origin.chatId)
        dispatch({ type: 'stream_cleared' })
      }
    } catch (error) {
      if (currentRun(run)) effects.onError?.(error)
    } finally {
      flushProgress(run)
      if (runs.get(origin.chatId) === run) {
        removeApproval(origin.chatId)
        if (currentRun(run)) update({ ...transitionConversation(state, { type: 'run_finished' }),
          streamingConversationId: null, activeTurnId: null, sessionAutoApproved: false })
        runs.delete(origin.chatId)
      }
    }
  }

  async function clearHistory() {
    const origin = session
    try {
      await transport.clearChatHistory(origin.chatId)
      if (origin !== session) return
      // Invalidate in-flight snapshots, polls and runs even when the id stays the same.
      const run = runs.get(origin.chatId)
      if (run) clearProgressTimer(run)
      runs.delete(origin.chatId)
      session = { chatId: origin.chatId }
      cursor = null
      removeApproval(origin.chatId)
      update({ ...transitionConversation(state, { type: 'session_switched', draft: '' }),
        interactions: [], hasOlderInteractions: false, agentActivitySnapshots: {},
        streamingConversationId: null, activeTurnId: null, sessionAutoApproved: false })
    } catch (error) {
      if (origin === session) throw error
    }
  }

  async function selectProject(
    choosePath: () => Promise<string | null>,
    navigation: {
      openSession(session: ChatSession, view: 'chat' | 'workspace'): Promise<void>
      newChat(path: string, view: 'chat' | 'workspace'): Promise<void>
      showCurrentView(view: 'chat' | 'workspace'): void
    },
    view: 'chat' | 'workspace',
  ) {
    const origin = session
    const selection = ++projectSerial
    const isCurrent = () => origin === session && selection === projectSerial
    try {
      const path = await choosePath()
      if (path && isCurrent()) await navigateToProject(path, {
        ...navigation, listSessions: refreshSessions, currentSessionId: () => session.chatId, isCurrent,
      }, view)
    } catch (error) {
      if (isCurrent()) effects.onError?.(error)
    }
  }

  function watch(browser: ConversationBrowser, historyVisible: boolean) {
    let active = true
    const visible = () => active && browser.document.visibilityState === 'visible'
    const sync = () => { if (historyVisible && visible()) void syncHistory(visible) }
    const refresh = () => { if (visible()) void refreshSessions().catch(() => {}) }
    const focus = () => { sync(); refresh() }
    sync()
    const historyTimer = historyVisible ? browser.window.setInterval(sync, 1500) : undefined
    const sessionsTimer = browser.window.setInterval(refresh, 5000)
    browser.window.addEventListener('focus', focus)
    browser.document.addEventListener('visibilitychange', focus)
    return () => {
      active = false
      if (historyTimer !== undefined) browser.window.clearInterval(historyTimer)
      browser.window.clearInterval(sessionsTimer)
      browser.window.removeEventListener('focus', focus)
      browser.document.removeEventListener('visibilitychange', focus)
    }
  }

  return {
    getSnapshot: () => state,
    subscribe: (notify: () => void) => { subscribers.add(notify); return () => { subscribers.delete(notify) } },
    activate, refreshHistory, refreshSessions, syncHistory, loadOlder, sendPrompt, cancel,
    receiveApproval, resolveApproval, clearHistory, selectProject, watch,
    appendInteraction: (row: Omit<InteractionMemory, 'chatId' | 'status'> & Partial<Pick<InteractionMemory, 'chatId' | 'status'>>) =>
      update({ interactions: [...state.interactions, { chatId: session.chatId, status: 'completed', ...row }] }),
    compose: (message: string) => dispatch({ type: 'compose', message }),
    selectWorkspace: (path: string) => dispatch({ type: 'workspace_selected', path }),
    attachImage: (image: ConversationState['imageAttachments'][number]) => dispatch({ type: 'image_attached', image }),
    removeImage: (index: number) => dispatch({ type: 'image_removed', index }),
    attachVideo: (video: ConversationState['videoAttachments'][number]) => dispatch({ type: 'video_attached', video }),
    removeVideo: (index: number) => dispatch({ type: 'video_removed', index }),
    attachDocument: (document: ConversationState['documentAttachment']) => dispatch({ type: 'document_attached', document }),
    clearComposer: () => dispatch({ type: 'composer_cleared' }),
  }
}
