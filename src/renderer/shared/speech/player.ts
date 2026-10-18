import type { SpeechReply, SpeechTransport } from './types'

type Prepared = Extract<SpeechReply, { kind: 'prepared' }>
type Input = { chatId: string; turnId?: number; text?: string; automatic?: boolean; audioOnly?: boolean }
export type SpeechState = 'idle' | 'preparing' | 'speaking'

/** One cancellable playback run, shared by Desktop, Web and Companion. */
export class SpeechPlayer {
  private generation = 0
  private owner: string | null = null
  private context: AudioContext | null = null
  private source: AudioBufferSourceNode | null = null
  private gain: GainNode | null = null
  private timer: ReturnType<typeof setInterval> | undefined
  private frame = 0
  private settle: (() => void) | null = null
  private renewing = false
  private localVolume = 1
  private hostVolume = 1
  private speaking = false
  constructor(private transport: SpeechTransport, private changed: (state: SpeechState) => void, private failed: (message: string) => void, private samples?: (samples: Float32Array | null) => void) {}

  unlock(): Promise<void> {
    try {
      this.context ??= new AudioContext()
      const context = this.context
      return new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('Use the Read button to allow audio playback.')), 1500)
        context.resume().then(() => { clearTimeout(timer); resolve() }, error => { clearTimeout(timer); reject(error) })
      })
    }
    catch (error) { return Promise.reject(error) }
  }
  volume(value: number) {
    this.localVolume = Math.min(1, Math.max(0, value))
    if (this.gain) this.gain.gain.value = this.localVolume * this.hostVolume
  }
  stop(): Promise<void> {
    this.generation += 1
    const owner = this.owner
    this.owner = null
    clearInterval(this.timer); this.timer = undefined
    cancelAnimationFrame(this.frame)
    const wasSpeaking = this.speaking; this.speaking = false
    try { this.source?.stop() } catch {}
    this.source?.disconnect(); this.source = null
    this.gain?.disconnect(); this.gain = null
    this.settle?.(); this.settle = null
    if (wasSpeaking && typeof speechSynthesis !== 'undefined') speechSynthesis.cancel()
    this.samples?.(null); this.changed('idle')
    return owner ? this.transport({ operation: 'release', owner }).then(() => {}, () => {}) : Promise.resolve()
  }
  async dispose() { const context = this.context; this.context = null; await this.stop(); await context?.close() }
  private async renew(owner: string): Promise<boolean> {
    const reply = await this.transport({ operation: 'renew', owner })
    return reply.kind === 'lease' && reply.active
  }
  async play(input: Input): Promise<void> {
    const previous = this.stop()
    const generation = this.generation
    const owner = crypto.randomUUID()
    this.owner = owner
    this.changed('preparing')
    // Resume synchronously from the Read/Enable button's user gesture.
    const unlocked = this.unlock().then(() => null, error => error)
    try {
      await previous; const unlockError = await unlocked; if (unlockError) throw unlockError
      if (generation !== this.generation) return
      let prepared: SpeechReply | undefined
      for (let attempt = 0; attempt < 40; attempt++) {
        if (generation !== this.generation) return
        try { prepared = await this.transport({ operation: 'prepare', owner, ...input }); break }
        catch (error) {
          if (!input.automatic || !/Another Mint surface/.test(String(error)) || attempt === 39) throw error
          // Wait for the revoked player to acknowledge stopping; never overlap its final audio segment.
          await new Promise(resolve => setTimeout(resolve, 300))
        }
      }
      if (generation !== this.generation) return
      if (!prepared || prepared.kind !== 'prepared') throw new Error('Mint returned no speech plan')
      this.hostVolume = prepared.volume
      this.timer = setInterval(() => {
        if (this.renewing) return
        this.renewing = true
        this.renew(owner).then(active => { if (!active && this.owner === owner) void this.stop() })
          .catch(() => { if (this.owner === owner) void this.stop() }).finally(() => { this.renewing = false })
      }, 1000)
      if (prepared.provider === 'native') {
        if (input.audioOnly) throw new Error('This voice cannot stream audio to Companion')
        await this.playNative(prepared, generation)
      } else {
        for (let index = 0; index < prepared.chunks && generation === this.generation; index++) {
          const audio = await this.transport({ operation: 'audio', owner, index })
          if (generation !== this.generation) return
          if (audio.kind !== 'audio') throw new Error('Mint returned no speech audio')
          const bytes = Uint8Array.from(atob(audio.data), character => character.charCodeAt(0))
          const buffer = await this.context!.decodeAudioData(bytes.buffer)
          if (generation !== this.generation) return
          await this.playBuffer(buffer, prepared, owner, generation)
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (generation === this.generation && !(input.automatic && /already been read automatically|Another Mint surface|A newer message|Automatic replies are routed/.test(message))) this.failed(message)
    } finally {
      if (generation === this.generation) await this.stop()
      // Covers cancellation racing with preparation on another socket/process.
      else await this.transport({ operation: 'release', owner }).catch(() => {})
    }
  }
  private async playBuffer(buffer: AudioBuffer, plan: Prepared, owner: string, generation: number) {
    const context = this.context!
    const gain = context.createGain(); this.gain = gain
    gain.gain.value = plan.volume * this.localVolume
    const analyser = context.createAnalyser(); analyser.fftSize = 256
    gain.connect(analyser); analyser.connect(context.destination)
    const samples = new Float32Array(analyser.fftSize)
    let frame = 0
    const sample = () => {
      if (generation !== this.generation) return
      analyser.getFloatTimeDomainData(samples); this.samples?.(samples)
      frame = requestAnimationFrame(sample); this.frame = frame
    }
    this.changed('speaking'); this.speaking = true; sample()
    try {
      // Short independently leased segments cannot continue for minutes after a frozen tab loses ownership.
      let offset = 0
      while (offset < buffer.duration && generation === this.generation) {
        if (!await this.renew(owner) || generation !== this.generation) return
        const length = Math.min(4 * plan.speed, buffer.duration - offset)
        await new Promise<void>(resolve => {
          const source = context.createBufferSource(); this.source = source
          source.buffer = buffer; source.playbackRate.value = plan.speed
          source.connect(gain); this.settle = resolve
          source.onended = () => { source.disconnect(); if (this.source === source) { this.source = null; this.settle = null } resolve() }
          source.start(0, offset, length)
        })
        offset += length
      }
    } finally {
      cancelAnimationFrame(frame); if (generation === this.generation) this.samples?.(null)
      analyser.disconnect(); gain.disconnect(); if (this.gain === gain) this.gain = null
    }
  }
  private async playNative(plan: Prepared, generation: number) {
    if (typeof speechSynthesis === 'undefined') throw new Error('System speech is unavailable')
    this.changed('speaking'); this.speaking = true
    await new Promise<void>((resolve, reject) => {
      const utterance = new SpeechSynthesisUtterance(plan.text)
      utterance.lang = /[\u0e00-\u0e7f]/.test(plan.text) ? 'th-TH' : 'en-US'
      utterance.volume = plan.volume * this.localVolume; utterance.rate = plan.speed; utterance.pitch = plan.pitch
      const voice = speechSynthesis.getVoices().find(voice => voice.lang.toLowerCase().startsWith(utterance.lang.slice(0, 2)))
      if (voice) utterance.voice = voice
      this.settle = resolve
      utterance.onend = () => { if (this.settle === resolve) this.settle = null; resolve() }
      utterance.onerror = event => { if (this.settle === resolve) this.settle = null; if (generation !== this.generation) resolve(); else reject(new Error(event.error)) }
      speechSynthesis.speak(utterance)
    })
  }
}
