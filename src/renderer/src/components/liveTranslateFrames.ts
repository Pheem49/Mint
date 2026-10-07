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

/** Ignore pixels that keep moving between captures, such as a game scene behind stable subtitles. */
export function createFrameChangeTracker(): (current: Uint8ClampedArray) => boolean {
  let previous: Uint8ClampedArray | null = null
  let motion: Uint8Array | null = null
  return (current) => {
    if (!previous || previous.length !== current.length) {
      previous = current
      motion = new Uint8Array(current.length)
      return true
    }
    let stableChanges = 0
    for (let pixel = 0; pixel < current.length; pixel += 1) {
      const changed = Math.abs(current[pixel] - previous[pixel]) >= 28
      if (changed && motion![pixel] === 0) stableChanges += 1
      motion![pixel] = changed ? Math.min(2, motion![pixel] + 1) : Math.max(0, motion![pixel] - 1)
    }
    previous = current
    return stableChanges >= Math.max(20, Math.ceil(current.length * 0.0015))
  }
}
