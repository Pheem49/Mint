/**
 * shared/components/ChatCodeBlock.tsx
 * Syntax-highlighted code block with copy + download actions.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import { useState } from 'react'

interface Props {
  code: string
  language: string
  key?: any
}


export function ChatCodeBlock({ code, language }: Props) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy code: ', err)
    }
  }

  const handleDownload = () => {
    try {
      const blob = new Blob([code], { type: 'text/plain;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `code.${language === 'plaintext' ? 'txt' : language}`
      link.click()
      URL.revokeObjectURL(url)
    } catch (err) {
      console.error('Failed to download code: ', err)
    }
  }

  const displayLang = language.charAt(0).toUpperCase() + language.slice(1).toLowerCase()

  return (
    <div className="chat-code-block-container" style={{ whiteSpace: 'normal' }}>
      <div className="chat-code-block-header">
        <span className="chat-code-block-lang">{displayLang}</span>
        <div className="chat-code-block-actions">
          <button type="button" onClick={handleDownload} title="Download code" className="chat-code-action-btn">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
          </button>
          <button type="button" onClick={handleCopy} title="Copy code" className="chat-code-action-btn">
            {copied ? (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            ) : (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
            )}
          </button>
        </div>
      </div>
      <pre className="chat-code-block-body">
        <code>{code}</code>
      </pre>
    </div>
  )
}
