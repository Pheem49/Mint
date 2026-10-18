import React, { useState } from 'react'
import { createRoot } from 'react-dom/client'
import WorkspacePanel from '../src/components/WorkspacePanel'
import { installWorkspacePlatform } from '../shared/platform'
import type { WorkspaceSnapshot } from '../shared/types'

const roots = ['/tmp/mint-expansion-a', '/tmp/mint-expansion-b']
for (const root of roots) localStorage.removeItem(`mint:workspace-expanded-folders:${encodeURIComponent(root)}`)
const snapshot = (root: string): WorkspaceSnapshot => ({ path: root, revision: 1,
  tree: { name: root.split('/').at(-1)!, path: '.', kind: 'directory', children: [
    { name: 'docs', path: 'docs', kind: 'directory', children: [
      { name: 'nested', path: 'docs/nested', kind: 'directory', children: [
        { name: 'note.txt', path: 'docs/nested/note.txt', kind: 'file', children: [] },
      ] },
    ] },
  ] },
  git: { isRepository: false, currentBranch: null, isDirty: false, branches: [], remoteBranches: [], detachedHead: null },
})
installWorkspacePlatform({
  getWorkspaceSnapshot: async operation => snapshot(operation.root),
  listWorkspaceHistory: async () => [],
} as any)
const noop = () => {}
function Fixture() {
  const [root, setRoot] = useState(roots[0])
  const [mounted, setMounted] = useState(true)
  return <>
    <button id="switch" onClick={() => setRoot(value => value === roots[0] ? roots[1] : roots[0])}>Switch workspace</button>
    <button id="mount" onClick={() => setMounted(value => !value)}>Toggle view</button>
    {mounted && <WorkspacePanel workspacePath={root} agentMode sending={false} onEnableAgentMode={noop} onSetMessage={noop} onWorkspaceReady={noop} />}
  </>
}
createRoot(document.getElementById('root')!).render(<Fixture />)
const delay = () => new Promise(resolve => setTimeout(resolve, 30))
async function until(check: () => boolean) { for (let i = 0; i < 100; i++) { if (check()) return; await delay() } throw Error('Timed out') }
const row = (path: string) => document.querySelector<HTMLButtonElement>(`.workspace-tree-row[title="${path}"]`)
async function click(selector: string) { document.querySelector<HTMLButtonElement>(selector)!.click(); await delay() }
const checks: string[] = []
function check(value: unknown, name: string) { if (!value) throw Error(name); checks.push(name) }
try {
  await until(() => Boolean(row('docs')))
  check(row('docs')?.getAttribute('aria-expanded') === 'false' && !row('docs/nested'), 'first visit starts with every child folder collapsed')
  await click('[title="docs"]')
  check(row('docs/nested')?.getAttribute('aria-expanded') === 'false' && !row('docs/nested/note.txt'), 'opening a parent keeps unvisited descendants collapsed')
  await click('[title="docs/nested"]')
  check(row('docs/nested/note.txt'), 'nested folder can be expanded')
  await click('[title="docs"]'); await click('[title="docs"]')
  check(row('docs/nested/note.txt'), 'collapsing a parent preserves its nested expansion state')
  await click('#switch')
  await until(() => document.querySelector('.workspace-root-row')?.textContent?.includes('mint-expansion-b') === true)
  check(!row('docs/nested'), 'another workspace with the same relative paths starts collapsed')
  await click('[title="docs"]')
  await click('#switch')
  await until(() => document.querySelector('.workspace-root-row')?.textContent?.includes('mint-expansion-a') === true)
  check(row('docs/nested/note.txt'), 'returning to a workspace restores its own expanded folders')
  await click('#mount'); await click('#mount')
  await until(() => Boolean(row('docs')))
  check(row('docs/nested/note.txt'), 'reopening Workspace restores saved expansion')
  await click('[title="docs/nested"]')
  await click('#mount'); await click('#mount')
  await until(() => Boolean(row('docs/nested')))
  check(!row('docs/nested/note.txt'), 'reopening also restores an explicitly collapsed folder')
  await click('[aria-label="Refresh workspace"]')
  check(!row('docs/nested/note.txt'), 'snapshot refresh preserves expansion choices')
  document.getElementById('result')!.textContent = JSON.stringify({ status: 'PASS', checks })
} catch (error) { document.getElementById('result')!.textContent = JSON.stringify({ status: 'FAIL', error: String(error), checks }) }
