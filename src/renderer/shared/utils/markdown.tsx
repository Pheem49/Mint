/**
 * shared/utils/markdown.tsx
 * Markdown parsing and rendering for message bubbles, built on react-markdown + remark-gfm.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import { Children, cloneElement, Fragment, isValidElement, type CSSProperties, type ReactElement, type ReactNode } from 'react'
import ReactMarkdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { ChatCodeBlock } from '../components/ChatCodeBlock'
import WeatherCard from '../components/WeatherCard'
import StockCard from '../components/StockCard'
import CalculationCard from '../components/CalculationCard'
import ImageSearchCard from '../components/ImageSearchCard'
import ImageGenCard from '../components/ImageGenCard'
import MermaidCard from '../components/MermaidCard'
import UiGridCard from '../components/UiGridCard'
import { parseCardJsonSafely } from './cardJson'
import UiFeatureCard from '../components/UiFeatureCard'
import UiMockupWidget from '../components/UiMockupWidget'
import {
  Info,
  Lightbulb,
  AlertCircle,
  AlertTriangle,
  ShieldAlert,
  type LucideIcon,
} from 'lucide-react'

/**
 * Unwraps Brave Search internal image proxy URLs (imgs.search.brave.com)
 * back to the direct source image URL so it loads without 403 Forbidden.
 */
export function unwrapBraveImageUrl(url: string): string {
  if (!url || !url.includes('imgs.search.brave.com')) return url
  try {
    const match = url.match(/imgs\.search\.brave\.com\/[^\/]+\/[^\/]+\/[^\/]+\/(.+)$/)
    if (!match) return url
    const rawB64 = match[1].replace(/\//g, '')
    const padded = rawB64 + '='.repeat((4 - (rawB64.length % 4)) % 4)
    const stdB64 = padded.replace(/-/g, '+').replace(/_/g, '/')
    const decoded = atob(stdB64)
    const unescaped = decoded.replace(/&amp;/g, '&')
    if (unescaped.startsWith('http://') || unescaped.startsWith('https://')) {
      return unescaped
    }
  } catch {
    // If decoding fails, fall back to original url
  }
  return url
}

export const resolveMediaUrl = (url: string): string => {
  if (!url) return ''
  const unwrapped = unwrapBraveImageUrl(url)
  if (unwrapped.startsWith('data:') || unwrapped.startsWith('http://') || unwrapped.startsWith('https://')) {
    return unwrapped
  }
  if (unwrapped.startsWith('/api/')) {
    const origin = typeof window !== 'undefined' && window.location.port === '9000'
      ? 'http://localhost:3000'
      : (typeof window !== 'undefined' ? window.location.origin : '')
    return `${origin}${unwrapped}`
  }
  return unwrapped
}

export const isTableLine = (line: string): boolean => {
  const trimmed = line.trim()
  return trimmed.startsWith('|') && trimmed.endsWith('|') && trimmed.length > 1
}

export function readableAssistantText(text: string): string {
  if (typeof text !== 'string') return ''
  const trimmed = text.trim()
  if (!trimmed.startsWith('{')) return sanitizeLatex(text)
  try {
    const value = JSON.parse(trimmed)
    if (value?.action === 'finish' && typeof value?.input?.summary === 'string' && value.input.summary.trim()) {
      return sanitizeLatex(value.input.summary)
    }
    if (typeof value?.finish?.summary === 'string' && value.finish.summary.trim()) {
      return sanitizeLatex(value.finish.summary)
    }
  } catch {
    return sanitizeLatex(text)
  }
  return sanitizeLatex(text)
}

/** Replace common LaTeX math symbols with their Unicode equivalents */
function sanitizeLatex(text: string): string {
  return text
    // arrows
    .replace(/\$\\rightarrow\$|\\\(\\rightarrow\\\)|\\rightarrow|\bightarrow\b/g, '→')
    .replace(/\$\\leftarrow\$|\\\(\\leftarrow\\\)|\\leftarrow|\beftarrow\b/g, '←')
    .replace(/\$\\Rightarrow\$|\\\(\\Rightarrow\\\)|\\Rightarrow|\begRightarrow\b/g, '⇒')
    .replace(/\$\\Leftarrow\$|\\\(\\Leftarrow\\\)|\\Leftarrow/g, '⇐')
    .replace(/\$\\leftrightarrow\$|\\\(\\leftrightarrow\\\)|\\leftrightarrow/g, '↔')
    // comparison
    .replace(/\$\\leq\$|\\\(\\leq\\\)|\\leq/g, '≤')
    .replace(/\$\\geq\$|\\\(\\geq\\\)|\\geq/g, '≥')
    .replace(/\$\\neq\$|\\\(\\neq\\\)|\\neq/g, '≠')
    .replace(/\$\\approx\$|\\\(\\approx\\\)/g, '≈')
    // math
    .replace(/\$\\times\$|\\\(\\times\\\)|\\times/g, '×')
    .replace(/\$\\div\$|\\\(\\div\\\)|\\div/g, '÷')
    .replace(/\$\\pm\$|\\\(\\pm\\\)/g, '±')
    .replace(/\$\\infty\$|\\\(\\infty\\\)|\\infty/g, '∞')
    // other common symbols
    .replace(/\$\\cdot\$|\\\(\\cdot\\\)|\\cdot/g, '·')
    .replace(/\$\\in\$|\\\(\\in\\\)|\\in\b/g, '∈')
    .replace(/\$\\subset\$|\\\(\\subset\\\)|\\subset/g, '⊂')
    .replace(/\$\\cup\$|\\\(\\cup\\\)|\\cup\b/g, '∪')
    .replace(/\$\\cap\$|\\\(\\cap\\\)|\\cap\b/g, '∩')
    // strip remaining inline math delimiters $...$ and \(...\) that weren't matched
    .replace(/\$([^$\n]{1,60})\$/g, '$1')
    .replace(/\\\(([^)]{1,60})\\\)/g, '$1')
}



