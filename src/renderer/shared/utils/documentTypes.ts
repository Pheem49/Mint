/**
 * shared/utils/documentTypes.ts
 * File extensions the backend can extract text from for a chat document
 * attachment. Mirrors `supported()` in crates/mint-core/src/knowledge.rs —
 * keep both lists in sync.
 */

export const SUPPORTED_DOCUMENT_EXTENSIONS = [
  'txt', 'md', 'pdf', 'docx', 'xlsx', 'json', 'toml', 'yaml', 'yml',
  'rs', 'ts', 'tsx', 'js', 'jsx', 'py', 'html', 'css',
] as const

export const SUPPORTED_DOCUMENT_ACCEPT = SUPPORTED_DOCUMENT_EXTENSIONS.map((ext) => `.${ext}`).join(',')

export function isSupportedDocument(filename: string): boolean {
  const match = /\.([a-zA-Z0-9]+)$/.exec(filename.trim())
  const ext = match?.[1]?.toLowerCase()
  return !!ext && (SUPPORTED_DOCUMENT_EXTENSIONS as readonly string[]).includes(ext)
}
