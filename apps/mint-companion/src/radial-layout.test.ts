import { describe, expect, it } from 'vitest'
import { radialMenuOrigin } from './radial-layout'

describe('radial menu viewport placement', () => {
  it('keeps the expanded menu inside the right and bottom edges', () => {
    const origin = radialMenuOrigin({ x: 680, y: 24 }, 700, 500)

    expect(origin.left + 152).toBeLessThanOrEqual(692)
    expect(origin.top + 296).toBeLessThanOrEqual(492)
  })

  it('keeps the menu inside the left and top edges', () => {
    const origin = radialMenuOrigin({ x: 24, y: 24 }, 700, 500)

    expect(origin.left - 73).toBeGreaterThanOrEqual(8)
    expect(origin.top + 68).toBeGreaterThanOrEqual(8)
  })
})
