import { describe, expect, it, vi } from 'vitest'
import { installRendererPlatform } from '../platform'
import * as desktopAdapter from '../../src/tauri'
import { openArtifactInBrowser } from './artifactBrowser'

const native = vi.hoisted(() => ({ invoke: vi.fn(async () => undefined) }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }))

const url = 'http://127.0.0.1:12345/site/index.html'
describe('opening a preview in system browser', () => {
  it('opens an existing served URL through browser on Desktop', async () => {
    const opened: string[] = []
    await openArtifactInBrowser({ path: 'site/index.html', url }, '/projects/Demo', {
      desktop: true,
      startPreview: async () => { throw new Error('An existing URL must be reused') },
      openBrowser: async (target) => { opened.push(target) },
    })
    expect(opened).toEqual([url])
  })
  it('serves chat artifacts from their pinned project before opening browser', async () => {
    const actions: unknown[] = []
    await openArtifactInBrowser({ path: 'site/index.html', workspacePath: '/projects/Original' }, '/projects/Other', {
      desktop: true,
      startPreview: async (root, path) => { actions.push([root, path]); return { url, jobId: 'preview-1' } },
      openBrowser: async (target) => { actions.push(target) },
    })
    expect(actions).toEqual([['/projects/Original', 'site/index.html'], url])
  })
  it('opens preview via system browser on Web', async () => {
    const actions: unknown[] = []
    await openArtifactInBrowser({ path: 'site/index.html', url, workspacePath: '/projects/Demo' }, undefined, {
      desktop: false,
      startPreview: async (root, path) => { actions.push([root, path]); return { url, jobId: 'preview-1' } },
      openBrowser: async (target) => { actions.push(target) },
    })
    expect(actions).toEqual([url])
  })
  it('reports browser launch failures so the button can show an error', async () => {
    await expect(openArtifactInBrowser({ path: 'site/index.html', url }, undefined, {
      desktop: true,
      startPreview: async () => { throw new Error('Unexpected server startup') },
      openBrowser: async () => { throw new Error('Browser unavailable') },
    })).rejects.toThrow('Browser unavailable')
  })
  it('does not start a server without a source project', async () => {
    await expect(openArtifactInBrowser({ path: 'site/index.html' }, undefined, {
      desktop: true,
      startPreview: async () => { throw new Error('Unexpected server startup') },
      openBrowser: async () => { throw new Error('Unexpected browser startup') },
    })).rejects.toThrow('Select a project')
  })
})

it('sends the open_url desktop action for the default Desktop action', async () => {
  vi.stubGlobal('window', { __TAURI_INTERNALS__: {} })
  try {
    installRendererPlatform({ ...desktopAdapter, isTauriRuntime: () => true })
    native.invoke.mockClear()
    await openArtifactInBrowser({ path: 'site/index.html', url }, undefined)
    expect(native.invoke).toHaveBeenCalledWith('run_desktop_action', { action: { type: 'open_url', target: url } })
  } finally {
    vi.unstubAllGlobals()
  }
})

it.each([
  ['/projects/Demo', '/projects/Demo/site/index.html', 'site/index.html'],
  ['C:\\Projects\\Demo', 'c:\\projects\\demo\\site\\index.html', 'site/index.html'],
])('opens absolute artifact paths relative to their source project %s', async (root, path, relative) => {
  const requests: unknown[] = []
  await openArtifactInBrowser({ path, workspacePath: root }, '/projects/Other', {
    desktop: true,
    startPreview: async (workspace, file) => { requests.push([workspace, file]); return { url, jobId: 'preview-1' } },
    openBrowser: async () => {},
  })
  expect(requests).toEqual([[root, relative]])
})

it('rejects an absolute artifact from a different project without starting a server', async () => {
  await expect(openArtifactInBrowser({ path: '/projects/Demo-copy/index.html', workspacePath: '/projects/Demo' }, undefined, {
    desktop: true,
    startPreview: async () => { throw new Error('Unexpected server startup') },
    openBrowser: async () => {},
  })).rejects.toThrow('outside its source project')
})
