import { describe, expect, it } from 'vitest'
import { createWorkspacePreview, createWorkspacePreviewOpener } from './workspacePreview'
import { parseWorkspaceFileDrag, workspaceFileUrl, workspacePreviewId } from './workspaceDrag'
import { createWorkspaceImageReader } from './workspaceImage'
import { createWorkspaceMediaReader } from './workspaceMedia'
import { PreviewObjectUrls } from './previewObjectUrls'
import { readPdfRange } from './pdfRange'

const root = '/projects/Demo'
describe('direct media resources', () => {
  it('reads authenticated Web media into a Blob rather than Base64', async () => {
    let saved: Blob | undefined
    const read = createWorkspaceMediaReader({
      isNative: () => false, apiBase: () => 'http://localhost/api', convertFileSrc: () => { throw Error('Not native') },
      fetch: async input => {
        const url = new URL(String(input)); expect(url.pathname).toBe('/api/file/media'); expect(url.searchParams.get('workspace')).toBe(root)
        return new Response(new Uint8Array([0, 255, 128]), { headers: { 'Content-Type': 'audio/wav', 'Content-Length': '3' } })
      }, createObjectURL: blob => { saved = blob; return 'blob:audio' },
    })
    expect(await read('sound.wav', root)).toEqual({ url: 'blob:audio', mimeType: 'audio/wav', byteLength: 3, objectUrl: true })
    expect(new Uint8Array(await saved!.arrayBuffer())).toEqual(new Uint8Array([0, 255, 128]))
  })
  it('rejects oversized Web responses before allocating a Blob', async () => {
    const read = createWorkspaceMediaReader({ isNative: () => false, apiBase: () => '/api', convertFileSrc: path => path, fetch: async () => new Response('', { headers: { 'Content-Type': 'video/mp4', 'Content-Length': String(257 * 1024 * 1024) } }), createObjectURL: () => { throw Error('Must not allocate') } })
    await expect(read('large.mp4', root)).rejects.toThrow('256 MiB')
  })
  it('releases a shared Blob only after the last preview tab is closed, including late loads after unmount', () => {
    const revoked: string[] = [], owner = new PreviewObjectUrls(url => revoked.push(url))
    const file = { path: 'sound.wav', url: 'blob:audio', objectUrl: true }
    expect(owner.accept(file)).toBe(true)
    owner.update([file, file]); owner.update([file]); expect(revoked).toEqual([])
    owner.update([]); expect(revoked).toEqual(['blob:audio'])
    owner.dispose(); expect(owner.accept({ ...file, url: 'blob:late' })).toBe(false)
    expect(revoked).toEqual(['blob:audio', 'blob:late'])
  })
  it('assembles partial PDF byte ranges when the native file protocol returns shorter chunks', async () => {
    const requests: string[] = []
    const data = await readPdfRange('asset://localhost/book.pdf', 2, 9, new AbortController().signal, async (_url, init) => {
      const header = new Headers(init?.headers).get('Range')!; requests.push(header)
      return header === 'bytes=2-8' ? new Response(new Uint8Array([2, 3, 4]), { status: 206, headers: { 'Content-Range': 'bytes 2-4/10' } }) : new Response(new Uint8Array([5, 6, 7, 8]), { status: 206, headers: { 'Content-Range': 'bytes 5-8/10' } })
    })
    expect(data).toEqual(new Uint8Array([2, 3, 4, 5, 6, 7, 8])); expect(requests).toEqual(['bytes=2-8', 'bytes=5-8'])
  })
})

describe('direct image transport', () => {
  it('reads through the existing API with the original workspace and encoded file path', async () => {
    const read = createWorkspaceImageReader({
      isNative: () => false, apiBase: () => 'http://localhost:3000/api',
      fetch: async input => {
        const url = new URL(String(input))
        expect(url.pathname).toBe('/api/file/image')
        expect(url.searchParams.get('workspace')).toBe(root)
        expect(url.searchParams.get('path')).toBe('images/รูป #1.png')
        return new Response(JSON.stringify({ dataUrl: 'data:image/png;base64,AP+AAQ==' }))
      },
    })
    expect(await read('images/รูป #1.png', root)).toBe('data:image/png;base64,AP+AAQ==')
  })
  it('surfaces a denied or unavailable image read', async () => {
    const read = createWorkspaceImageReader({ isNative: () => false, apiBase: () => '/api', fetch: async () => new Response(JSON.stringify({ error: 'Image unavailable' }), { status: 400 }) })
    await expect(read('missing.png', root)).rejects.toThrow('Image unavailable')
  })
})

