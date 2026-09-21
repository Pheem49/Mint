import { describe, expect, it, vi } from 'vitest'
import { authPlatform, installRendererPlatform, installWorkspacePlatform, workspacePlatform, type WorkspacePlatform } from './platform'
import * as desktopAdapter from '../src/tauri'
import * as webAdapter from '../src-web/tauri'

describe('renderer platform interfaces', () => {
  it('accepts both complete runtime adapters', () => {
    expect(() => installRendererPlatform(desktopAdapter)).not.toThrow()
    expect(() => installRendererPlatform(webAdapter)).not.toThrow()
  })
  it('routes grouped capabilities through the installed runtime adapter', async () => {
    const authGetCurrentUser = vi.fn(async () => ({ id: '1', name: 'Mint', email: null, image: null }))
    installRendererPlatform(new Proxy({ APP_ICON_PATH: './icon.png', authGetCurrentUser }, {
      get: (target, property) => property in target ? target[property as keyof typeof target] : vi.fn(),
    }) as any)
    await expect(authPlatform.authGetCurrentUser()).resolves.toMatchObject({ id: '1', name: 'Mint' })
    expect(authGetCurrentUser).toHaveBeenCalledOnce()
  })

  it('routes workspace operations as one request and returns a snapshot', async () => {
    const operation = { root: '/workspace', relativePath: 'src/main.ts', revision: 7 }
    const snapshot = { path: '/workspace', revision: 8, tree: { name: 'workspace', path: '.', kind: 'directory', children: [] },
      git: { isRepository: false, currentBranch: null, detachedHead: null, branches: [], remoteBranches: [], isDirty: false } }
    const createWorkspaceFile = vi.fn(async () => snapshot)
    const adapter = {
      getWorkspaceSnapshot: vi.fn(), getGitBranchInfo: vi.fn(), switchGitBranch: vi.fn(),
      createGitBranch: vi.fn(), checkoutRemoteGitBranch: vi.fn(), getGitGraph: vi.fn(),
      createWorkspaceFile, createWorkspaceFolder: vi.fn(), deleteWorkspaceItem: vi.fn(),
    } satisfies WorkspacePlatform
    installWorkspacePlatform(adapter)
    await expect(workspacePlatform.createWorkspaceFile(operation)).resolves.toEqual(snapshot)
    expect(createWorkspaceFile).toHaveBeenCalledWith(operation)
  })
})
