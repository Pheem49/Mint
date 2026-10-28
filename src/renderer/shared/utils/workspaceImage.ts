/** Read through native IPC or the existing authenticated API; never start a preview service. */
export function createWorkspaceImageReader(environment: {
  isNative(): boolean
  apiBase(): string
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>
}) {
  return async (path: string, workspacePath: string): Promise<string> => {
    if (environment.isNative()) {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke<string>('read_workspace_image', { path, workspacePath })
    }
    const response = await environment.fetch(`${environment.apiBase()}/file/image?${new URLSearchParams({ path, workspace: workspacePath })}`)
    const value = await response.json()
    if (!response.ok) throw Error(value.error || 'Could not read workspace image')
    return value.dataUrl
  }
}
