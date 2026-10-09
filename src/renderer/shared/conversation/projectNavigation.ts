import type { ChatSession } from '../types'
import { normalizeWorkspacePath } from '../utils/workspaces'

interface ProjectNavigation {
  listSessions(): Promise<ChatSession[]>
  currentSessionId(): string
  openSession(session: ChatSession, view: 'chat' | 'workspace'): Promise<void>
  newChat(path: string, view: 'chat' | 'workspace'): Promise<void>
  showCurrentView?(view: 'chat' | 'workspace'): void
  isCurrent?(): boolean
}

function sessionTime(session: ChatSession): number {
  return Date.parse(session.updatedAt) || Date.parse(session.createdAt) || 0
}

/** Navigate using persisted project ownership; selecting a project never moves a chat. */
export async function navigateToProject(path: string, navigation: ProjectNavigation, view: 'chat' | 'workspace' = 'chat'): Promise<void> {
  const targetPath = normalizeWorkspacePath(path)
  if (!targetPath || navigation.isCurrent?.() === false) return
  const sessions = await navigation.listSessions()
  if (navigation.isCurrent?.() === false) return
  const latest = sessions
    .filter(session => session.kind !== 'cli' && !session.id.startsWith('cli')
      && session.id !== 'conversation-default' && !session.id.includes('::subagent::')
      && normalizeWorkspacePath(session.workspacePath) === targetPath)
    .sort((a, b) => sessionTime(b) - sessionTime(a) || Date.parse(b.createdAt) - Date.parse(a.createdAt))[0]
  if (latest) {
    if (latest.id !== navigation.currentSessionId()) await navigation.openSession(latest, view)
    else navigation.showCurrentView?.(view)
  } else {
    await navigation.newChat(targetPath, view)
  }
}
