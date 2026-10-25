import type { ArtifactFile } from './artifacts'

/** Tabs own Blob URLs, so duplicates share one URL and closing the last tab releases it. */
export class PreviewObjectUrls {
  private urls = new Set<string>()
  private disposed = false
  constructor(private revoke: (url: string) => void = url => URL.revokeObjectURL(url)) {}
  accept(artifact: ArtifactFile): boolean {
    if (!artifact.objectUrl || !artifact.url) return !this.disposed
    if (this.disposed) { this.revoke(artifact.url); return false }
    this.urls.add(artifact.url)
    return true
  }
  update(artifacts: ArtifactFile[]) {
    const next = new Set(artifacts.filter(file => file.objectUrl && file.url).map(file => file.url!))
    for (const url of this.urls) if (!next.has(url)) this.revoke(url)
    this.urls = next
  }
  dispose() { this.disposed = true; for (const url of this.urls) this.revoke(url); this.urls.clear() }
}
