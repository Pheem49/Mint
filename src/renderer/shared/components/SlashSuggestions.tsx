import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from 'react'
import { SLASH_COMMANDS, type SlashCommand } from '../constants/slashCommands'
import { listLearnedSkills, type LearnedSkill } from '@/tauri'

/** Parent calls `handleKeyDown` from the textarea's onKeyDown; a `true` return
 *  means the menu consumed the key and the caller should stop. */
export interface SlashSuggestionsHandle {
  handleKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => boolean
}

interface Props {
  /** Current composer text. */
  message: string
  /** Replace the whole composer text with `text` (a completed `/cmd `, `$skill `,
   *  or the message with its trailing `@token` swapped). */
  onComplete: (text: string) => void
  /** Workspace path, for loading learned skills. */
  workspacePath?: string
  /** `settingsConfig.mcpServers` — `{ [name]: { disabled?: boolean } }`. */
  mcpServers?: Record<string, { disabled?: boolean } | undefined>
  /** The composer textarea, so an outside-click that lands on it doesn't dismiss. */
  anchorRef: RefObject<HTMLTextAreaElement | null>
  /** While true the menu is fully inert (used during history browsing). */
  suppressed?: boolean
}

type Mode = 'slash' | 'skill' | 'at' | null

const CATEGORY_ORDER = ['system', 'models', 'workspace', 'tools'] as const
const CATEGORY_LABEL: Record<(typeof CATEGORY_ORDER)[number], string> = {
  system: 'System',
  models: 'Models',
  workspace: 'Workspace',
  tools: 'Tools',
}

interface ContextItem {
  label: string
  desc: string
  kind: 'context' | 'plugin'
}

const BUILTIN_CONTEXTS: ContextItem[] = [
  { label: '@workspace', desc: 'Include workspace path & context', kind: 'context' },
  { label: '@file', desc: 'Reference workspace file', kind: 'context' },
  { label: '@docs', desc: 'Include documentation context', kind: 'context' },
  { label: '@memory', desc: 'Include long-term memory store', kind: 'context' },
]

