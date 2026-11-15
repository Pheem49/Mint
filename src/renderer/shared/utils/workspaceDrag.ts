import { normalizeWorkspacePath } from './workspaces'

export const WORKSPACE_FILE_MIME = 'application/x-mint-workspace-file'
export const WORKSPACE_PATH_MIME = 'application/x-mint-workspace-path'
export const WORKSPACE_IMAGE_MIME = 'application/x-mint-workspace-image'

export interface WorkspaceFileDrag { root: string; path: string; kind: 'file' }

const IMAGE_EXT_RE = /\.(png|jpe?g|gif|webp|ico|svg|bmp|avif)$/i

export function isImageFileName(path: string): boolean {
  return IMAGE_EXT_RE.test(path.trim())
}

/**
 * Format a reference token suitable for dragging into chat or prompt input.
 * Ensures the result starts with '@' and prefers a workspace-relative path or clean filename.
 */
export function formatWorkspaceReference(path: string, workspaceRoot?: string | null): string {
  const clean = path.replace(/\\/g, '/').trim()
  if (!clean) return ''
  if (clean.startsWith('@')) return clean

  if (workspaceRoot) {
    const rootNorm = workspaceRoot.replace(/\\/g, '/').replace(/\/+$/, '')
    if (clean === rootNorm || clean.startsWith(`${rootNorm}/`)) {
      const rel = clean.slice(rootNorm.length + 1)
      if (rel) return `@${rel}`
    }
  }

  // Strip leading slash if workspace reference
  const trimmed = clean.startsWith('/') ? clean.slice(1) : clean
  return `@${trimmed}`
}

export interface SetWorkspaceDragDataOptions {
  path: string
  root?: string | null
  kind?: 'file' | 'directory'
  url?: string | null
}

/**
 * Populate DataTransfer with standard Mint workspace file / reference drag data.
 * Sets both the proprietary MIME types and text/plain fallback so drop targets
 * (Chat composer, form, conversation panel) receive the expected @reference.
 */
export function setWorkspaceDragData(
  dataTransfer: DataTransfer,
  options: SetWorkspaceDragDataOptions
): void {
  const dragText = formatWorkspaceReference(options.path, options.root)
  if (!dragText) return

  dataTransfer.setData(WORKSPACE_PATH_MIME, dragText)
  dataTransfer.setData('text/plain', dragText)

  const isImg =
    isImageFileName(options.path) ||
    Boolean(options.url?.startsWith('data:image/') || options.url?.startsWith('blob:'))
  if (isImg && options.kind !== 'directory') {
    const fileName = options.path.split(/[\\/]/).pop() || options.path
    try {
      dataTransfer.setData(
        WORKSPACE_IMAGE_MIME,
        JSON.stringify({
          path: options.path,
          root: options.root || '',
          url: options.url || '',
          name: fileName,
        })
      )
    } catch {
      // ignore
    }
  }

  if (options.kind !== 'directory' && options.path) {
    const root = options.root ? normalizeWorkspacePath(options.root) : ''
    const relativePath = dragText.startsWith('@') ? dragText.slice(1) : options.path
    try {
      dataTransfer.setData(
        WORKSPACE_FILE_MIME,
        JSON.stringify({ root, path: relativePath, kind: 'file' })
      )
    } catch {
      // ignore
    }
  }

  dataTransfer.effectAllowed = 'copy'
}

/**
 * Resolves a dragged image reference (data URI, blob URL, web URL, or workspace path)
 * into a base64 dataUri that can be directly attached to a multimodal prompt.
 */
export async function resolveImageToDataUri(
  target: string,
  root?: string | null,
  platformReader?: (path: string, workspacePath: string) => Promise<string>
): Promise<string> {
  const clean = target.trim()
  if (!clean) return ''

  if (clean.startsWith('data:image/')) {
    return clean
  }

  if (
    clean.startsWith('blob:') ||
    clean.startsWith('http://') ||
    clean.startsWith('https://') ||
    clean.startsWith('/api/') ||
    (clean.startsWith('/') && IMAGE_EXT_RE.test(clean))
  ) {
    try {
      const res = await fetch(clean)
      if (res.ok) {
        const blob = await res.blob()
        return await new Promise<string>((resolve, reject) => {
          const reader = new FileReader()
          reader.onload = () => resolve(reader.result as string)
          reader.onerror = reject
          reader.readAsDataURL(blob)
        })
      }
    } catch {
      // fallback
    }
  }

  if (platformReader) {
    try {
      const res = await platformReader(clean, root || '')
      if (res && res.startsWith('data:image/')) return res
    } catch {
      // ignore
    }
  }

  return ''
}



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