export function cleanSpeechText(text: string): string {
  return readableAssistantText(text)
    .replace(/\*\*([\s\S]*?)\*\*/g, '$1')
    .replace(/[*_`#]/g, '')
    .trim()
}

export function renderSpeakerIcon(isSpeaking: boolean): ReactNode {
  if (isSpeaking) {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
        <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
        <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
      </svg>
    )
  }
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
      <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
    </svg>
  )
}

export function renderCopyIcon(isCopied: boolean): ReactNode {
  if (isCopied) {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block', color: 'var(--accent, #10b981)' }}>
        <polyline points="20 6 9 17 4 12" />
      </svg>
    )
  }
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
  )
}

// --- react-markdown wiring ---------------------------------------------

/** Recursively flattens rendered children back into plain text (for heuristics that need the raw string). */
function flattenText(node: ReactNode): string {
  if (node == null || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(flattenText).join('')
  if (isValidElement(node)) return flattenText((node.props as { children?: ReactNode }).children)
  return ''
}

/** Wraps bare "@mention" tokens with styled spans and strips any leftover "[badge:color text]" into plain natural text. */
function highlightMentions(node: ReactNode, keyPrefix = 'm'): ReactNode {
  if (typeof node === 'string') {
    const parts = node.split(/(@[\w\-.\/]+|\[badge(?::[a-zA-Z]+)?\s+[^\]]+\])/g)
    if (parts.length <= 1) return node
    return parts.map((part, i) => {
      if (i % 2 === 1) {
        if (part.startsWith('@')) {
          return (
            <span key={`${keyPrefix}-${i}`} className="chat-mention">
              {part}
            </span>
          )
        }
        const badgeMatch = part.match(/^\[badge(?::([a-zA-Z]+))?\s+([^\]]+)\]$/)
        if (badgeMatch) {
          // Render plain natural text without robotic badge frame/pill
          return <Fragment key={`${keyPrefix}-${i}`}>{badgeMatch[2]}</Fragment>
        }
      }
      return part ? <Fragment key={`${keyPrefix}-t-${i}`}>{part}</Fragment> : null
    })
  }
  if (Array.isArray(node)) {
    return node.map((child, i) => (
      <Fragment key={`${keyPrefix}-${i}`}>{highlightMentions(child, `${keyPrefix}-${i}`)}</Fragment>
    ))
  }
  return node
}

/** Section headers like "🚀 ขั้นตอนเริ่มต้น:" — a whole paragraph that's just an emoji-led title ending in a colon. */
const EMOJI_HEADER_RE = /^(?:[\u{1F300}-\u{1F9FF}]|[\u{2600}-\u{26FF}]|[\u{2700}-\u{27BF}]).*:$/u

function readHastText(node: unknown): string {
  const n = node as { type?: string; value?: string; children?: unknown[] } | undefined
  if (!n) return ''
  if (n.type === 'text') return n.value ?? ''
  if (Array.isArray(n.children)) return n.children.map(readHastText).join('')
  return ''
}

function findCodeHastNode(preNode: unknown): { properties?: { className?: unknown }; children?: unknown[] } | undefined {
  const n = preNode as { children?: Array<{ type?: string; tagName?: string }> } | undefined
  return n?.children?.find((c) => c.type === 'element' && c.tagName === 'code') as
    | { properties?: { className?: unknown }; children?: unknown[] }
    | undefined
}

/** Clones each `<li>` child with `ordered`/`index` props so `li` can render a bullet or a number without shared state. */
function tagListItems(children: ReactNode, opts: { ordered: boolean; start?: number }): ReactNode {
  // `Children.toArray` can include stray whitespace text nodes between <li> siblings (from
  // loose-list formatting) — those must not consume a number, so index only real elements.
  let n = 0
  return Children.toArray(children).map((child) => {
    if (!isValidElement(child)) return child
    const index = (opts.start ?? 1) + n
    n += 1
    return cloneElement(child as ReactElement<{ ordered?: boolean; index?: number }>, {
      key: child.key ?? index,
      ordered: opts.ordered,
      index,
    })
  })
}

/** Strips leading/trailing whitespace-only text-node children (the same stray "\n" nodes
 * loose-list formatting inserts between siblings, see `tagListItems` above, but here found
 * as a direct child of a single <li>). `.chat-list-text` renders with `white-space: pre-wrap`
 * so that whitespace renders as a literal blank line, dropping the actual text onto its own
 * row below the bullet/number instead of beside it. */
function trimListItemEdges(children: ReactNode): ReactNode {
  const arr = Children.toArray(children)
  while (arr.length && typeof arr[0] === 'string' && arr[0].trim() === '') arr.shift()
  while (arr.length && typeof arr[arr.length - 1] === 'string' && (arr[arr.length - 1] as string).trim() === '') arr.pop()
  return arr
}

function parseJsonSafely(codeText: string): any {
  if (!codeText || !codeText.trim()) return null
  let cleaned = codeText.trim()
  if (cleaned.startsWith('```')) {
    cleaned = cleaned.replace(/^```[a-zA-Z0-9_-]*\s*/, '').replace(/\s*```$/, '').trim()
  }
  return parseCardJsonSafely(cleaned)
}

function renderJsonCard(
  codeText: string,
  lang: string,
  render: (data: any) => ReactNode,
): ReactNode {
  const parsed = parseJsonSafely(codeText)
  return parsed
    ? render(parsed)
    : <ChatCodeBlock code={codeText} language={lang} />
}

function renderCodeCard(lang: string, codeText: string): ReactNode {
  switch (lang) {
    case 'weather_json':
    case 'weather-json':
      return renderJsonCard(codeText, lang, (data) => <WeatherCard data={data} />)
    case 'stock_json':
    case 'stock-json':
      return renderJsonCard(codeText, lang, (data) => <StockCard data={data} />)
    case 'calculation_json':
    case 'calculation-json':
      return renderJsonCard(codeText, lang, (data) => <CalculationCard data={data} />)
    case 'image_search_json':
    case 'image-search-json':
      return renderJsonCard(codeText, lang, (data) => <ImageSearchCard data={data} />)
    case 'image_gen_json':
    case 'image-gen-json':
      return renderJsonCard(codeText, lang, (data) => <ImageGenCard data={data} />)
    case 'mermaid':
      return <MermaidCard code={codeText} />
    case 'ui_grid':
    case 'ui-grid':
    case 'ui_grid_json':
    case 'ui-grid-json': {
      let clean = codeText.trim()
      if (/^ui[-_]grid(?:[-_]json)?\s*/i.test(clean)) {
        clean = clean.replace(/^ui[-_]grid(?:[-_]json)?\s*/i, '')
      }
      const parsed = parseJsonSafely(clean)
      if (parsed) return <UiGridCard data={parsed} />
      return <ChatCodeBlock code={codeText} language={lang} />
    }
    case 'ui_card':
    case 'ui-card':
    case 'ui_card_json':
    case 'ui-card-json':
    case 'ui_feature':
    case 'ui-feature': {
      let clean = codeText.trim()
      if (/^ui[-_](?:card|feature)(?:[-_]json)?\s*/i.test(clean)) {
        clean = clean.replace(/^ui[-_](?:card|feature)(?:[-_]json)?\s*/i, '')
      }
      const parsed = parseJsonSafely(clean)
      if (parsed) return <UiFeatureCard data={parsed} />
      return <ChatCodeBlock code={codeText} language={lang} />
    }
    case 'ui_mockup':
    case 'ui-mockup':
    case 'ui_mockup_json':
    case 'ui-mockup-json': {
      let clean = codeText.trim()
      if (/^ui[-_]mockup(?:[-_]json)?\s*/i.test(clean)) {
        clean = clean.replace(/^ui[-_]mockup(?:[-_]json)?\s*/i, '')
      }
      const parsed = parseJsonSafely(clean)
      if (parsed) return <UiMockupWidget data={parsed} />
      return <ChatCodeBlock code={codeText} language={lang} />
    }
    default:
      return <ChatCodeBlock code={codeText} language={lang} />
  }
}

function stripAlertTag(node: ReactNode): ReactNode {
  let done = false
  function clean(item: ReactNode): ReactNode {
    if (done || item == null) return item
    if (typeof item === 'string') {
      const match = item.match(/^\s*\[!(?:NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*(?:\r?\n)?/i)
      if (match) {
        done = true
        return item.slice(match[0].length)
      }
      return item
    }
    if (Array.isArray(item)) {
      return item.map(clean).filter((v) => v !== '' && v != null)
    }
    if (isValidElement(item)) {
      const props = item.props as { children?: ReactNode }
      if (props.children) {
        return cloneElement(item, {}, clean(props.children))
      }
    }
    return item
  }
  return clean(node)
}

function renderAlertBox(type: string, children: ReactNode): ReactNode {
  const config: { title: string; color: string; border: string; bg: string; borderSubtle: string; Icon: LucideIcon } = {
    NOTE: { title: 'Note', color: '#38bdf8', border: '#38bdf8', bg: 'rgba(56, 189, 248, 0.08)', borderSubtle: 'rgba(56, 189, 248, 0.25)', Icon: Info },
    TIP: { title: 'Tip', color: '#34d399', border: '#10b981', bg: 'rgba(16, 185, 129, 0.08)', borderSubtle: 'rgba(16, 185, 129, 0.25)', Icon: Lightbulb },
    IMPORTANT: { title: 'Important', color: '#c084fc', border: '#a855f7', bg: 'rgba(168, 85, 247, 0.08)', borderSubtle: 'rgba(168, 85, 247, 0.25)', Icon: AlertCircle },
    WARNING: { title: 'Warning', color: '#fbbf24', border: '#f59e0b', bg: 'rgba(245, 158, 11, 0.08)', borderSubtle: 'rgba(245, 158, 11, 0.25)', Icon: AlertTriangle },
    CAUTION: { title: 'Caution', color: '#f87171', border: '#ef4444', bg: 'rgba(239, 68, 68, 0.08)', borderSubtle: 'rgba(239, 68, 68, 0.25)', Icon: ShieldAlert },
  }[type] || { title: 'Note', color: '#38bdf8', border: '#38bdf8', bg: 'rgba(56, 189, 248, 0.08)', borderSubtle: 'rgba(56, 189, 248, 0.25)', Icon: Info }

  const IconComponent = config.Icon
  return (
    <div
      className={`chat-callout-card chat-callout--${type.toLowerCase()}`}
      style={{
        margin: '14px 0',
        padding: '12px 16px',
        borderRadius: '10px',
        borderLeft: `4px solid ${config.border}`,
        borderTop: `1px solid ${config.borderSubtle}`,
        borderRight: `1px solid ${config.borderSubtle}`,
        borderBottom: `1px solid ${config.borderSubtle}`,
        background: config.bg,
        boxShadow: '0 2px 10px rgba(0, 0, 0, 0.15)',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          marginBottom: '6px',
          color: config.color,
          fontWeight: 700,
          fontSize: '0.86rem',
          letterSpacing: '0.3px',
          textTransform: 'none',
        }}
      >
        <IconComponent size={16} strokeWidth={2.2} />
        <span>{config.title}</span>
      </div>
      <div style={{ fontSize: '0.86rem', color: 'var(--text-chat, #cbd5e1)', lineHeight: 1.5 }}>
        {children}
      </div>
    </div>
  )
}

// `ol`/`ul` inject `ordered`/`index` onto their `li` children (via cloneElement below) so each
// list item knows how to render itself without needing shared state; react-markdown's own
// component types don't declare these, so the object is cast to `Components` where it's used.
const mdComponents = {
  p({ children }) {
    const text = flattenText(children).trim()
    if (EMOJI_HEADER_RE.test(text)) {
      return <div className="chat-heading chat-heading-3 chat-section-title">{children}</div>
    }
    const alertMatch = text.match(/^\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]/i)
    if (alertMatch) {
      return renderAlertBox(alertMatch[1].toUpperCase(), stripAlertTag(children))
    }
    // A plain <div> (not <p>) so block-level media cards can nest inside safely.
    return <div className="chat-paragraph">{highlightMentions(children)}</div>
  },
  h1: ({ children }) => <div className="chat-heading chat-heading-1">{highlightMentions(children)}</div>,
  h2: ({ children }) => <div className="chat-heading chat-heading-2">{highlightMentions(children)}</div>,
  h3: ({ children }) => <div className="chat-heading chat-heading-3">{highlightMentions(children)}</div>,
  h4: ({ children }) => <div className="chat-heading chat-heading-3">{highlightMentions(children)}</div>,
  h5: ({ children }) => <div className="chat-heading chat-heading-3">{highlightMentions(children)}</div>,
  h6: ({ children }) => <div className="chat-heading chat-heading-3">{highlightMentions(children)}</div>,
  ul: ({ children }) => <div className="chat-list-block">{tagListItems(children, { ordered: false })}</div>,
  ol: ({ children, start }) => (
    <div className="chat-list-block">{tagListItems(children, { ordered: true, start: typeof start === 'number' ? start : 1 })}</div>
  ),
  li: ({ children, ordered, index }: { children?: ReactNode; ordered?: boolean; index?: number }) => {
    const content = trimListItemEdges(children)
    if (ordered) {
      return (
        <div className="chat-list-item chat-ordered-item">
          <span className="chat-list-number">{index}.</span>
          <div className="chat-list-text">{highlightMentions(content)}</div>
        </div>
      )
    }
    // chat-list-text is a div, not a span: "loose" lists (items separated by blank lines)
    // wrap their content in a block-level <div class="chat-paragraph">, which a <span> can't
    // contain without forcing a line break onto its own line.
    return (
      <div className="chat-list-item chat-bullet-item">
        <span className="chat-list-bullet">•</span>
        <div className="chat-list-text">{highlightMentions(content)}</div>
      </div>
    )
  },
  strong: ({ children }) => <strong className="chat-bold-highlight">{children}</strong>,
  a: ({ href, children }) => (
    <a href={href} target="_blank" rel="noopener noreferrer" className="chat-link">
      {highlightMentions(children)}
    </a>
  ),
  img: ({ src, alt }) => {
    const rawUrl = String(src || '')
    const url = resolveMediaUrl(rawUrl)
    const label = alt || 'Generated Image'
    const isExternal = url.startsWith('https://') || url.startsWith('http://')
    if (isExternal) {
      return (
        <a
          href={url}
          target="_blank"
          rel="noopener noreferrer"
          title={label}
          className="chat-media-card chat-media-card--thumbnail"
          style={{ display: 'block', margin: '6px 0 10px 0', borderRadius: '10px', overflow: 'hidden', border: '1px solid var(--border, rgba(255,255,255,0.12))', background: 'var(--panel-bg, #141416)', maxWidth: '420px', textDecoration: 'none', cursor: 'pointer' }}
        >
          <img
            src={url}
            alt={label}
            loading="lazy"
            referrerPolicy="no-referrer"
            style={{ width: '100%', maxHeight: '200px', objectFit: 'cover', display: 'block' }}
            onError={(e) => { (e.currentTarget as HTMLImageElement).closest('a')!.style.display = 'none' }}
          />
          {label !== 'Generated Image' && (
            <div style={{ padding: '5px 10px', fontSize: '0.72rem', color: 'var(--text-muted, #64748b)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{label}</div>
          )}
        </a>
      )
    }
    return (
      <div className="chat-media-card" style={{ margin: '10px 0', borderRadius: '12px', overflow: 'hidden', border: '1px solid var(--border, rgba(255,255,255,0.12))', background: 'var(--panel-bg, #141416)' }}>
        <img src={url} alt={label} referrerPolicy="no-referrer" style={{ width: '100%', maxHeight: '420px', objectFit: 'contain', display: 'block', borderRadius: '8px' }} />
      </div>
    )
  },
  code({ children }) {
    // Detect single-line UI widgets parsed as inline code (e.g. `ui-card [...]` or ```ui-card [...]```)
    const rawText = typeof children === 'string'
      ? children
      : Array.isArray(children) && children.every((c) => typeof c === 'string')
        ? children.join('')
        : ''
    if (rawText) {
      const trimmed = rawText.trim()
      const match = trimmed.match(/^(ui[-_](?:card|grid|mockup|feature)(?:[-_]json)?)\s+([\s\S]+)$/i)
      if (match) {
        const lang = match[1].replace(/_/g, '-').toLowerCase()
        const parsed = parseJsonSafely(match[2].trim())
        if (parsed) {
          if (lang.startsWith('ui-grid')) return <UiGridCard data={parsed} />
          if (lang.startsWith('ui-card') || lang.startsWith('ui-feature')) return <UiFeatureCard data={parsed} />
          if (lang.startsWith('ui-mockup')) return <UiMockupWidget data={parsed} />
        }
      }
    }
    // Only reached for regular inline code — block code is intercepted by `pre` below.
    return <code className="chat-inline-code">{children}</code>
  },
  pre({ node }) {
    const codeNode = findCodeHastNode(node)
    const rawClassName = codeNode?.properties?.className
    const classNames = Array.isArray(rawClassName)
      ? rawClassName.map(String)
      : typeof rawClassName === 'string'
        ? rawClassName.split(/\s+/)
        : []
    const langClass = classNames.find((c) => c.startsWith('language-'))
    let lang = langClass ? langClass.slice('language-'.length) : 'plaintext'
    let codeText = readHastText(codeNode).replace(/\n$/, '')

    // Check if AST node has data.meta with arguments/json (e.g. ```ui-card [{"title": ...}]```)
    const meta = (codeNode as any)?.data?.meta
    if (typeof meta === 'string' && meta.trim()) {
      const trimmedMeta = meta.trim()
      if (!codeText.trim()) {
        codeText = trimmedMeta
      } else if (trimmedMeta.startsWith('[') || trimmedMeta.startsWith('{')) {
        codeText = trimmedMeta + '\n' + codeText
      }
    }

    // If lang has JSON attached (e.g. language-ui-card [{"title": ...}])
    if (lang.includes('[') || lang.includes('{')) {
      const splitIdx = lang.search(/[\s\[\{]/)
      if (splitIdx > 0) {
        const extractedLang = lang.slice(0, splitIdx)
        const rest = lang.slice(splitIdx)
        lang = extractedLang
        codeText = (rest + ' ' + codeText).trim()
      }
    }

    // If lang is plaintext or generic, but codeText starts with ui-card / ui-grid / ui-mockup
    if (lang === 'plaintext' || !lang) {
      const match = codeText.trim().match(/^(ui[-_](?:card|grid|mockup|feature)(?:[-_]json)?)\s+([\s\S]+)$/i)
      if (match) {
        lang = match[1].replace(/_/g, '-').toLowerCase()
        codeText = match[2].trim()
      }
    }

    return renderCodeCard(lang, codeText)
  },
  table: ({ children }) => (
    <div className="chat-table-container" style={{ overflowX: 'auto', margin: '14px 0', width: '100%', borderRadius: '8px', border: '1px solid var(--border-light, rgba(255, 255, 255, 0.12))', background: 'var(--panel-bg)', boxShadow: 'var(--shadow-sm, 0 4px 12px rgba(0, 0, 0, 0.15))' }}>
      <table className="chat-table" style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.86rem', textAlign: 'left', lineHeight: '1.5' }}>
        {children}
      </table>
    </div>
  ),
  thead: ({ children }) => (
    <thead>
      {Children.map(children, (child) =>
        isValidElement(child)
          ? cloneElement(child as ReactElement<{ style?: CSSProperties }>, {
              // color-mix with --text-main so the tint adapts per theme
              // (light overlay on dark themes, dark overlay on light theme).
              style: { background: 'color-mix(in srgb, var(--text-main, #e2e8f0) 5%, transparent)', borderBottom: '2px solid var(--border-light, rgba(255, 255, 255, 0.15))' },
            })
          : child
      )}
    </thead>
  ),
  tbody: ({ children }) => {
    // Only real <tr> elements count toward the zebra index — stray whitespace text nodes must not.
    const rowElements = Children.toArray(children).filter(isValidElement)
    let n = 0
    return (
      <tbody>
        {Children.toArray(children).map((child) => {
          if (!isValidElement(child)) return child
          const idx = n
          n += 1
          return cloneElement(child as ReactElement<{ style?: CSSProperties }>, {
            key: child.key ?? idx,
            style: {
              background: idx % 2 === 1 ? 'color-mix(in srgb, var(--text-main, #e2e8f0) 2%, transparent)' : 'transparent',
              borderBottom: idx < rowElements.length - 1 ? '1px solid var(--border, rgba(255, 255, 255, 0.08))' : 'none',
            },
          })
        })}
      </tbody>
    )
  },
  // `style` here comes from the cloneElement calls in `thead`/`tbody` above.
  tr: ({ children, style }: { children?: ReactNode; style?: CSSProperties }) => <tr style={style}>{children}</tr>,
  th: ({ children }) => (
    <th style={{ padding: '12px 16px', fontWeight: 700, color: 'var(--accent, #38bdf8)', fontFamily: 'ui-sans-serif, system-ui, sans-serif' }}>
      {highlightMentions(children)}
    </th>
  ),
  td: ({ children }) => (
    <td style={{ padding: '12px 16px', color: 'var(--text-chat, var(--text-main, #e2e8f0))' }}>
      {highlightMentions(children)}
    </td>
  ),

  blockquote: ({ children }) => {
    const rawText = flattenText(children).trim()
    const alertMatch = rawText.match(/^\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]/i)

    if (alertMatch) {
      return renderAlertBox(alertMatch[1].toUpperCase(), stripAlertTag(children))
    }

    return (
      <blockquote
        style={{
          margin: '12px 0',
          padding: '8px 16px',
          borderLeft: '3px solid var(--accent, #38bdf8)',
          background: 'rgba(255, 255, 255, 0.02)',
          borderRadius: '0 8px 8px 0',
          color: 'var(--text-muted, #94a3b8)',
          fontStyle: 'italic',
        }}
      >
        {children}
      </blockquote>
    )
  },
}

/** Normalizes non-standard list markers ("(1)", "[1]", "•") to plain GFM syntax so remark recognizes them as real lists. */
function normalizeListMarkers(text: string): string {
  return text
    .split('\n')
    .map((line) => {
      const bracketOrdered = line.match(/^(\s*)[\(\[](\d+)[\)\]](\s+.*)$/)
      if (bracketOrdered) return `${bracketOrdered[1]}${bracketOrdered[2]}.${bracketOrdered[3]}`
      const dotBullet = line.match(/^(\s*)[•](\s+.*)$/)
      if (dotBullet) return `${dotBullet[1]}-${dotBullet[2]}`
      return line
    })
    .join('\n')
}

type Segment = { type: 'text'; value: string } | { type: 'video'; src: string }

/** Splits out raw <video src="..."> lines (emitted by the backend for generated videos) since they aren't real markdown. */
function splitVideoSegments(text: string): Segment[] {
  const segments: Segment[] = []
  let buffer: string[] = []
  const flush = () => {
    if (buffer.length > 0) {
      segments.push({ type: 'text', value: buffer.join('\n') })
      buffer = []
    }
  }
  for (const line of text.split('\n')) {
    const match = line.match(/<video[^>]*src="([^"]+)"[^>]*>/)
    if (match) {
      flush()
      segments.push({ type: 'video', src: match[1] })
    } else {
      buffer.push(line)
    }
  }
  flush()
  return segments
}

export function renderFormattedMessage(text: string): ReactNode {
  const displayText = readableAssistantText(text)
  if (!displayText) return null

  const segments = splitVideoSegments(displayText)

  return (
    <div className="chat-formatted-body">
      {segments.map((segment, i) => {
        if (segment.type === 'video') {
          return (
            <div key={`video-${i}`} className="chat-media-card" style={{ margin: '10px 0', borderRadius: '12px', overflow: 'hidden', border: '1px solid var(--border, rgba(255,255,255,0.12))', background: '#000' }}>
              <video controls src={resolveMediaUrl(segment.src)} style={{ width: '100%', maxHeight: '400px', borderRadius: '8px', display: 'block' }} />
            </div>
          )
        }
        return (
          <ReactMarkdown key={`md-${i}`} remarkPlugins={[remarkGfm]} components={mdComponents as Components}>
            {normalizeListMarkers(segment.value)}
          </ReactMarkdown>
        )
      })}
    </div>
  )
}