const SlashSuggestions = forwardRef<SlashSuggestionsHandle, Props>(function SlashSuggestions(
  { message, onComplete, workspacePath, mcpServers, anchorRef, suppressed = false },
  ref,
) {
  const [skills, setSkills] = useState<LearnedSkill[]>([])
  const [selIndex, setSelIndex] = useState(0)
  // The exact composer text the menu was last dismissed for (a pick, Escape, or
  // an outside click). It reopens as soon as `message` diverges from this.
  const [dismissedFor, setDismissedFor] = useState<string | null>(null)

  const popupRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  const raw = message.replace(/^\s+/, '')
  const mode: Mode = raw.startsWith('/')
    ? 'slash'
    : raw.startsWith('$')
    ? 'skill'
    : /@[\w\-./]*$/.test(message)
    ? 'at'
    : null

  useEffect(() => {
    if (mode !== 'skill') return
    let alive = true
    listLearnedSkills(workspacePath)
      .then((res) => {
        if (alive && Array.isArray(res)) setSkills(res)
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [mode, workspacePath])

  // ── Slash: prefix-match the command token, grouped by category ──
  const slashMatches = useMemo(() => {
    if (mode !== 'slash') return [] as SlashCommand[]
    const q = raw.toLowerCase()
    const hits = SLASH_COMMANDS.filter((c) => c.command.toLowerCase().startsWith(q))
    return CATEGORY_ORDER.flatMap((cat) => hits.filter((h) => (h.category ?? 'tools') === cat))
  }, [mode, raw])

  // The command whose arguments are being typed (raw is past its token) — used
  // for the usage hint shown when there are no more prefix matches.
  const activeSlashCmd = useMemo(() => {
    if (mode !== 'slash') return undefined
    const q = raw.toLowerCase()
    return [...SLASH_COMMANDS]
      .sort((a, b) => b.command.length - a.command.length)
      .find((c) => q === c.command.toLowerCase() || q.startsWith(c.command.toLowerCase() + ' '))
  }, [mode, raw])

  // ── Skill: `$name` prefix-match ──
  const skillMatches = useMemo(() => {
    if (mode !== 'skill') return [] as LearnedSkill[]
    const q = raw.slice(1).split(/\s+/)[0].toLowerCase()
    return skills.filter((s) => s.name.toLowerCase().startsWith(q))
  }, [mode, raw, skills])

  // ── @mention: builtins + enabled MCP servers, substring match ──
  const atMatches = useMemo(() => {
    if (mode !== 'at') return [] as ContextItem[]
    const q = (message.match(/@([\w\-./]*)$/)?.[1] ?? '').toLowerCase()
    const mcp: ContextItem[] = Object.entries(mcpServers ?? {})
      .filter(([, srv]) => srv?.disabled !== true)
      .map(([name]) => ({
        label: `@${name}`,
        desc: 'Restrict this message to this MCP server / plugin',
        kind: 'plugin' as const,
      }))
    return [...BUILTIN_CONTEXTS, ...mcp].filter((c) => c.label.toLowerCase().includes(q))
  }, [mode, message, mcpServers])

  // Flat, render-order list the keyboard walks over.
  const flat: Array<SlashCommand | LearnedSkill | ContextItem> =
    mode === 'slash' ? slashMatches : mode === 'skill' ? skillMatches : mode === 'at' ? atMatches : []

  const hasEmptyState = mode === 'slash' && flat.length === 0 && raw.trim().length > 1
  const open =
    !suppressed && mode !== null && dismissedFor !== message && (flat.length > 0 || hasEmptyState)

  // Reset the highlight whenever the query text changes (not on arrow / hover).
  useEffect(() => {
    setSelIndex(0)
  }, [message])

  // Keep the highlighted row visible.
  useEffect(() => {
    if (!open || !listRef.current) return
    const el = listRef.current.querySelector<HTMLElement>(`[data-idx="${selIndex}"]`)
    el?.scrollIntoView({ block: 'nearest' })
  }, [selIndex, open])

  // Dismiss on a click that isn't on the popup or the composer.
  useEffect(() => {
    if (!open) return
    const onDown = (event: MouseEvent) => {
      const target = event.target as Node
      if (popupRef.current?.contains(target) || anchorRef.current?.contains(target)) return
      setDismissedFor(message)
    }
    document.addEventListener('mousedown', onDown)
    return () => document.removeEventListener('mousedown', onDown)
  }, [open, message, anchorRef])

  const completionFor = (item: SlashCommand | LearnedSkill | ContextItem): string => {
    if (mode === 'slash') return `${(item as SlashCommand).command} `
    if (mode === 'skill') return `$${(item as LearnedSkill).name} `
    return message.replace(/@[\w\-./]*$/, `${(item as ContextItem).label} `)
  }

  const pick = (item: SlashCommand | LearnedSkill | ContextItem) => {
    const text = completionFor(item)
    setDismissedFor(text)
    setSelIndex(0)
    onComplete(text)
  }

  useImperativeHandle(ref, () => ({
    handleKeyDown(event) {
      if (suppressed || mode === null) return false
      const isNav =
        (event.key === 'ArrowUp' || event.key === 'ArrowDown') &&
        !event.shiftKey &&
        !event.altKey &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.nativeEvent.isComposing

      // Arrows bring a just-dismissed but still-relevant menu back, rather than
      // falling through to history browsing.
      if (isNav && !open && flat.length > 0) {
        setDismissedFor(null)
        setSelIndex(event.key === 'ArrowDown' ? 0 : flat.length - 1)
        event.preventDefault()
        return true
      }
      if (!open) return false

      if (isNav && flat.length > 0) {
        setSelIndex((i) =>
          event.key === 'ArrowDown'
            ? (i + 1) % flat.length
            : (i - 1 + flat.length) % flat.length,
        )
        event.preventDefault()
        return true
      }
      if ((event.key === 'Enter' || event.key === 'Tab') && !event.shiftKey && !event.nativeEvent.isComposing) {
        const item = flat[selIndex] ?? flat[0]
        if (!item) return false
        pick(item)
        event.preventDefault()
        return true
      }
      if (event.key === 'Escape') {
        setDismissedFor(message)
        event.preventDefault()
        return true
      }
      return false
    },
  }))

  if (!open) return null

  const headerLabel =
    flat.length === 0
      ? 'No matches'
      : mode === 'slash'
      ? `${flat.length} command${flat.length === 1 ? '' : 's'}`
      : mode === 'skill'
      ? `${flat.length} skill${flat.length === 1 ? '' : 's'}`
      : `${flat.length} mention${flat.length === 1 ? '' : 's'}`

  const row = (
    item: SlashCommand | LearnedSkill | ContextItem,
    idx: number,
    name: string,
    desc: string,
    badge?: string,
    usage?: string,
  ) => (
    <button
      key={name}
      type="button"
      role="option"
      aria-selected={idx === selIndex}
      data-idx={idx}
      className={`slash-suggestion-item ${idx === selIndex ? 'active' : ''}`}
      onMouseDown={(e) => {
        e.preventDefault()
        pick(item)
      }}
      onPointerMove={() => setSelIndex(idx)}
    >
      <span className="slash-cmd-name">{name}</span>
      {badge && <span className="skill-badge">{badge}</span>}
      <span className="slash-cmd-body">
        <span className="slash-cmd-desc">{desc}</span>
        {usage && <span className="slash-cmd-usage">{usage}</span>}
      </span>
    </button>
  )

  return (
    <div className="slash-suggestions-popup" ref={popupRef}>
      <div className="slash-suggestions-header">
        {mode === 'slash' ? 'Slash Commands' : mode === 'skill' ? 'Learned Skills' : 'Context Mentions'}
        <span className="slash-suggestions-count">{headerLabel}</span>
      </div>
      <div className="slash-suggestions-list" ref={listRef} role="listbox">
        {mode === 'slash' &&
          (flat.length > 0
            ? CATEGORY_ORDER.map((cat) => {
                const items = slashMatches.filter((m) => (m.category ?? 'tools') === cat)
                if (!items.length) return null
                return (
                  <div key={cat} className="slash-suggestions-group">
                    <div className="slash-suggestions-group-label">{CATEGORY_LABEL[cat]}</div>
                    {items.map((item) =>
                      row(item, slashMatches.indexOf(item), item.command, item.description, undefined, item.usage),
                    )}
                  </div>
                )
              })
            : (
              <div className="slash-suggestions-empty">
                {activeSlashCmd ? (
                  <>
                    <span className="slash-cmd-name">{activeSlashCmd.command}</span>
                    <span className="slash-cmd-usage">
                      {activeSlashCmd.usage || 'No further arguments'}
                    </span>
                  </>
                ) : (
                  <span>
                    No command matches <code>{raw.trim()}</code>
                  </span>
                )}
              </div>
            ))}

        {mode === 'skill' &&
          skillMatches.map((item, idx) =>
            row(item, idx, `$${item.name}`, item.description || item.content?.slice(0, 60) || '', '[Skill]'),
          )}

        {mode === 'at' &&
          atMatches.map((item, idx) =>
            row(item, idx, item.label, item.desc, item.kind === 'plugin' ? '[Plugin]' : '[Context]'),
          )}
      </div>
    </div>
  )
})

export default SlashSuggestions
