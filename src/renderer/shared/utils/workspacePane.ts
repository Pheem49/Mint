export const WORKSPACE_PANE_STORAGE_KEY = 'mint:workspace-panel-width'
const MIN = 280
const MAX = 720
const DEFAULT = 400
const clamp = (width: number, max = MAX) => Math.min(max, Math.max(MIN, Number.isFinite(width) ? width : DEFAULT))

export function workspacePaneLayout(preferred: number, available: number) {
  const max = Math.max(MIN, Math.min(MAX, available - 460 - 18))
  return { width: clamp(preferred, max), min: MIN, max, stacked: available < MIN + 460 + 18 }
}
export function workspacePaneKeyWidth(key: string, layout: ReturnType<typeof workspacePaneLayout>): number | null {
  const width = { ArrowLeft: layout.width - 24, ArrowRight: layout.width + 24, Home: layout.min, End: layout.max, Enter: DEFAULT }[key]
  return width === undefined ? null : clamp(width, layout.max)
}
export function readWorkspacePaneWidth(read: () => string | null) {
  try {
    const value = read()
    return value?.trim() && Number.isFinite(Number(value)) ? clamp(Number(value)) : DEFAULT
  } catch { return DEFAULT }
}
