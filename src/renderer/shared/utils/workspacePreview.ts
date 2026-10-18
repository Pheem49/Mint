import { detectArtifactType, type ArtifactFile } from '../components/ArtifactPreviewPanel'
import { parseWorkspaceFileDrag, workspacePreviewId } from './workspaceDrag'

interface PreviewApi {
  readWorkspaceFile(path: string, root?: string): Promise<string>
  readWorkspaceImage?(path: string, root: string): Promise<string>
  readWorkspaceMedia?(path: string, root: string): Promise<import('../types').WorkspaceMediaSource>
  startHtmlPreview(root: string, path: string): Promise<{ url: string; jobId: string }>
}

export async function createWorkspacePreview(root: string, path: string, api: PreviewApi): Promise<ArtifactFile> {
  const type = detectArtifactType(path)
  if (type === 'pdf' || type === 'audio' || type === 'video') {
    if (!api.readWorkspaceMedia) throw Error('Direct media reading is unavailable')
    return { path, workspacePath: root, type, ...await api.readWorkspaceMedia(path, root) }
  }
  if (type === 'image' || type === 'svg') {
    if (!api.readWorkspaceImage) throw Error('Direct image reading is unavailable')
    return { path, workspacePath: root, type, url: await api.readWorkspaceImage(path, root) }
  }
  const name = path.replace(/\\/g, '/').split('/').pop() || ''
  const text = /\.(txt|log|csv|tsv|jsonl?|ya?ml|toml|ini|conf|cfg|env|lock|js|jsx|mjs|cjs|ts|tsx|css|scss|sass|less|xml|sql|rs|py|pyi|sh|bash|zsh|fish|ps1|bat|cmd|go|java|kt|kts|c|h|cpp|hpp|cc|cs|rb|php|swift|dart|vue|svelte|lua|r|ex|exs|erl|hs|clj|proto|graphql|gql|gitignore|gitattributes|editorconfig)$/i.test(name) || /^(Dockerfile|Containerfile|Makefile|Gemfile|Procfile|LICENSE|NOTICE|README|\.env(?:\..*)?)$/i.test(name)
  if (type === 'code' && !text) return { path, workspacePath: root, type: 'file' }
  if (type === 'markdown' || type === 'code') {
    return { path, workspacePath: root, type, content: await api.readWorkspaceFile(path, root) }
  }
  const { url } = await api.startHtmlPreview(root, path)
  return { path, workspacePath: root, type, url }
}

/** Reuse open tabs and coalesce simultaneous drops before starting preview servers. */
export function createWorkspacePreviewOpener(api: PreviewApi & {
  selectIfOpen: (id: string) => boolean
  open: (artifact: ArtifactFile) => void
}) {
  const pending = new Map<string, Promise<void>>()
  return async (root: string, path: string): Promise<void> => {
    const file = parseWorkspaceFileDrag(JSON.stringify({ root, path, kind: 'file' }))
    const id = workspacePreviewId(file.root, file.path)
    if (api.selectIfOpen(id)) return
    const existing = pending.get(id)
    if (existing) { await existing; api.selectIfOpen(id); return }
    const request = createWorkspacePreview(file.root, file.path, api).then(api.open)
    pending.set(id, request)
    try { await request } finally { pending.delete(id) }
  }
}
