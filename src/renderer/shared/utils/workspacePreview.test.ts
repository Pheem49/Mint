import { describe, expect, it } from 'vitest'
import { createWorkspacePreview } from './workspacePreview'

const root = '/projects/Demo'
describe('workspace preview loading', () => {
  it('reads Markdown from the selected root and pins that root to the tab', async () => {
    const artifact = await createWorkspacePreview(root, 'docs/README.md', {
      readWorkspaceFile: async (path, workspace) => { expect([path, workspace]).toEqual(['docs/README.md', root]); return '# Hello\n\n**Mint**' },
      startHtmlPreview: async () => { throw new Error('Markdown should not start a server') },
    })
    expect(artifact).toEqual({ path: 'docs/README.md', workspacePath: root, type: 'markdown', content: '# Hello\n\n**Mint**' })
  })
  for (const path of ['site/index.html', 'assets/photo.png', 'assets/icon.svg']) {
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
