import { describe, expect, it } from 'vitest'
import { calculateGazeTarget, smoothGaze, calculateStatusGesture } from './tracking'

describe('calculateGazeTarget', () => {
  const bounds = { left: 0, top: 0, width: 800, height: 600 }

  it('returns (0, 0) when pointer is at the character center', () => {
    // Face center at 50% width and 40% height (y = 240)
    const target = calculateGazeTarget({ x: 400, y: 240 }, bounds, 0.4)
    expect(target.x).toBeCloseTo(0, 2)
    expect(target.y).toBeCloseTo(0, 2)
  })

  it('returns positive X and positive Y when looking up-right', () => {
    const target = calculateGazeTarget({ x: 600, y: 100 }, bounds, 0.4)
    expect(target.x).toBeGreaterThan(0)
    expect(target.y).toBeGreaterThan(0) // Live2D +Y is looking up
  })

  it('returns negative X and negative Y when looking down-left', () => {
    const target = calculateGazeTarget({ x: 200, y: 400 }, bounds, 0.4)
    expect(target.x).toBeLessThan(0)
    expect(target.y).toBeLessThan(0) // Live2D -Y is looking down
  })

  it('clamps coordinates to unit circle magnitude 1.0 when pointer is far away', () => {
    const target = calculateGazeTarget({ x: 5000, y: 5000 }, bounds, 0.4)
    const magnitude = Math.hypot(target.x, target.y)
    expect(magnitude).toBeCloseTo(1, 2)
  })

  it('correctly tracks cursor far to the left across the desktop (negative x offset)', () => {
    const target = calculateGazeTarget({ x: -1000, y: 300 }, { left: 0, top: 0, width: 480, height: 800 }, 0.45)
    expect(target.x).toBeLessThan(-0.9)
    expect(Math.hypot(target.x, target.y)).toBeCloseTo(1, 2)
  })

  it('safely handles degenerate bounds or NaN pointer inputs', () => {
    expect(calculateGazeTarget({ x: NaN, y: 0 }, bounds)).toEqual({ x: 0, y: 0 })
    expect(calculateGazeTarget({ x: 100, y: 100 }, { left: 0, top: 0, width: 0, height: 0 })).toEqual({ x: 0, y: 0 })
  })

  it('adjusts facial center when character is shifted vertically', () => {
    // When shifted up by 10% (offsetY = 0.1), face center ratio shifts to 0.42 - 0.1 = 0.32
    // centerY = 600 * 0.32 = 192
    const target = calculateGazeTarget({ x: 400, y: 192 }, bounds, 0.32)
    expect(target.x).toBeCloseTo(0, 2)
    expect(target.y).toBeCloseTo(0, 2)
  })
})

describe('smoothGaze', () => {
  it('interpolates towards target by smoothing factor', () => {
    const current = { x: 0, y: 0 }
    const target = { x: 1, y: 1 }
    const smoothed = smoothGaze(current, target, 0.2)

    expect(smoothed.x).toBeCloseTo(0.2, 2)
    expect(smoothed.y).toBeCloseTo(0.2, 2)
  })

  it('snaps to target when within deadzone to eliminate still-mouse jitter', () => {
    const current = { x: 0.5002, y: 0.5001 }
    const target = { x: 0.5, y: 0.5 }
    const smoothed = smoothGaze(current, target, 0.2, 0.005)

    expect(smoothed.x).toBe(0.5)
    expect(smoothed.y).toBe(0.5)
  })

  it('handles zero or negative factor safely without moving past target', () => {
    const current = { x: 0.1, y: 0.2 }
    const target = { x: 0.8, y: 0.9 }
    const smoothed = smoothGaze(current, target, 1.0)

    expect(smoothed.x).toBeCloseTo(0.8, 2)
    expect(smoothed.y).toBeCloseTo(0.9, 2)
  })
})

describe('calculateStatusGesture', () => {
  it('returns zero angles when idle', () => {
    const gesture = calculateStatusGesture('idle', 1.0)
    expect(gesture.angleZ).toBe(0)
    expect(gesture.angleY).toBe(0)
  })

  it('performs gesture during initial active duration', () => {
    const thinking = calculateStatusGesture('thinking', 0.5)
    expect(thinking.angleZ).not.toBe(0)

    const responding = calculateStatusGesture('responding', 0.5)
    expect(responding.angleY).not.toBe(0)
  })

  it('decays to zero after gesture duration expires so character settles', () => {
    const settledThinking = calculateStatusGesture('thinking', 3.0)
    expect(settledThinking.angleZ).toBe(0)
    expect(settledThinking.angleY).toBe(0)

    const settledWorking = calculateStatusGesture('working', 3.0)
    expect(settledWorking.angleZ).toBe(0)
    expect(settledWorking.angleY).toBe(0)

    const settledCompleted = calculateStatusGesture('completed', 2.0)
    expect(settledCompleted.angleZ).toBe(0)
    expect(settledCompleted.angleY).toBe(0)
  })

  it('returns zero for negative elapsed time', () => {
    const gesture = calculateStatusGesture('thinking', -1.0)
    expect(gesture.angleZ).toBe(0)
    expect(gesture.angleY).toBe(0)
  })
})
