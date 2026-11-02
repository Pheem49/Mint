/**
 * shared/conversation/progressiveStream.ts
 * Deep module for structure-aware progressive AI response rendering (Line-by-Line + Block-Aware).
 * Holds incomplete Markdown tables and unclosed UI Card / JSON / Mermaid fenced blocks until
 * their block completes, while streaming normal paragraphs, lists, alerts, and regular code blocks
 * smoothly line by line.
 */

const ATOMIC_FENCED_LANGS = new Set([
  'ui-grid',
  'ui_grid',
  'ui-grid-json',
  'ui_grid_json',
  'ui-card',
  'ui_card',
  'ui-card-json',
  'ui_card_json',
  'ui-feature',
  'ui_feature',
  'ui-mockup',
  'ui_mockup',
  'ui-mockup-json',
  'ui_mockup_json',
  'weather_json',
  'weather-json',
  'stock_json',
  'stock-json',
  'calculation_json',
  'calculation-json',
  'image_search_json',
  'image-search-json',
  'image_gen_json',
  'image-gen-json',
  'mermaid',
])

function isTableLine(line: string): boolean {
  const trimmed = line.trim()
  return trimmed.startsWith('|') && trimmed.endsWith('|') && trimmed.length > 1
}

function isAtomicFencedLang(lang: string): boolean {
  return ATOMIC_FENCED_LANGS.has(lang.trim().toLowerCase())
}

/**
 * Splits Markdown text into progressive units:
 * - Standard lines & normal code block lines -> 1 line per unit
 * - Consecutive Markdown table lines (`| ... |`) -> 1 atomic multi-line unit
 * - Custom UI / JSON / Mermaid fenced blocks -> 1 atomic multi-line unit
 * When `isComplete` is false, trailing incomplete lines, open tables, and unclosed atomic
 * fenced blocks are held back so half-formed tables or raw JSON never flash in the UI.
 */
export function splitProgressiveStreamUnits(raw: string, isComplete: boolean): string[] {
  if (!raw) return []
  const normalized = raw.replace(/\r\n?/g, '\n')
  let slice = normalized
  if (!isComplete && !normalized.endsWith('\n')) {
    const lastNewline = normalized.lastIndexOf('\n')
    if (lastNewline === -1) return []
    slice = normalized.slice(0, lastNewline + 1)
  }

  const rawLines = slice.endsWith('\n') ? slice.slice(0, -1).split('\n') : slice.split('\n')
  const units: string[] = []
  let inCodeBlock = false
  let pendingCodeFence: { fence: string; canPromote: boolean } | null = null
  let inAtomicFence = false
  let atomicFenceBuffer: string[] = []
  let tableBuffer: string[] = []

  for (const line of rawLines) {
    const trimmed = line.trimStart()

    if (!inCodeBlock && !inAtomicFence && isTableLine(trimmed)) {
      tableBuffer.push(line)
      continue
    }

    if (tableBuffer.length > 0) {
      units.push(tableBuffer.join('\n'))
      tableBuffer = []
    }

    if (trimmed.startsWith('```')) {
      if (!inCodeBlock && !inAtomicFence) {
        const afterOpen = trimmed.replace(/^`+/, '').trim()
        if (afterOpen.endsWith('```') && afterOpen.length > 3) {
          units.push(line)
          continue
        }
        const splitIdx = afterOpen.search(/[\s[{]/)
        const lang = splitIdx > 0 ? afterOpen.slice(0, splitIdx) : afterOpen
        if (isAtomicFencedLang(lang)) {
          inAtomicFence = true
          atomicFenceBuffer = [line]
        } else {
          inCodeBlock = true
          const canPromote = !lang || lang.toLowerCase() === 'plaintext'
          pendingCodeFence = { fence: line, canPromote }
        }
      } else if (inAtomicFence) {
        atomicFenceBuffer.push(line)
        units.push(atomicFenceBuffer.join('\n'))
        atomicFenceBuffer = []
        inAtomicFence = false
      } else {
        inCodeBlock = false
        if (pendingCodeFence) {
          units.push(`${pendingCodeFence.fence}\n${line}`)
          pendingCodeFence = null
        } else {
          units.push(line)
        }
      }
      continue
    }

    if (inAtomicFence) {
      atomicFenceBuffer.push(line)
      continue
    }

    if (pendingCodeFence) {
      const { fence, canPromote } = pendingCodeFence
      pendingCodeFence = null
      const splitIdx = trimmed.search(/[\s[{]/)
      const firstToken = splitIdx > 0 ? trimmed.slice(0, splitIdx) : trimmed
      if (canPromote && isAtomicFencedLang(firstToken)) {
        inCodeBlock = false
        inAtomicFence = true
        atomicFenceBuffer = [fence, line]
      } else {
        units.push(`${fence}\n${line}`)
      }
      continue
    }

    units.push(line)
  }

  if (isComplete) {
    if (tableBuffer.length > 0) {
      units.push(tableBuffer.join('\n'))
    }
    if (pendingCodeFence) {
      units.push(pendingCodeFence.fence)
      pendingCodeFence = null
    }
    if (atomicFenceBuffer.length > 0) {
      units.push(atomicFenceBuffer.join('\n'))
    }
  }

  return units
}

/**
 * Returns the safe-to-render prefix of a streaming Markdown document.
 */
export function extractRenderableStreamPrefix(raw: string, isComplete: boolean): string {
  const units = splitProgressiveStreamUnits(raw, isComplete)
  return units.join('\n')
}

export interface ProgressiveStreamPacer {
  pushDelta(delta: string): void
  finish(authoritativeText?: string): void
  step(): string | null
  flushAll(): string
  isDone(): boolean
}

export function createProgressiveStreamPacer(
  onReveal?: (revealedText: string) => void,
): ProgressiveStreamPacer {
  let rawBuffer = ''
  let completed = false
  let revealedUnits = 0

  return {
    pushDelta(delta: string) {
      if (delta) {
        rawBuffer += delta
      }
    },
    finish(authoritativeText?: string) {
      if (typeof authoritativeText === 'string') {
        rawBuffer = authoritativeText
      }
      completed = true
    },
    step(): string | null {
      const units = splitProgressiveStreamUnits(rawBuffer, completed)
      if (revealedUnits >= units.length) {
        return null
      }
      const remaining = units.length - revealedUnits
      const advance = remaining <= 12 ? 1 : Math.min(4, Math.max(1, Math.floor(remaining / 10)))
      revealedUnits = Math.min(units.length, revealedUnits + advance)
      const revealed = units.slice(0, revealedUnits).join('\n')
      onReveal?.(revealed)
      return revealed
    },
    flushAll(): string {
      completed = true
      const units = splitProgressiveStreamUnits(rawBuffer, true)
      revealedUnits = units.length
      const revealed = units.join('\n')
      onReveal?.(revealed)
      return revealed
    },
    isDone(): boolean {
      if (!completed) return false
      const units = splitProgressiveStreamUnits(rawBuffer, true)
      return revealedUnits >= units.length
    },
  }
}
