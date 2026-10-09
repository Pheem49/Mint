import type { WorkspaceOperation, WorkspaceSnapshot } from '../types'
export class WorkspaceSnapshotReader {
  private generation = 0
  private inFlightRoot: string | null = null
  private disposed = false
  constructor(private read: (request: WorkspaceOperation) => Promise<WorkspaceSnapshot>) {}
  async refresh(request: WorkspaceOperation): Promise<WorkspaceSnapshot | null> {
    if (this.disposed || this.inFlightRoot === request.root) return null
    const generation = ++this.generation
    this.inFlightRoot = request.root
    try {
      const snapshot = await this.read(request)
      return !this.disposed && generation === this.generation ? snapshot : null
    } catch (error) {
      if (this.disposed || generation !== this.generation) return null
      throw error
    } finally {
      if (generation === this.generation) this.inFlightRoot = null
    }
  }
  dispose() {
    this.disposed = true
    this.generation++
    this.inFlightRoot = null
  }
}
