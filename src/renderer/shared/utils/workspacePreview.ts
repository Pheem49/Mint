import { detectArtifactType, type ArtifactFile } from '../components/ArtifactPreviewPanel'

interface PreviewApi {
  readWorkspaceFile(path: string, root?: string): Promise<string>
  startHtmlPreview(root: string, path: string): Promise<{ url: string; jobId: string }>
}

export async function createWorkspacePreview(root: string, path: string, api: PreviewApi): Promise<ArtifactFile> {
  const type = detectArtifactType(path)
  if (type === 'code') throw new Error('This file type does not support Preview')
  if (type === 'markdown') {
    return { path, workspacePath: root, type, content: await api.readWorkspaceFile(path, root) }
  }
  const { url } = await api.startHtmlPreview(root, path)
  return { path, workspacePath: root, type, url }
}
