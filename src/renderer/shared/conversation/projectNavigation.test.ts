import { describe, expect, it } from 'vitest'
import type { ChatSession } from '../types'
import { navigateToProject } from './projectNavigation'

function session(id: string, workspacePath: string | null, updatedAt: string, kind = 'conversation'): ChatSession {
  return { id, workspacePath, updatedAt, kind, title: id, createdAt: '2026-10-01T00:00:00Z', messageCount: 2, totalBytes: 20 }
}

function fixture(sessions: ChatSession[]) {
  const state = { activeId: 'conversation-origin', workspace: '/projects/old', draft: 'unfinished text', view: 'chat' };
  const ports = {
    listSessions: async () => sessions,
    currentSessionId: () => state.activeId,
    openSession: async (target: ChatSession, view = 'chat') => { state.activeId = target.id; state.workspace = target.workspacePath || ''; state.draft = ''; state.view = view },
    newChat: async (path: string, view = 'chat') => { state.activeId = 'conversation-new'; state.workspace = path; state.draft = ''; state.view = view },
    showCurrentView: (view: string) => { state.view = view },
  }
  return { state, ports }
}

describe('project selection', () => {
  it('opens the Workspace file tree when the folder action selects an existing project', async () => {
    const { state, ports } = fixture([session('conversation-target', '/projects/new', '2026-10-09T00:00:00Z')])
    await navigateToProject('/projects/new', ports, 'workspace')
    expect(state).toMatchObject({ activeId: 'conversation-target', workspace: '/projects/new', view: 'workspace' })
  })
  it('keeps Workspace visible when the selected project starts a new chat', async () => {
    const { state, ports } = fixture([])
    await navigateToProject('/projects/new', ports, 'workspace')
    expect(state).toMatchObject({ activeId: 'conversation-new', workspace: '/projects/new', view: 'workspace' })
  })
  it('shows Workspace without clearing the draft when its latest conversation is already open', async () => {
    const { state, ports } = fixture([session('conversation-origin', '/projects/old', '2026-10-09T00:00:00Z')])
    await navigateToProject('/projects/old', ports, 'workspace')
    expect(state).toMatchObject({ activeId: 'conversation-origin', draft: 'unfinished text', view: 'workspace' })
  })
  it('opens the latest conversation in the selected project without moving the current conversation', async () => {
    const rows = [session('conversation-origin', '/projects/old', '2026-10-09T00:00:00Z'), session('conversation-older', '/projects/new', '2026-10-07T00:00:00Z'), session('conversation-latest', '/projects/new', '2026-10-08T00:00:00Z')]
    const { state, ports } = fixture(rows)
    await navigateToProject('/projects/new', ports)
    expect(rows[0].workspacePath).toBe('/projects/old')
    expect(state).toMatchObject({ activeId: 'conversation-latest', workspace: '/projects/new', view: 'chat' })
  })

  it('opens a new chat for a project with no conversations and leaves an unassigned old chat unassigned', async () => {
    const rows = [session('conversation-origin', null, '2026-10-09T00:00:00Z')]
    const { state, ports } = fixture(rows)
    await navigateToProject('/projects/empty', ports)
    expect(rows[0].workspacePath).toBeNull()
    expect(state).toMatchObject({ activeId: 'conversation-new', workspace: '/projects/empty', view: 'chat', draft: '' })
  })
  it('matches normalized project paths and ignores newer CLI and nested subagent sessions', async () => {
    const rows = [
      session('conversation-origin', '/projects/old', '2026-10-09T00:00:00Z'),
      session('cli-latest', '/projects/new', '2026-10-10T00:00:00Z', 'cli'),
      session('conversation-parent::subagent::worker', '/projects/new', '2026-10-10T00:00:00Z'),
      session('conversation-target', '/projects/new/', '2026-10-08T00:00:00Z'),
    ]
    const { state, ports } = fixture(rows)
    await navigateToProject(' /projects/new/ ', ports)
    expect(state.activeId).toBe('conversation-target')
    expect(rows[0].workspacePath).toBe('/projects/old')
  })

  it('opens a new chat when only CLI or internal sessions exist for the project', async () => {
    const { state, ports } = fixture([
      session('conversation-origin', '/projects/old', '2026-10-09T00:00:00Z'),
      session('cli', '/projects/new', '2026-10-10T00:00:00Z', 'cli'),
    ])
    await navigateToProject('/projects/new/', ports)
    expect(state).toMatchObject({ activeId: 'conversation-new', workspace: '/projects/new', view: 'chat' })
  })

  it('keeps the current draft when the latest target session is already open', async () => {
    const { state, ports } = fixture([session('conversation-origin', '/projects/old', '2026-10-09T00:00:00Z')])
    await navigateToProject('/projects/old', ports)
    expect(state).toMatchObject({ activeId: 'conversation-origin', draft: 'unfinished text', view: 'chat' })
  })

  it('ignores a slow earlier selection after the user chooses another project', async () => {
    const { state, ports } = fixture([session('conversation-origin', '/projects/old', '2026-10-09T00:00:00Z')])
    let finish!: (rows: ChatSession[]) => void
    const slow = new Promise<ChatSession[]>(resolve => { finish = resolve })
    let selection = 1
    const earlier = navigateToProject('/projects/first', { ...ports, listSessions: () => slow, isCurrent: () => selection === 1 })
    selection = 2
    await navigateToProject('/projects/last', ports)
    finish([session('conversation-first', '/projects/first', '2026-10-09T00:00:00Z')])
    await earlier
    expect(state).toMatchObject({ activeId: 'conversation-new', workspace: '/projects/last' })
  })

  it('leaves the current conversation intact when session discovery fails', async () => {
    const { state, ports } = fixture([session('conversation-origin', '/projects/old', '2026-10-09T00:00:00Z')])
    await expect(navigateToProject('/projects/new', { ...ports, listSessions: async () => { throw new Error('offline') } })).rejects.toThrow('offline')
    expect(state).toMatchObject({ activeId: 'conversation-origin', workspace: '/projects/old', draft: 'unfinished text' })
  })

})
