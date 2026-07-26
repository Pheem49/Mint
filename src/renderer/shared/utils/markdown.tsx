/**
 * shared/utils/markdown.tsx
 * Custom markdown parsing and rendering engine for message bubbles.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import { Fragment, type ReactNode } from 'react'
import { ChatCodeBlock } from '../components/ChatCodeBlock'

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



export function renderFormattedMessage(text: string): ReactNode {
  const displayText = readableAssistantText(text)
  if (!displayText) return null

  const formatInline = (str: string): ReactNode => {
    if (!str) return null

    // Split code backticks first
    const codeParts = str.split(/`([\s\S]*?)`/g)
    return codeParts.map((codePart, codeIndex) => {
      if (codeIndex % 2 === 1) {
        return (
          <code key={`code-${codeIndex}`} className="chat-inline-code">
            {codePart}
          </code>
        )
      }

      // Match markdown links [label](url)
      const linkRegex = /\[([^\]]+)\]\((https?:\/\/[^\s\)]+)\)/g
      const linkMatches: Array<{ index: number; length: number; label: string; url: string }> = []
      let match: RegExpExecArray | null
      while ((match = linkRegex.exec(codePart)) !== null) {
        linkMatches.push({
          index: match.index,
          length: match[0].length,
          label: match[1],
          url: match[2],
        })
      }

      const renderTextAndFormatting = (subStr: string, keyPrefix: string): ReactNode => {
        // Process bold **text**
        const boldParts = subStr.split(/\*\*([\s\S]*?)\*\*/g)
        return boldParts.map((boldPart, boldIndex) => {
          if (boldIndex % 2 === 1) {
            return (
              <strong key={`${keyPrefix}-bold-${boldIndex}`} className="chat-bold-highlight">
                {boldPart}
              </strong>
            )
          }
          // Process mentions (@...)
          const mentionParts = boldPart.split(/(@[\w\-\.\/]+)/g)
          return mentionParts.map((mentionPart, mentionIndex) => {
            if (mentionIndex % 2 === 1) {
              return (
                <span key={`${keyPrefix}-mention-${mentionIndex}`} className="chat-mention">
                  {mentionPart}
                </span>
              )
            }
            // Process raw URLs (https://... or http://...)
            const urlRegex = /(https?:\/\/[^\s<>\(\)]+)/g
            const urlParts = mentionPart.split(urlRegex)
            return urlParts.map((urlPart, urlIndex) => {
              if (urlIndex % 2 === 1) {
                return (
                  <a
                    key={`${keyPrefix}-url-${urlIndex}`}
                    href={urlPart}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="chat-link"
                  >
                    {urlPart}
                  </a>
                )
              }
              return urlPart
            })
          })
        })
      }

      if (linkMatches.length === 0) {
        return renderTextAndFormatting(codePart, `c-${codeIndex}`)
      }

      const linkElements: ReactNode[] = []
      let lastIdx = 0
      linkMatches.forEach((lMatch, lIdx) => {
        if (lMatch.index > lastIdx) {
          linkElements.push(renderTextAndFormatting(codePart.slice(lastIdx, lMatch.index), `c-${codeIndex}-pre-${lIdx}`))
        }
        linkElements.push(
          <a
            key={`link-${codeIndex}-${lIdx}`}
            href={lMatch.url}
            target="_blank"
            rel="noopener noreferrer"
            className="chat-link"
          >
            {renderTextAndFormatting(lMatch.label, `c-${codeIndex}-label-${lIdx}`)}
          </a>
        )
        lastIdx = lMatch.index + lMatch.length
      })
      if (lastIdx < codePart.length) {
        linkElements.push(renderTextAndFormatting(codePart.slice(lastIdx), `c-${codeIndex}-post`))
      }

      return linkElements
    })
  }

  const renderHtmlTable = (tableLines: string[], key: string) => {
    const rows: string[][] = []
    for (const line of tableLines) {
      const trimmed = line.trim()
      if (!trimmed) continue
      const content = trimmed.slice(1, -1)
      const contentEscaped = content.replace(/\\\|/g, '\u0000')
      const cols = contentEscaped.split('|').map(s => s.replace(/\u0000/g, '|').trim())
      rows.push(cols)
    }

    if (rows.length === 0) return null

    let hasSeparator = false
    let separatorIdx = -1
    for (let i = 0; i < rows.length; i++) {
      const row = rows[i]
      if (row.length > 0 && row.every(col => /^[-\s:]+$/.test(col))) {
        hasSeparator = true
        separatorIdx = i
        break
      }
    }

    let headerRow: string[] | null = null
    const dataRows: string[][] = []

    if (hasSeparator) {
      if (separatorIdx > 0) {
        headerRow = rows[separatorIdx - 1]
        for (let i = 0; i < rows.length; i++) {
          if (i !== separatorIdx && i !== separatorIdx - 1) {
            dataRows.push(rows[i])
          }
        }
      } else {
        for (let i = 0; i < rows.length; i++) {
          if (i !== separatorIdx) {
            dataRows.push(rows[i])
          }
        }
      }
    } else {
      headerRow = rows[0]
      for (let i = 1; i < rows.length; i++) {
        dataRows.push(rows[i])
      }
    }

    return (
      <div key={key} className="chat-table-container" style={{
        overflowX: 'auto',
        margin: '14px 0',
        width: '100%',
        borderRadius: '8px',
        border: '1px solid rgba(255, 255, 255, 0.12)',
        background: 'rgba(30, 41, 59, 0.35)',
        boxShadow: '0 4px 12px rgba(0, 0, 0, 0.15)',
      }}>
        <table className="chat-table" style={{
          width: '100%',
          borderCollapse: 'collapse',
          fontSize: '0.86rem',
          textAlign: 'left',
          lineHeight: '1.5',
        }}>
          {headerRow && (
            <thead>
              <tr style={{
                background: 'rgba(255, 255, 255, 0.04)',
                borderBottom: '2px solid rgba(255, 255, 255, 0.15)',
              }}>
                {headerRow.map((col, idx) => (
                  <th key={`th-${idx}`} style={{
                    padding: '12px 16px',
                    fontWeight: 700,
                    color: 'var(--accent, #38bdf8)',
                    fontFamily: 'ui-sans-serif, system-ui, sans-serif',
                  }}>
                    {formatInline(col)}
                  </th>
                ))}
              </tr>
            </thead>
          )}
          <tbody>
            {dataRows.map((row, rIdx) => (
              <tr key={`tr-${rIdx}`} style={{
                background: rIdx % 2 === 1 ? 'rgba(255, 255, 255, 0.015)' : 'transparent',
                borderBottom: rIdx < dataRows.length - 1 ? '1px solid rgba(255, 255, 255, 0.08)' : 'none',
              }}>
                {row.map((col, cIdx) => (
                  <td key={`td-${cIdx}`} style={{
                    padding: '12px 16px',
                    color: 'var(--text-main, #e2e8f0)',
                  }}>
                    {formatInline(col)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    )
  }

  const lines = displayText.split('\n')
  const blocks: ReactNode[] = []

  let inCodeBlock = false
  let codeBlockLang = ''
  let codeBlockLines: string[] = []

  let inTable = false
  let tableLines: string[] = []

  const flushTable = (index: number) => {
    if (tableLines.length > 0) {
      blocks.push(renderHtmlTable(tableLines, `table-${index}`))
      tableLines = []
      inTable = false
    }
  }

  let currentParagraphLines: string[] = []

  const flushParagraph = (index: number) => {
    if (currentParagraphLines.length > 0) {
      const paragraphText = currentParagraphLines.join('\n')
      if (paragraphText.trim()) {
        blocks.push(
          <div key={`para-${index}`} className="chat-paragraph">
            {formatInline(paragraphText)}
          </div>
        )
      }
      currentParagraphLines = []
    }
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const trimmedLine = line.trim()

    // Code block toggle
    if (trimmedLine.startsWith('```')) {
      if (inTable) flushTable(i)
      flushParagraph(i)

      if (inCodeBlock) {
        const codeText = codeBlockLines.join('\n')
        blocks.push(
          <ChatCodeBlock
            key={`code-block-${i}`}
            code={codeText}
            language={codeBlockLang}
          />
        )
        inCodeBlock = false
        codeBlockLines = []
      } else {
        inCodeBlock = true
        codeBlockLang = trimmedLine.slice(3).trim() || 'plaintext'
      }
      continue
    }

    if (inCodeBlock) {
      codeBlockLines.push(line)
      continue
    }

    // Markdown Table
    if (isTableLine(line)) {
      flushParagraph(i)
      inTable = true
      tableLines.push(line)
      continue
    } else if (inTable) {
      flushTable(i)
    }

    // Empty line -> flush paragraph (creates paragraph gap)
    if (!trimmedLine) {
      flushParagraph(i)
      continue
    }

    // Markdown Headers (# Header)
    const headerMatch = line.match(/^(#{1,6})\s+(.*)$/)
    if (headerMatch) {
      flushParagraph(i)
      const level = Math.min(headerMatch[1].length, 6)
      const content = headerMatch[2]
      blocks.push(
        <div key={`header-${i}`} className={`chat-heading chat-heading-${level}`}>
          {formatInline(content)}
        </div>
      )
      continue
    }

    // Numbered list items: "1. ", "2) ", "(1) ", etc.
    const orderedMatch = line.match(/^(\s*)(?:(\d+)[\.\)]|[\(\[](\d+)[\)\]])\s+(.*)$/)
    if (orderedMatch) {
      flushParagraph(i)
      const num = orderedMatch[2] || orderedMatch[3]
      const content = orderedMatch[4]
      blocks.push(
        <div key={`ordered-${i}`} className="chat-list-item chat-ordered-item">
          <span className="chat-list-number">{num}.</span>
          <span className="chat-list-text">{formatInline(content)}</span>
        </div>
      )
      continue
    }

    // Bullet list items: "- ", "* ", "+ ", "• "
    const bulletMatch = line.match(/^(\s*)([-*+•])\s+(.*)$/)
    if (bulletMatch) {
      flushParagraph(i)
      const content = bulletMatch[3]
      blocks.push(
        <div key={`bullet-${i}`} className="chat-list-item chat-bullet-item">
          <span className="chat-list-bullet">•</span>
          <span className="chat-list-text">{formatInline(content)}</span>
        </div>
      )
      continue
    }

    // Emoji header / Section title (e.g. "🚀 ขั้นตอนเริ่มต้นสำหรับคุณภีม:", "⚠️ ข้อควรระวังจากมัน (สำคัญมาก!):")
    const isEmojiHeader = /^(?:[\u{1F300}-\u{1F9FF}]|[\u{2600}-\u{26FF}]|[\u{2700}-\u{27BF}]).*:$/u.test(trimmedLine)
    if (isEmojiHeader) {
      flushParagraph(i)
      blocks.push(
        <div key={`section-${i}`} className="chat-heading chat-heading-3 chat-section-title">
          {formatInline(line)}
        </div>
      )
      continue
    }

    // Regular text line -> append to current paragraph
    currentParagraphLines.push(line)
  }

  if (inTable) flushTable(lines.length)
  flushParagraph(lines.length)

  if (inCodeBlock && codeBlockLines.length > 0) {
    const codeText = codeBlockLines.join('\n')
    blocks.push(
      <ChatCodeBlock
        key={`code-block-end`}
        code={codeText}
        language={codeBlockLang}
      />
    )
  }

  return <div className="chat-formatted-body">{blocks}</div>
}

