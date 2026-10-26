import { afterEach, describe, expect, it, vi } from 'vitest'
import { authPlatform, installRendererPlatform, installWorkspacePlatform, workspacePlatform, type WorkspacePlatform } from './platform'
import * as desktopAdapter from '../src/tauri'
import * as webAdapter from '../src-web/tauri'

afterEach(() => vi.unstubAllGlobals())

describe.each([['Desktop fallback', desktopAdapter], ['Web', webAdapter]] as const)('%s skill inventory', (_name, adapter) => {
  it('forwards the selected workspace and returns skill location metadata', async () => {
    vi.stubGlobal('window', {
      location: { hostname: 'localhost', port: '9000', protocol: 'http:' },
      localStorage: { getItem: () => null },
    })
    let requestUrl: URL | undefined
    const skills = [{ id: 0, name: 'example', sourcePath: '/selected project/skills/example.md',
      content: 'selected workspace', updatedAt: '', createdAt: '', location: 'workspace',
      description: null, is_workspace: true }]
    vi.stubGlobal('fetch', async (input: string) => {
      requestUrl = new URL(input, 'http://localhost:9000')
      return new Response(JSON.stringify(skills), { headers: { 'Content-Type': 'application/json' } })
    })
    await expect(adapter.listLearnedSkills('/selected project')).resolves.toEqual(skills)
    expect(requestUrl?.pathname).toBe('/api/learned-skills')
    expect(requestUrl?.searchParams.get('workspacePath')).toBe('/selected project')
  })
})

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
      getWorkspaceSnapshot: vi.fn(), getWorkspaceGitDiff: vi.fn(), getGitBranchInfo: vi.fn(), switchGitBranch: vi.fn(),
      createGitBranch: vi.fn(), checkoutRemoteGitBranch: vi.fn(), getGitGraph: vi.fn(),
      createWorkspaceFile, createWorkspaceFolder: vi.fn(), deleteWorkspaceItem: vi.fn(),
      moveWorkspaceItem: vi.fn(), listWorkspaceHistory: vi.fn(), undoWorkspaceAction: vi.fn(),
    } satisfies WorkspacePlatform
    installWorkspacePlatform(adapter)
    await expect(workspacePlatform.createWorkspaceFile(operation)).resolves.toEqual(snapshot)
    expect(createWorkspaceFile).toHaveBeenCalledWith(operation)
  })
})

it('routes background terminal controls through both platform adapters', async () => {
  const { backgroundPlatform } = await import('./platform')
  for (const adapter of [desktopAdapter, webAdapter]) {
    expect(adapter.listBackgroundJobs).toBeTypeOf('function')
    expect(adapter.getBackgroundJob).toBeTypeOf('function')
    expect(adapter.stopBackgroundJob).toBeTypeOf('function')
  }
  const job = { id: 'bg-backend-1', status: 'running' }
  const listBackgroundJobs = vi.fn(async () => [job])
  const getBackgroundJob = vi.fn(async () => ({ ...job, stdout: 'hello', stderr: '' }))
  const stopBackgroundJob = vi.fn(async () => ({ ...job, status: 'stopping' }))
  installRendererPlatform(new Proxy({ listBackgroundJobs, getBackgroundJob, stopBackgroundJob }, { get: (target, name) => target[name as keyof typeof target] || vi.fn() }) as any)
  await expect(backgroundPlatform.list('/Demo')).resolves.toEqual([job])
  await expect(backgroundPlatform.output(job.id)).resolves.toMatchObject({ stdout: 'hello' })
  await expect(backgroundPlatform.stop(job.id)).resolves.toMatchObject({ status: 'stopping' })
  expect(listBackgroundJobs).toHaveBeenCalledWith('/Demo')
  expect(getBackgroundJob).toHaveBeenCalledWith(job.id)
  expect(stopBackgroundJob).toHaveBeenCalledWith(job.id)
})

describe.each([['Desktop fallback', desktopAdapter], ['Web', webAdapter]] as const)('%s workspace mods', (_name, adapter) => {
  it('forwards explicit workspace, chat, instance and action identity', async () => {
    vi.stubGlobal('window', { location: { hostname: 'localhost', port: '9000', protocol: 'http:' }, localStorage: { getItem: () => null } })
    let body: unknown
    vi.stubGlobal('fetch', async (_url: unknown, init: RequestInit) => {
      body = JSON.parse(String(init.body))
      return new Response(JSON.stringify({ mods: [] }), { headers: { 'Content-Type': 'application/json' } })
    })
    await adapter.modsOperation({ workspacePath: '/project', chatId: 'chat-a', action: 'action', id: 'checklist', instanceId: 'revision-instance', command: 'add', value: 'Test' })
    expect(body).toEqual({ workspacePath: '/project', chatId: 'chat-a', action: 'action', id: 'checklist', instanceId: 'revision-instance', command: 'add', value: 'Test' })
  })
})

describe.each([['Desktop fallback', desktopAdapter], ['Web', webAdapter]] as const)('%s openExternal', (_name, adapter) => {
  it('exports openExternal function and invokes window.open in browser fallback', async () => {
    const openMock = vi.fn()
    vi.stubGlobal('window', { open: openMock })
    expect(adapter.openExternal).toBeTypeOf('function')
    await adapter.openExternal('https://example.com/test')
    expect(openMock).toHaveBeenCalledWith('https://example.com/test', '_blank', 'noopener,noreferrer')
  })
})

