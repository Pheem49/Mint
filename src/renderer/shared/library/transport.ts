import type { LibraryItem, LibraryPage, LibraryQuery } from '../types';
interface Environment {
  isNative(): boolean;
  apiBase(): string;
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
}
export function createLibraryTransport(environment: Environment) {
  const request = async (path: string, init?: RequestInit) => {
    const response = await environment.fetch(`${environment.apiBase()}/library${path}`, init);
    if (!response.ok) {
      const data = await response.json().catch(() => null);
      throw new Error(data?.message || `Library unavailable (${response.status})`);
    }
    return response;
  };
  return {
    async removeLibraryItem(id: string): Promise<void> {
      if (environment.isNative()) {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('remove_library_item', { id });
        return;
      }
      await request(`/${encodeURIComponent(id)}`, { method: 'DELETE' });
    },
    async listLibrary(query: LibraryQuery): Promise<LibraryPage> {
      if (environment.isNative()) {
        const { invoke } = await import('@tauri-apps/api/core');
        return invoke('list_library', { query });
      }
      const params = new URLSearchParams();
      Object.entries(query).forEach(([key, value]) => { if (value !== undefined && value !== null && value !== '')
        params.set(key, String(value)); });
      return (await request(`?${params}`)).json();
    },
    async getLibraryItem(id: string): Promise<LibraryItem> {
      if (environment.isNative()) {
        const { invoke } = await import('@tauri-apps/api/core');
        return invoke('get_library_item', { id });
      }
      return (await request(`/${encodeURIComponent(id)}`)).json();
    },
    async setLibraryTrashed(id: string, trashed: boolean): Promise<void> {
      if (environment.isNative()) {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('set_library_trashed', { id, trashed });
        return;
      }
      await request(`/${encodeURIComponent(id)}/${trashed ? 'trash' : 'restore'}`, { method: 'POST' });
    },
    async readLibraryFile(id: string, thumbnail = false): Promise<Blob> {
      if (environment.isNative()) {
        const { invoke } = await import('@tauri-apps/api/core');
        const uri = await invoke<string>('read_library_file', { id, thumbnail });
        // Use a local data URI; remote fetch always passes through the authenticated adapter.
        return (await fetch(uri)).blob();
      }
      return (await request(`/${encodeURIComponent(id)}/${thumbnail ? 'thumbnail' : 'content'}`)).blob();
    },
  };
}
