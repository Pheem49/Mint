import type { WorkspaceMediaSource } from '../types'

const WEB_MEDIA_LIMIT = 256 * 1024 * 1024

/** Native media uses one authorized asset file with Range support; Web owns a bounded Blob. */
export function createWorkspaceMediaReader(environment: {
  isNative(): boolean
  apiBase(): string
  convertFileSrc(path: string): string
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>
  createObjectURL?: (blob: Blob) => string
}) {
  return async (path: string, workspacePath: string): Promise<WorkspaceMediaSource> => {
    if (environment.isNative()) {
      const { invoke } = await import('@tauri-apps/api/core')
      const source = await invoke<{ path: string; mimeType: string; byteLength: number }>('prepare_workspace_media', { path, workspacePath })
      return { url: environment.convertFileSrc(source.path), mimeType: source.mimeType, byteLength: source.byteLength, range: true }
    }
    const response = await environment.fetch(`${environment.apiBase()}/file/media?${new URLSearchParams({ path, workspace: workspacePath })}`)
    if (!response.ok) { const value = await response.json().catch(() => ({})); throw Error(value.error || 'Could not read workspace media') }
    const mimeType = response.headers.get('Content-Type')?.split(';')[0] || ''
    if (!/^(application\/pdf|audio\/[\w.+-]+|video\/[\w.+-]+)$/.test(mimeType)) { await response.body?.cancel(); throw Error('Unsupported media response') }
    if (Number(response.headers.get('Content-Length')) > WEB_MEDIA_LIMIT) { await response.body?.cancel(); throw Error('Web media preview supports files up to 256 MiB. Open this file in Desktop for larger media.') }
    const reader = response.body?.getReader()
    if (!reader) throw Error('Media content is unavailable')
    const chunks: Uint8Array<ArrayBuffer>[] = []
    let byteLength = 0
    try {
      for (;;) {
        const { value, done } = await reader.read()
        if (done) break
        byteLength += value.byteLength
        if (byteLength > WEB_MEDIA_LIMIT) throw Error('Web media preview supports files up to 256 MiB. Open this file in Desktop for larger media.')
        chunks.push(value as Uint8Array<ArrayBuffer>)
      }
    } catch (error) { await reader.cancel().catch(() => {}); throw error }
    finally { reader.releaseLock() }
    const blob = new Blob(chunks, { type: mimeType })
    return { url: (environment.createObjectURL ?? URL.createObjectURL)(blob), mimeType, byteLength, objectUrl: true }
  }
}
