import type { ArtifactFile } from '../components/ArtifactPreviewPanel'
import { runtimePlatform, workspacePlatform } from '../platform'

interface BrowserApi {
  desktop: boolean
  startPreview(root: string, path: string, mintBrowser: boolean): Promise<{ url: string; jobId: string }>
  openMintBrowser(url: string): Promise<void>
}

export async function openArtifactInMintBrowser(
  artifact: ArtifactFile,
  workspace: string | undefined,
  api: BrowserApi = {
    desktop: runtimePlatform.isTauriRuntime(),
    startPreview: (root, path, mint) => workspacePlatform.startHtmlPreview(root, path, mint),
    openMintBrowser: async (url) => {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('open_mint_browser', { url })
    },
  },
): Promise<void> {
  if (api.desktop && artifact.url) {
    await api.openMintBrowser(artifact.url)
    return
  }
  const root = (artifact.workspacePath ?? workspace)?.trim()
  if (!root) throw new Error('Select a project before opening its preview in Mint Browser')
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
  // The authenticated local Web endpoint launches the same automation browser on the backend.
  const preview = await api.startPreview(root, path, !api.desktop)
  if (api.desktop) await api.openMintBrowser(preview.url)
}
