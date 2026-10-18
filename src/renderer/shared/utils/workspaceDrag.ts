import { normalizeWorkspacePath } from './workspaces'

export const WORKSPACE_FILE_MIME = 'application/x-mint-workspace-file'
export interface WorkspaceFileDrag { root: string; path: string; kind: 'file' }

/** Only explicit Workspace file drags are accepted; text mentions and folders stay separate. */
export function parseWorkspaceFileDrag(raw: string): WorkspaceFileDrag {
  let value: Partial<WorkspaceFileDrag>
  try { value = JSON.parse(raw) } catch { throw Error('Drag a file from Workspace to preview it.') }
  if (!value || value.kind !== 'file' || typeof value.root !== 'string' || typeof value.path !== 'string') throw Error('Drag a file from Workspace to preview it.')
  const root = normalizeWorkspacePath(value.root)
  const path = value.path.replace(/\\/g, '/')
  if (!root || !/^(\/|[a-z]:\/)/i.test(root) || /\0/.test(root + path) || !path || /^(\/|[a-z]:)/i.test(path) || path.split('/').includes('..')) throw Error('This file is outside its source workspace.')
  const relative = path.split('/').filter(part => part && part !== '.').join('/')
  if (!relative) throw Error('Choose a file to preview.')
  return { root, path: relative, kind: 'file' }
}

export function workspacePreviewId(root: string, path: string): string {
  const file = parseWorkspaceFileDrag(JSON.stringify({ root, path, kind: 'file' }))
  return `preview:${JSON.stringify([file.root, file.path])}`
}

export function workspaceFileUrl(root: string, path: string): string {
  const file = parseWorkspaceFileDrag(JSON.stringify({ root, path, kind: 'file' }))
  const full = `${file.root.replace(/\/$/, '')}/${file.path}`
  const encoded = full.split('/').map(part => encodeURIComponent(part)).join('/')
  if (/^[a-z]:\//i.test(full)) return `file:///${full.slice(0, 2)}${encoded.slice(encoded.indexOf('/'))}`
  return `${full.startsWith('//') ? 'file:' : 'file://'}${encoded}`
}
