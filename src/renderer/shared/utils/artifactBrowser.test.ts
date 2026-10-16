import { describe, expect, it, vi } from 'vitest'
import { installRendererPlatform } from '../platform'
import * as desktopAdapter from '../../src/tauri'
import { openArtifactInMintBrowser } from './artifactBrowser'

const native = vi.hoisted(() => ({ invoke: vi.fn(async () => undefined) }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }))

const url = 'http://127.0.0.1:12345/site/index.html'
describe('opening a preview in Mint Browser', () => {
  it('opens an existing served URL through Mint Browser on Desktop', async () => {
    const opened: string[] = []
    await openArtifactInMintBrowser({ path: 'site/index.html', url }, '/projects/Demo', {
      desktop: true,
      startPreview: async () => { throw new Error('An existing URL must be reused') },
      openMintBrowser: async (target) => { opened.push(target) },
    })
    expect(opened).toEqual([url])
  })
  it('serves chat artifacts from their pinned project before opening Mint Browser', async () => {
    const actions: unknown[] = []
    await openArtifactInMintBrowser({ path: 'site/index.html', workspacePath: '/projects/Original' }, '/projects/Other', {
      desktop: true,
      startPreview: async (root, path, mint) => { actions.push([root, path, mint]); return { url, jobId: 'preview-1' } },
      openMintBrowser: async (target) => { actions.push(target) },
    })
    expect(actions).toEqual([['/projects/Original', 'site/index.html', false], url])
  })
  it('opens the controlled browser through the authenticated local backend on Web', async () => {
    const requests: unknown[] = []
    await openArtifactInMintBrowser({ path: 'site/index.html', url, workspacePath: '/projects/Demo' }, undefined, {
      desktop: false,
      startPreview: async (root, path, mint) => { requests.push([root, path, mint]); return { url, jobId: 'preview-1' } },
      openMintBrowser: async () => { throw new Error('Web must not call native IPC') },
    })
    expect(requests).toEqual([['/projects/Demo', 'site/index.html', true]])
  })
  it('reports browser launch failures so the button can show an error', async () => {
    await expect(openArtifactInMintBrowser({ path: 'site/index.html', url }, undefined, {
      desktop: true,
      startPreview: async () => { throw new Error('Unexpected server startup') },
      openMintBrowser: async () => { throw new Error('Chrome unavailable') },
    })).rejects.toThrow('Chrome unavailable')
  })
  it('does not start a server without a source project', async () => {
    await expect(openArtifactInMintBrowser({ path: 'site/index.html' }, undefined, {
      desktop: true,
      startPreview: async () => { throw new Error('Unexpected server startup') },
      openMintBrowser: async () => { throw new Error('Unexpected browser startup') },
    })).rejects.toThrow('Select a project')
  })
})

it('sends the existing open_mint_browser IPC command for the default Desktop action', async () => {
  installRendererPlatform({ ...desktopAdapter, isTauriRuntime: () => true })
  native.invoke.mockClear()
  await openArtifactInMintBrowser({ path: 'site/index.html', url }, undefined)
  expect(native.invoke).toHaveBeenCalledWith('open_mint_browser', { url })
})

it.each([
  ['/projects/Demo', '/projects/Demo/site/index.html', 'site/index.html'],
  ['C:\\Projects\\Demo', 'c:\\projects\\demo\\site\\index.html', 'site/index.html'],
])('opens absolute artifact paths relative to their source project %s', async (root, path, relative) => {
  const requests: unknown[] = []
  await openArtifactInMintBrowser({ path, workspacePath: root }, '/projects/Other', {
    desktop: true,
    startPreview: async (workspace, file, mint) => { requests.push([workspace, file, mint]); return { url, jobId: 'preview-1' } },
    openMintBrowser: async () => {},
  })
  expect(requests).toEqual([[root, relative, false]])
})

it('rejects an absolute artifact from a different project without starting a server', async () => {
  await expect(openArtifactInMintBrowser({ path: '/projects/Demo-copy/index.html', workspacePath: '/projects/Demo' }, undefined, {
    desktop: true,
    startPreview: async () => { throw new Error('Unexpected server startup') },
    openMintBrowser: async () => {},
  })).rejects.toThrow('outside its source project')
})
