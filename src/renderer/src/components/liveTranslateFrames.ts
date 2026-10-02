const SAMPLE_WIDTH = 160
const SAMPLE_HEIGHT = 90

export async function sampleFrame(dataUri: string): Promise<Uint8ClampedArray> {
  const image = new Image()
  await new Promise<void>((resolve, reject) => {
    image.onload = () => resolve()
    image.onerror = () => reject(new Error('Unable to read the selected screen area.'))
    image.src = dataUri
  })
  const canvas = document.createElement('canvas')
  canvas.width = SAMPLE_WIDTH
  canvas.height = SAMPLE_HEIGHT
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context) throw new Error('Unable to compare screen frames.')
  context.drawImage(image, 0, 0, SAMPLE_WIDTH, SAMPLE_HEIGHT)
  const rgba = context.getImageData(0, 0, SAMPLE_WIDTH, SAMPLE_HEIGHT).data
  const gray = new Uint8ClampedArray(SAMPLE_WIDTH * SAMPLE_HEIGHT)
  for (let pixel = 0; pixel < gray.length; pixel += 1) {
    const offset = pixel * 4
    gray[pixel] = Math.round(rgba[offset] * 0.299 + rgba[offset + 1] * 0.587 + rgba[offset + 2] * 0.114)
  }
  return gray
}

/** Ignore compression noise and a blinking caret, but catch changed words or lines. */
export function hasMeaningfulFrameChange(previous: Uint8ClampedArray | null, current: Uint8ClampedArray): boolean {
  if (!previous || previous.length !== current.length) return true
  let changed = 0
  for (let pixel = 0; pixel < current.length; pixel += 1) {
    if (Math.abs(current[pixel] - previous[pixel]) >= 28) changed += 1
  }
  return changed >= Math.max(20, Math.ceil(current.length * 0.0015))
}
