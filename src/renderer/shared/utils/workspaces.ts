/** Workspace catalog shared by the chat and Code hub pickers. */
export function normalizeWorkspacePath(path?: string | null): string {
  const normalized = (path || '').trim().replace(/\\/g, '/')
  if (/^[a-z]:\/+$/i.test(normalized)) return normalized.slice(0, 3)
  return normalized.replace(/\/+$/, '') || (normalized.startsWith('/') ? '/' : '')
}

export function workspacePaths(recentPaths: string[], currentPath?: string, sessionPaths: (string | undefined)[] = []): string[] {
  return [...new Set([...recentPaths, currentPath, ...sessionPaths].map(normalizeWorkspacePath).filter(Boolean))]
}
