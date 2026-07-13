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
    .replace(/\$\\rightarrow\$|\\\(\\rightarrow\\\)/g, '→')
    .replace(/\$\\leftarrow\$|\\\(\\leftarrow\\\)/g, '←')
    .replace(/\$\\Rightarrow\$|\\\(\\Rightarrow\\\)/g, '⇒')
    .replace(/\$\\Leftarrow\$|\\\(\\Leftarrow\\\)/g, '⇐')
    .replace(/\$\\leftrightarrow\$|\\\(\\leftrightarrow\\\)/g, '↔')
    // comparison
    .replace(/\$\\leq\$|\\\(\\leq\\\)/g, '≤')
    .replace(/\$\\geq\$|\\\(\\geq\\\)/g, '≥')
    .replace(/\$\\neq\$|\\\(\\neq\\\)/g, '≠')
    .replace(/\$\\approx\$|\\\(\\approx\\\)/g, '≈')
    // math
    .replace(/\$\\times\$|\\\(\\times\\\)/g, '×')
    .replace(/\$\\div\$|\\\(\\div\\\)/g, '÷')
    .replace(/\$\\pm\$|\\\(\\pm\\\)/g, '±')
    .replace(/\$\\infty\$|\\\(\\infty\\\)/g, '∞')
    // other common symbols
    .replace(/\$\\cdot\$|\\\(\\cdot\\\)/g, '·')
    .replace(/\$\\in\$|\\\(\\in\\\)/g, '∈')
    .replace(/\$\\subset\$|\\\(\\subset\\\)/g, '⊂')
    .replace(/\$\\cup\$|\\\(\\cup\\\)/g, '∪')
    .replace(/\$\\cap\$|\\\(\\cap\\\)/g, '∩')
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

export function renderFormattedMessage(text: string): ReactNode {
  const displayText = readableAssistantText(text)
  if (!displayText) return null

  const formatInline = (str: string) => {
    const codeParts = str.split(/`([\s\S]*?)`/g)
    return codeParts.map((codePart, codeIndex) => {
      if (codeIndex % 2 === 1) {
        return (
          <code key={`code-${codeIndex}`} className="chat-inline-code">
            {codePart}
          </code>
        )
      }
      const boldParts = codePart.split(/\*\*([\s\S]*?)\*\*/g)
      return boldParts.map((boldPart, boldIndex) => {
        if (boldIndex % 2 === 1) {
          return (
            <strong key={`bold-${boldIndex}`} className="chat-bold-highlight">
              {boldPart}
            </strong>
          )
        }
        const mentionParts = boldPart.split(/(@[\w\-\.\/]+)/g)
        return mentionParts.map((mentionPart, mentionIndex) => {
          if (mentionIndex % 2 === 1) {
            return (
              <span key={`mention-${mentionIndex}`} className="chat-mention">
                {mentionPart}
              </span>
            )
          }
          return mentionPart
        })
      })
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
  const items: ReactNode[] = []

  let inCodeBlock = false
  let codeBlockLang = ''
  let codeBlockLines: string[] = []

  let inTable = false
  let tableLines: string[] = []

  const flushTable = (index: number) => {
    if (tableLines.length > 0) {
      items.push(renderHtmlTable(tableLines, `table-${index}`))
      tableLines = []
      inTable = false
    }
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]

    if (line.trim().startsWith('```')) {
      if (inTable) {
        flushTable(i)
      }
      if (inCodeBlock) {
        const codeText = codeBlockLines.join('\n')
        items.push(
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
        codeBlockLang = line.trim().slice(3).trim() || 'plaintext'
      }
      continue
    }

    if (inCodeBlock) {
      codeBlockLines.push(line)
      continue
    }

    if (isTableLine(line)) {
      inTable = true
      tableLines.push(line)
    } else {
      if (inTable) {
        flushTable(i)
      }

      const headerMatch = line.match(/^(#{1,6})\s+(.*)$/)
      if (headerMatch) {
        const level = headerMatch[1].length
        const content = headerMatch[2]

        const style = {
          fontWeight: 'bold',
          display: 'block',
          marginTop: level === 1 ? '16px' : level === 2 ? '14px' : '10px',
          marginBottom: '6px',
          fontSize: level === 1 ? '1.25em' : level === 2 ? '1.15em' : '1.05em',
          color: 'var(--text-main)',
        }

        items.push(
          <span key={`line-${i}`} style={style}>
            {formatInline(content)}
          </span>
        )
      } else {
        const listMatch = line.match(/^(\s*)([-*+])\s+(.*)$/)
        if (listMatch) {
          const indent = listMatch[1]
          const content = listMatch[3]
          items.push(
            <Fragment key={`line-${i}`}>
              {indent}• {formatInline(content)}
              {i < lines.length - 1 && '\n'}
            </Fragment>
          )
        } else {
          items.push(
            <Fragment key={`line-${i}`}>
              {formatInline(line)}
              {i < lines.length - 1 && '\n'}
            </Fragment>
          )
        }
      }
    }
  }

  if (inTable) {
    flushTable(lines.length)
  }

  if (inCodeBlock && codeBlockLines.length > 0) {
    const codeText = codeBlockLines.join('\n')
    items.push(
      <ChatCodeBlock
        key={`code-block-end`}
        code={codeText}
        language={codeBlockLang}
      />
    )
  }

  return <>{items}</>
}