describe('workspace preview loading', () => {
  for (const [path, type, mimeType] of [['guide.pdf', 'pdf', 'application/pdf'], ['song.mp3', 'audio', 'audio/mpeg'], ['clip.mp4', 'video', 'video/mp4'], ['clip.webm', 'video', 'video/webm']]) {
    it(`opens ${path} directly without a preview server or text decoding`, async () => {
      const artifact = await createWorkspacePreview(root, path, {
        readWorkspaceFile: async () => { throw Error('Do not decode media as text') },
        readWorkspaceMedia: async (file, workspace) => { expect([file, workspace]).toEqual([path, root]); return { url: 'blob:preview-media', mimeType, objectUrl: true } },
        startHtmlPreview: async () => { throw Error('Media must not start a service') },
      })
      expect(artifact).toEqual({ path, workspacePath: root, type, url: 'blob:preview-media', mimeType, objectUrl: true })
    })
  }
  for (const path of ['src/main.py', 'notes.txt', 'config.toml', '.env', 'Dockerfile']) {
    it(`reads ${path} as text without starting a preview server`, async () => {
      const artifact = await createWorkspacePreview(root, path, {
        readWorkspaceFile: async () => 'file contents',
        startHtmlPreview: async () => { throw Error('Text must not start a server') },
      })
      expect(artifact).toEqual({ path, workspacePath: root, type: 'code', content: 'file contents' })
    })
  }
  it('shows unsupported file information without trying to decode binary content', async () => {
    const artifact = await createWorkspacePreview(root, 'archive.zip', {
      readWorkspaceFile: async () => { throw Error('Do not decode binaries') },
      startHtmlPreview: async () => { throw Error('Do not serve unsupported files') },
    })
    expect(artifact).toEqual({ path: 'archive.zip', workspacePath: root, type: 'file' })
  })
  it('reads Markdown from the selected root and pins that root to the tab', async () => {
    const artifact = await createWorkspacePreview(root, 'docs/README.md', {
      readWorkspaceFile: async (path, workspace) => { expect([path, workspace]).toEqual(['docs/README.md', root]); return '# Hello\n\n**Mint**' },
      startHtmlPreview: async () => { throw new Error('Markdown should not start a server') },
    })
    expect(artifact).toEqual({ path: 'docs/README.md', workspacePath: root, type: 'markdown', content: '# Hello\n\n**Mint**' })
  })
  for (const path of ['assets/photo.png', 'assets/icon.svg']) {
    it(`reads ${path} directly without a background preview service`, async () => {
      const artifact = await createWorkspacePreview(root, path, {
        readWorkspaceFile: async () => { throw Error('Do not decode images as text') },
        readWorkspaceImage: async (file, workspace) => {
          expect([file, workspace]).toEqual([path, root])
          return 'data:image/png;base64,iVBORw=='
        },
        startHtmlPreview: async () => { throw Error('Images must not start a preview service') },
      })
      expect(artifact.url).toBe('data:image/png;base64,iVBORw==')
      expect(artifact.workspacePath).toBe(root)
    })
  }
  for (const path of ['site/index.html']) {
    it(`serves ${path} without reading it as plain text before opening`, async () => {
      const artifact = await createWorkspacePreview(root, path, {
        readWorkspaceFile: async () => { throw new Error('The panel loads text separately; binary images must not be read as UTF-8') },
        startHtmlPreview: async (workspace, file) => { expect([workspace, file]).toEqual([root, path]); return { url: `http://127.0.0.1:12345/${path}`, jobId: 'preview-1' } },
      })
      expect(artifact.path).toBe(path)
      expect((artifact as any).workspacePath).toBe(root)
      expect((artifact as any).url).toBe(`http://127.0.0.1:12345/${path}`)
    })
  }
})

describe('workspace file drops', () => {
  it('keeps file ownership and canonical tab identity across equivalent paths', () => {
    expect(parseWorkspaceFileDrag(JSON.stringify({ root: '/projects/Demo/', path: './docs/notes.md', kind: 'file' }))).toEqual({ root, path: 'docs/notes.md', kind: 'file' })
    expect(workspacePreviewId(root, 'docs/notes.md')).toBe(workspacePreviewId(`${root}/`, './docs/notes.md'))
    expect(workspacePreviewId('/projects/Other', 'docs/notes.md')).not.toBe(workspacePreviewId(root, 'docs/notes.md'))
  })
  for (const value of ['not JSON', 'null', JSON.stringify({ root, path: 'docs', kind: 'directory' }), JSON.stringify({ root, path: '../outside.txt', kind: 'file' }), JSON.stringify({ root, path: '/etc/passwd', kind: 'file' })]) {
    it(`rejects invalid file drop ${value}`, () => { expect(() => parseWorkspaceFileDrag(value)).toThrow() })
  }
  it('encodes external file URLs without treating filename punctuation as URL instructions', () => {
    expect(workspaceFileUrl('/projects/Demo', 'รูป #100%.zip')).toBe('file:///projects/Demo/%E0%B8%A3%E0%B8%B9%E0%B8%9B%20%23100%25.zip')
    expect(workspaceFileUrl('C:\\Projects\\Demo', 'a b.zip')).toBe('file:///C:/Projects/Demo/a%20b.zip')
  })
  it('coalesces simultaneous drops and selects an existing tab without reading or serving it again', async () => {
    let resolve!: (value: string) => void
    let calls = 0, opened = 0, selected = 0
    const tabs = new Set<string>()
    const open = createWorkspacePreviewOpener({
      readWorkspaceFile: async () => { throw Error('Image must not be read as text') },
      readWorkspaceImage: async () => { calls++; return new Promise(value => { resolve = value }) },
      startHtmlPreview: async () => { throw Error('Images must not start a server') },
      selectIfOpen: id => { if (!tabs.has(id)) return false; selected++; return true },
      open: file => { opened++; tabs.add(workspacePreviewId(file.workspacePath!, file.path)) },
    })
    const first = open(root, 'photo.png'), second = open(root, './photo.png')
    resolve('data:image/png;base64,iVBORw==')
    await Promise.all([first, second])
    await open(root, 'photo.png')
    expect([calls, opened, selected]).toEqual([1, 1, 2])
  })
  it('allows retrying a failed drop', async () => {
    let failed = true, opened = 0
    const open = createWorkspacePreviewOpener({
      readWorkspaceFile: async () => { if (failed) throw Error('File unavailable'); return 'recovered' },
      startHtmlPreview: async () => { throw Error('Text must not start a server') },
      selectIfOpen: () => false, open: () => { opened++ },
    })
    await expect(open(root, 'notes.txt')).rejects.toThrow('File unavailable')
    failed = false
    await open(root, 'notes.txt')
    expect(opened).toBe(1)
  })
})
