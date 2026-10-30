import type { ArtifactFile } from './artifacts'
import { runtimePlatform, workspacePlatform } from '../platform'

export interface BrowserApi {
  desktop?: boolean
  startPreview?(root: string, path: string): Promise<{ url: string; jobId: string }>
  openBrowser?(url: string): Promise<void>
}

export async function openArtifactInBrowser(
  artifact: ArtifactFile,
  workspace: string | undefined,
  api: BrowserApi = {},
): Promise<void> {
  const openBrowser = api.openBrowser ?? (async (targetUrl: string) => {
    await runtimePlatform.openExternal(targetUrl)
  })
  const startPreview = api.startPreview ?? ((root: string, targetPath: string) => workspacePlatform.startHtmlPreview(root, targetPath))

  if (artifact.url) {
    await openBrowser(artifact.url)
    return
  }
  const root = (artifact.workspacePath ?? workspace)?.trim()
  if (!root) throw new Error('Select a project before opening its preview in browser')
  let path = artifact.path.replace(/\\/g, '/')
  if (/^(\/|[a-z]:\/)/i.test(path)) {
    const normalizedRoot = root.replace(/\\/g, '/').replace(/\/+$/, '')
    const prefix = `${normalizedRoot}/`
    const windows = /^[a-z]:\//i.test(normalizedRoot) || normalizedRoot.startsWith('//')
    if (!(windows ? path.toLowerCase().startsWith(prefix.toLowerCase()) : path.startsWith(prefix))) {
      throw new Error('This file is outside its source project')
    }
    path = path.slice(prefix.length)
  }
  const preview = await startPreview(root, path)
  await openBrowser(preview.url)
}

