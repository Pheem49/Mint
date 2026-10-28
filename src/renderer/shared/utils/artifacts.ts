export type ArtifactType =
  | 'html'
  | 'svg'
  | 'markdown'
  | 'image'
  | 'code'
  | 'file'
  | 'pdf'
  | 'audio'
  | 'video'

export type ViewportMode = 'desktop' | 'tablet' | 'mobile'

export interface ArtifactFile {
  path: string
  workspacePath?: string
  url?: string
  content?: string
  type?: ArtifactType
  mimeType?: string
  byteLength?: number
  objectUrl?: boolean
  range?: boolean
}

export function detectArtifactType(filePath: string): ArtifactType {
  const lower = filePath.toLowerCase()
  if (lower.endsWith('.html') || lower.endsWith('.htm')) return 'html'
  if (/\.(png|jpe?g|gif|webp|ico)$/.test(lower)) return 'image'
  if (lower.endsWith('.svg')) return 'svg'
  if (lower.endsWith('.md') || lower.endsWith('.markdown')) return 'markdown'
  if (lower.endsWith('.pdf')) return 'pdf'
  if (/\.(mp3|wav|ogg|oga|opus|flac|m4a|aac|weba)$/.test(lower)) return 'audio'
  if (/\.(mp4|m4v|webm|ogv|mov)$/.test(lower)) return 'video'
  return 'code'
}

export function getFileLanguage(filePath: string): string {
  const ext = filePath.split('.').pop()?.toLowerCase() || ''
  const map: Record<string, string> = {
    js: 'javascript',
    jsx: 'javascript',
    ts: 'typescript',
    tsx: 'typescript',
    html: 'html',
    htm: 'html',
    css: 'css',
    json: 'json',
    rs: 'rust',
    py: 'python',
    svg: 'xml',
    md: 'markdown',
  }
  return map[ext] || ext || 'plaintext'
}
