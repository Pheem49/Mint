import { expect, it } from 'vitest'
import { WorkspaceSnapshotReader } from './workspaceSnapshot'
import type { WorkspaceSnapshot } from '../types'

const snapshot = (path: string): WorkspaceSnapshot => ({ path, revision: 1, git: { isRepository: false, currentBranch: null, detachedHead: null, branches: [], remoteBranches: [], isDirty: false }, tree: { name: path, path: '', kind: 'directory', children: [] } })
const request = (root: string) => ({ root, relativePath: '', revision: 0 })

it('starts the new project load immediately and discards the late previous project snapshot', async () => {
  let finish!: (value: WorkspaceSnapshot) => void
  const reader = new WorkspaceSnapshotReader(async ({ root }) => root === '/old' ? new Promise(resolve => { finish = resolve }) : snapshot(root))
  const old = reader.refresh(request('/old'))
  expect(await reader.refresh(request('/new'))).toMatchObject({ path: '/new' })
  finish(snapshot('/old-canonical'))
  expect(await old).toBeNull()
})
it('does not publish a snapshot or canonical path after the panel unmounts', async () => {
  let finish!: (value: WorkspaceSnapshot) => void
  const reader = new WorkspaceSnapshotReader(() => new Promise(resolve => { finish = resolve }))
  const pending = reader.refresh(request('/old'))
  reader.dispose()
  finish(snapshot('/old-canonical'))
  expect(await pending).toBeNull()
})
it('suppresses errors from superseded requests while reporting the current project failure', async () => {
  let reject!: (error: Error) => void
  const reader = new WorkspaceSnapshotReader(async ({ root }) => {
    if (root === '/old') return new Promise((_resolve, fail) => { reject = fail })
    throw new Error('new project unavailable')
  })
  const old = reader.refresh(request('/old'))
  await expect(reader.refresh(request('/new'))).rejects.toThrow('new project unavailable')
  reject(new Error('old project unavailable'))
  expect(await old).toBeNull()
})
it('deduplicates simultaneous refreshes for one root without dropping later refreshes', async () => {
  let finish!: (value: WorkspaceSnapshot) => void
  let reads = 0
  const reader = new WorkspaceSnapshotReader(() => { reads++; return new Promise(resolve => { finish = resolve }) })
  const first = reader.refresh(request('/project'))
  const duplicate = reader.refresh(request('/project'))
  expect(reads).toBe(1)
  expect(await duplicate).toBeNull()
  finish(snapshot('/project'))
  expect(await first).toMatchObject({ path: '/project' })
  const later = reader.refresh(request('/project'))
  expect(reads).toBe(2)
  finish(snapshot('/project'))
  expect(await later).toMatchObject({ path: '/project' })
})
