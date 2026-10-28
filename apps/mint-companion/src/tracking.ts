export interface Point2D {
  x: number
  y: number
}

export interface BoundingRect {
  left: number
  top: number
  width: number
  height: number
}

export interface StatusGesture {
  angleZ: number
  angleY: number
}

/**
 * Calculates a normalized (-1 to 1) gaze coordinate targeting the pointer relative
 * to the character's facial center, clamped smoothly within the unit circle.
 */
export function calculateGazeTarget(
  pointer: Point2D,
  bounds: BoundingRect,
  faceCenterYRatio = 0.42
): Point2D {
  if (
    isNaN(pointer.x) ||
    isNaN(pointer.y) ||
    !isFinite(pointer.x) ||
    !isFinite(pointer.y) ||
    bounds.width <= 0 ||
    bounds.height <= 0
  ) {
    return { x: 0, y: 0 }
  }

  const centerX = bounds.left + bounds.width * 0.5
  const centerY = bounds.top + bounds.height * faceCenterYRatio

  const rawX = (pointer.x - centerX) / (bounds.width * 0.5)
  // In Live2D Cubism, positive Y is looking up, negative Y is looking down.
  const rawY = (centerY - pointer.y) / (bounds.height * 0.5)

  const distance = Math.hypot(rawX, rawY)
  if (distance <= 1) {
    return { x: rawX, y: rawY }
  }

  return {
    x: rawX / distance,
    y: rawY / distance,
  }
}

/**
 * Smoothly interpolates the current gaze towards the target gaze with a deadzone
 * that snaps to target to eliminate microscopic jitter when the mouse is still.
 */
export function smoothGaze(
  current: Point2D,
  target: Point2D,
  factor: number,
  deadzone = 0.005
): Point2D {
  const curX = isNaN(current.x) || !isFinite(current.x) ? 0 : current.x
  const curY = isNaN(current.y) || !isFinite(current.y) ? 0 : current.y
  const tgtX = isNaN(target.x) || !isFinite(target.x) ? 0 : target.x
  const tgtY = isNaN(target.y) || !isFinite(target.y) ? 0 : target.y

  const dx = tgtX - curX
  const dy = tgtY - curY
  const distance = Math.hypot(dx, dy)

  if (distance <= deadzone) {
    return { x: tgtX, y: tgtY }
  }

  const clampedFactor = Math.max(0, Math.min(1, factor))
  return {
    x: curX + dx * clampedFactor,
    y: curY + dy * clampedFactor,
  }
}

/**
 * Calculates a smooth gesture (head tilt and nod) for the active status.
 * Instead of continuous, never-ending sine oscillations, each gesture has an
 * expressive peak and a smooth decay envelope that settles back to neutral.
 */
export function calculateStatusGesture(
  status: string | undefined,
  elapsedSeconds: number,
  options?: { maxDuration?: number }
): StatusGesture {
  const maxDuration = options?.maxDuration ?? 2.5

  if (
    !status ||
    status === 'idle' ||
    elapsedSeconds < 0 ||
    elapsedSeconds >= maxDuration
  ) {
    return { angleZ: 0, angleY: 0 }
  }

  const decay = Math.max(0, 1 - elapsedSeconds / maxDuration)

  switch (status) {
    case 'thinking': {
      // Gentle curiosity tilt, decaying smoothly
      const angleZ = Math.sin(elapsedSeconds * Math.PI) * 3 * decay
      const angleY = Math.sin(elapsedSeconds * Math.PI * 0.5) * 1 * decay
      return { angleZ, angleY }
    }
    case 'working': {
      // Focused micro-nod and subtle lean
      const angleZ = Math.sin(elapsedSeconds * Math.PI * 1.5) * 1.5 * decay
      const angleY = Math.sin(elapsedSeconds * Math.PI * 1.5) * 2 * decay
      return { angleZ, angleY }
    }
    case 'responding': {
      // Affirmative talking nod
      const angleZ = Math.sin(elapsedSeconds * Math.PI) * 1.5 * decay
      const angleY = Math.sin(elapsedSeconds * Math.PI * 2) * 2.5 * decay
      return { angleZ, angleY }
    }
    case 'completed': {
      const completionDuration = Math.min(maxDuration, 1.0)
      if (elapsedSeconds < completionDuration) {
        const nodDecay = 1 - elapsedSeconds / completionDuration
        const angleY = Math.sin(elapsedSeconds * Math.PI * 2) * 5 * nodDecay
        return { angleZ: 0, angleY }
      }
      return { angleZ: 0, angleY: 0 }
    }
    default:
      return { angleZ: 0, angleY: 0 }
  }
}
