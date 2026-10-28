export class AutoSpeechTracker {
  private initialized = false
  private completed = new Set<number>()
  reset() { this.initialized = false; this.completed.clear() }
  observe(turns: Array<{ turnId: number; status: string }>): number | null {
    const completed = turns.filter(turn => turn.status === 'completed').map(turn => turn.turnId)
    const fresh = completed.filter(id => !this.completed.has(id))
    const wasInitialized = this.initialized
    completed.forEach(id => this.completed.add(id))
    this.initialized = true
    return wasInitialized && fresh.length ? Math.max(...fresh) : null
  }
}
export function mouthFromSamples(samples: Float32Array, previous: number): number {
  let energy = 0
  for (const sample of samples) energy += sample * sample
  const rms = Math.sqrt(energy / Math.max(1, samples.length))
  const target = Math.min(1, Math.max(0, (rms - 0.012) * 7))
  const value = previous + (target - previous) * (target > previous ? 0.55 : 0.25)
  return value < 0.005 ? 0 : value
}
