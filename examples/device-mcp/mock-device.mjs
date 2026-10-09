import { randomUUID } from 'node:crypto'
import { deflateSync } from 'node:zlib'

/** Replace this adapter with hardware I/O. Observations must come from sensors. */
export class MockDevice {
  #rpm = 0; #sequence = 0; #active; #operations = new Map()

  #observe(operation) {
    if (operation === this.#active && Date.now() >= operation.finishesAt) {
      this.#rpm = operation.target.rpm; operation.state = 'completed'; this.#active = undefined
    }
    return { deviceId: 'mock-device', operationId: operation?.id ?? 'status', state: operation?.state === 'accepted' ? 'running' : operation?.state ?? 'completed', target: operation?.target ?? {}, actual: { rpm: this.#rpm }, observationSeq: ++this.#sequence, ...(operation?.error ? { error: operation.error } : {}) }
  }

  getStatus(operationId) {
    // Read the physical state even when asking about an older operation.
    if (this.#active) this.#observe(this.#active)
    const operation = operationId ? this.#operations.get(operationId) : this.#active
    if (operationId && !operation) throw new Error('Unknown operationId')
    return this.#observe(operation)
  }

  #command(target, duration) {
    const operation = { id: randomUUID(), target, state: 'accepted', finishesAt: Date.now() + duration }
    this.#operations.set(operation.id, operation); this.#active = operation
    if (this.#operations.size > 256) this.#operations.delete(this.#operations.keys().next().value)
    return { deviceId: 'mock-device', operationId: operation.id, state: 'accepted', target, actual: { rpm: this.#rpm }, observationSeq: ++this.#sequence }
  }

  setSpeed(rpm) {
    if (this.#active) this.#observe(this.#active)
    if (this.#active) throw new Error('Device busy; explicitly stop the current operation first')
    return this.#command({ rpm }, 2000)
  }

  stop() {
    if (this.#active) { this.#active.state = 'failed'; this.#active.error = 'Superseded by explicit stop' }
    this.#rpm = 0
    return this.#command({ rpm: 0 }, 0)
  }

  capturePhoto() {
    const operation = { id: randomUUID(), state: 'completed', target: { captured: true } }
    this.#operations.set(operation.id, operation)
    if (this.#operations.size > 256) this.#operations.delete(this.#operations.keys().next().value)
    return { state: this.#observe(operation), image: samplePng() }
  }
}

function samplePng() {
  const crc = bytes => {
    let c = 0xffffffff
    for (const byte of bytes) { c ^= byte; for (let i = 0; i < 8; i++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1 }
    return (c ^ 0xffffffff) >>> 0
  }
  const chunk = (type, payload) => {
    const data = Buffer.concat([Buffer.from(type), payload]); const header = Buffer.alloc(4); header.writeUInt32BE(payload.length)
    const checksum = Buffer.alloc(4); checksum.writeUInt32BE(crc(data)); return Buffer.concat([header, data, checksum])
  }
  const header = Buffer.alloc(13); header.writeUInt32BE(32); header.writeUInt32BE(32, 4); header[8] = 8; header[9] = 2
  const pixels = Buffer.alloc(32 * (1 + 32 * 3))
  for (let y = 0; y < 32; y++) for (let x = 0; x < 32; x++) {
    const i = y * 97 + 1 + x * 3; pixels[i] = 50; pixels[i + 1] = 120 + x * 3; pixels[i + 2] = 80 + y * 3
  }
  return Buffer.concat([Buffer.from('89504e470d0a1a0a', 'hex'), chunk('IHDR', header), chunk('IDAT', deflateSync(pixels)), chunk('IEND', Buffer.alloc(0))]).toString('base64')
}
