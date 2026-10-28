const MENU_BOUNDS = { minX: -73, maxX: 152, minY: 36, maxY: 296 }
const MENU_ORIGIN = 110
const VIEWPORT_GUTTER = 8

export function radialMenuOrigin(anchor: { x: number; y: number }, viewportWidth: number, viewportHeight: number) {
  const minLeft = VIEWPORT_GUTTER - MENU_BOUNDS.minX
  const maxLeft = viewportWidth - VIEWPORT_GUTTER - MENU_BOUNDS.maxX
  const minTop = VIEWPORT_GUTTER - MENU_BOUNDS.minY
  const maxTop = viewportHeight - VIEWPORT_GUTTER - MENU_BOUNDS.maxY

  return {
    left: Math.max(minLeft, Math.min(anchor.x - MENU_ORIGIN, maxLeft)),
    top: Math.max(minTop, Math.min(anchor.y - MENU_ORIGIN, maxTop)),
  }
}