describe('desktop external link interceptor', () => {
  it('identifies external and internal URLs correctly', async () => {
    const { isExternalHref } = await import('./utils/externalLinks')
    expect(isExternalHref('https://example.com')).toBe(true)
    expect(isExternalHref('http://example.com/page')).toBe(true)
    expect(isExternalHref('//cdn.example.com/asset.js')).toBe(true)
    expect(isExternalHref('#/settings')).toBe(false)
    expect(isExternalHref('/library?item=123')).toBe(false)
    expect(isExternalHref('./relative')).toBe(false)
    expect(isExternalHref('../parent')).toBe(false)
    expect(isExternalHref('javascript:void(0)')).toBe(false)
    expect(isExternalHref('data:image/png;base64,...')).toBe(false)
    expect(isExternalHref('blob:http://localhost/123')).toBe(false)
  })

  it('intercepts clicks on external http/https links and prevents default', async () => {
    const openMock = vi.fn()
    vi.stubGlobal('window', { open: openMock })

    let clickHandler: ((e: any) => void) | null = null
    const mockDocument: any = {
      addEventListener: vi.fn((event: string, handler: any) => {
        if (event === 'click') clickHandler = handler
      }),
      removeEventListener: vi.fn((event: string, handler: any) => {
        if (event === 'click' && clickHandler === handler) clickHandler = null
      }),
    }

    const cleanup = desktopAdapter.installExternalLinkInterceptor(mockDocument)
    expect(mockDocument.addEventListener).toHaveBeenCalledWith('click', expect.any(Function), true)
    expect(clickHandler).toBeTypeOf('function')

    const externalAnchor: any = {
      getAttribute: (attr: string) => (attr === 'href' ? 'https://example.com/search-result' : null),
      hasAttribute: (attr: string) => false,
      href: 'https://example.com/search-result',
      target: '_blank',
    }
    const imgInsideAnchor: any = {
      closest: (selector: string) => (selector === 'a' ? externalAnchor : null),
    }

    const clickEvent: any = {
      defaultPrevented: false,
      button: 0,
      target: imgInsideAnchor,
      preventDefault: vi.fn(() => { clickEvent.defaultPrevented = true }),
      stopPropagation: vi.fn(),
    }

    clickHandler!(clickEvent)

    expect(clickEvent.preventDefault).toHaveBeenCalledOnce()
    // Does NOT suppress propagation so React/child event listeners can still run
    expect(clickEvent.stopPropagation).not.toHaveBeenCalled()
    expect(openMock).toHaveBeenCalledWith('https://example.com/search-result', '_blank', 'noopener,noreferrer')

    // Internal relative route (/library?item=lib-123) should NOT be intercepted even if anchor.href resolves to document origin
    openMock.mockClear()
    const relativeAnchor: any = {
      getAttribute: (attr: string) => (attr === 'href' ? '/library?item=lib-123' : null),
      hasAttribute: (attr: string) => false,
      href: 'http://localhost:9000/library?item=lib-123',
      target: '',
    }
    const relativeClickEvent: any = {
      defaultPrevented: false,
      button: 0,
      target: {
        closest: (selector: string) => (selector === 'a' ? relativeAnchor : null),
      },
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
    }

    clickHandler!(relativeClickEvent)

    expect(relativeClickEvent.preventDefault).not.toHaveBeenCalled()
    expect(openMock).not.toHaveBeenCalled()

    // Download link should not be intercepted
    openMock.mockClear()
    const downloadAnchor: any = {
      getAttribute: (attr: string) => (attr === 'href' ? 'https://example.com/file.png' : null),
      hasAttribute: (attr: string) => attr === 'download',
      href: 'https://example.com/file.png',
      target: '',
    }
    const downloadClickEvent: any = {
      defaultPrevented: false,
      button: 0,
      target: {
        closest: (selector: string) => (selector === 'a' ? downloadAnchor : null),
      },
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
    }

    clickHandler!(downloadClickEvent)

    expect(downloadClickEvent.preventDefault).not.toHaveBeenCalled()
    expect(openMock).not.toHaveBeenCalled()

    // Internal hash link should not be intercepted
    openMock.mockClear()
    const internalAnchor: any = {
      getAttribute: (attr: string) => (attr === 'href' ? '#/settings' : null),
      hasAttribute: (attr: string) => false,
      href: '#/settings',
      target: '',
    }
    const internalClickEvent: any = {
      defaultPrevented: false,
      button: 0,
      target: {
        closest: (selector: string) => (selector === 'a' ? internalAnchor : null),
      },
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
    }

    clickHandler!(internalClickEvent)

    expect(internalClickEvent.preventDefault).not.toHaveBeenCalled()
    expect(internalClickEvent.stopPropagation).not.toHaveBeenCalled()
    expect(openMock).not.toHaveBeenCalled()

    cleanup()
    expect(mockDocument.removeEventListener).toHaveBeenCalledWith('click', expect.any(Function), true)
  })
})
