import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import CliSessionsView from './CliSessionsView'
import type { ChatSession } from '../types'

const demo = '/home/pheem49/vscode/Project/Demo'
const mint = '/home/pheem49/vscode/Project/Mint-CLI'
const session = (id: string, kind: string, workspacePath?: string): ChatSession => ({
  id, kind, workspacePath, title: `Task ${id}`,
  createdAt: '2026-10-05T15:10:24Z', updatedAt: '2026-10-05T15:10:24Z',
  messageCount: 1, totalBytes: 100,
})

function render(chatSessions: ChatSession[], workspacePath?: string, recentWorkspacePaths?: string[]) {
  return renderToStaticMarkup(<CliSessionsView
    chatSessions={chatSessions}
    workspacePath={workspacePath}
    recentWorkspacePaths={recentWorkspacePaths}
    activeConversationId="conversation-demo"
    onSelectSession={() => {}}
    onDeleteSession={() => {}}
    onRenameSession={() => {}}
  />)
}

describe('Code hub Current Project', () => {
  it('shows only CLI sessions in the selected project and counts the visible scope', () => {
    const html = render([
      session('conversation-demo', 'conversation', demo),
      session('cli::demo', 'cli', demo),
      session('cli::mint', 'cli', mint),
      session('conversation-mint', 'conversation', mint),
    ], demo)
    expect(html).not.toContain('Task conversation-demo')
    expect(html).toContain('Task cli::demo')
    expect(html).not.toContain('Task cli::mint')
    expect(html).not.toContain('Task conversation-mint')
    expect(html).toMatch(/code-header-count[^>]*>1 session</)
    expect(html).not.toContain('mint --resume conversation-demo')
  })

  it('identifies a project without CLI sessions even when other repositories have sessions', () => {
    const html = render([
      session('conversation-demo', 'conversation', demo),
      session('cli::mint', 'cli', mint),
    ], demo)
    expect(html).toMatch(/code-header-count[^>]*>0 sessions</)
    expect(html).toContain('No CLI sessions found in Demo.')
    expect(html).toContain(`title="${demo}"`)
  })

  it('offers projects from app conversations and recent folders even without CLI sessions', () => {
    const html = render([
      session('conversation-demo', 'conversation', demo),
      session('cli::mint', 'cli', mint),
    ], mint, ['/projects/Empty', `${mint}/`])
    expect(html).toContain('aria-label="Project"')
    expect(html).toContain(`value="${demo}"`)
    expect(html).toContain('Project: Demo')
    expect(html).toContain('Project: Mint-CLI')
    expect(html).toContain('Project: Empty')
    expect(html.match(new RegExp(`value="${mint}"`, 'g'))).toHaveLength(1)
  })

  it('counts CLI sessions outside an empty project and offers to show them all', () => {
    const html = render([
      session('conversation-demo', 'conversation', demo),
      ...Array.from({ length: 37 }, (_, i) => session(`cli::mint-${i}`, 'cli', mint)),
    ], demo)
    expect(html).toContain('37 CLI sessions outside this project')
    expect(html).toMatch(/>Show all sessions<\/button>/)
  })

  it('distinguishes projects with the same folder name by their full paths', () => {
    const html = render([session('cli::demo', 'cli', demo)], demo, ['/another/Demo'])
    expect(html).toContain(`Project: ${demo}`)
    expect(html).toContain('Project: /another/Demo')
  })

  it('does not offer a show-all empty-state action when no CLI sessions exist', () => {
    const html = render([session('conversation-demo', 'conversation', demo)], demo)
    expect(html).not.toMatch(/>Show all sessions<\/button>/)
    expect(html).not.toContain('CLI sessions outside this project')
  })

  it('does not mix unassigned sessions into a selected project', () => {
    const html = render([
      session('cli::global', 'cli'),
      session('conversation-global', 'conversation'),
      session('cli::demo', 'cli', demo),
    ], demo)
    expect(html).toContain('Task cli::demo')
    expect(html).not.toContain('Task cli::global')
    expect(html).not.toContain('Task conversation-global')
  })

  it.each([
    [demo, ` ${demo}/ `],
    ['C:\\Projects\\Demo', 'C:/Projects/Demo/'],
  ])('matches equivalent workspace spellings: %s', (stored, selected) => {
    expect(render([session('cli::demo', 'cli', stored)], selected)).toContain('Task cli::demo')
  })

  it('keeps distinct directories and Linux case differences separate', () => {
    const html = render([
      session('cli::prefix', 'cli', `${demo}-other`),
      session('cli::case', 'cli', demo.toLowerCase()),
    ], demo)
    expect(html).not.toContain('Task cli::prefix')
    expect(html).not.toContain('Task cli::case')
  })

  it('requests a project selection instead of showing every session when no workspace is selected', () => {
    const html = render([session('cli::demo', 'cli', demo)])
    expect(html).not.toContain('Task cli::demo')
    expect(html).toContain('Select a project')
  })
})
