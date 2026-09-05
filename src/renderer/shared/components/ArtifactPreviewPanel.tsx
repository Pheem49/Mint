import React, { useState, useEffect, useCallback, useMemo } from 'react'
import { readWorkspaceFile } from '@/tauri'
import { renderFormattedMessage } from '../utils/markdown'
import { ChatCodeBlock } from './ChatCodeBlock'

export type ArtifactType = 'html' | 'svg' | 'markdown' | 'code'
export type ViewportMode = 'desktop' | 'tablet' | 'mobile'

export interface ArtifactFile {
  path: string
  content?: string
  type?: ArtifactType
}

interface Props {
  artifact: ArtifactFile | null
  onClose: () => void
  workspacePath?: string
}

export function detectArtifactType(filePath: string): ArtifactType {
  const lower = filePath.toLowerCase()
  if (lower.endsWith('.html') || lower.endsWith('.htm')) return 'html'
  if (lower.endsWith('.svg')) return 'svg'
  if (lower.endsWith('.md') || lower.endsWith('.markdown')) return 'markdown'
  return 'code'
}

export function getFileLanguage(filePath: string): string {
  const ext = filePath.split('.').pop()?.toLowerCase() || ''
  const map: Record<string, string> = {
    js: 'javascript',
    jsx: 'javascript',
    ts: 'typescript',
    tsx: 'typescript',
    html: 'html',
    htm: 'html',
    css: 'css',
    json: 'json',
    rs: 'rust',
    py: 'python',
    svg: 'xml',
    md: 'markdown',
  }
  return map[ext] || ext || 'plaintext'
}

export function ArtifactPreviewPanel({ artifact, onClose }: Props) {
  const [activeTab, setActiveTab] = useState<'preview' | 'code'>('preview')
  const [viewport, setViewport] = useState<ViewportMode>('desktop')
  const [content, setContent] = useState<string>(artifact?.content || '')
  const [loading, setLoading] = useState<boolean>(false)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState<boolean>(false)

  const filePath = artifact?.path || ''
  const fileName = useMemo(() => {
    if (!filePath) return ''
    const parts = filePath.replace(/\\/g, '/').split('/')
    return parts[parts.length - 1] || filePath
  }, [filePath])

  const artifactType: ArtifactType = useMemo(() => {
    if (artifact?.type) return artifact.type
    return detectArtifactType(filePath)
  }, [artifact, filePath])

  const language = useMemo(() => getFileLanguage(filePath), [filePath])

  const fetchContent = useCallback(async () => {
    if (!filePath) return
    setLoading(true)
    setError(null)
    try {
      const text = await readWorkspaceFile(filePath)
      setContent(text)
    } catch (err: any) {
      // If content was already supplied in artifact, fallback to it
      if (artifact?.content) {
        setContent(artifact.content)
      } else {
        setError(err?.message || 'Unable to read file content')
      }
    } finally {
      setLoading(false)
    }
  }, [filePath, artifact])

  useEffect(() => {
    if (artifact?.content) {
      setContent(artifact.content)
    } else {
      fetchContent()
    }
  }, [artifact, fetchContent])

  const handleCopyPath = async () => {
    if (!filePath) return
    try {
      await navigator.clipboard.writeText(filePath)
      setCopied(true)
      setTimeout(() => setCopied(false), 1800)
    } catch {
      // ignore
    }
  }

  const handleOpenExternal = () => {
    if (!content) return
    const mimeType =
      artifactType === 'html'
        ? 'text/html'
        : artifactType === 'svg'
          ? 'image/svg+xml'
          : artifactType === 'markdown'
            ? 'text/markdown'
            : 'text/plain'
    const blob = new Blob([content], { type: `${mimeType};charset=utf-8` })
    const url = URL.createObjectURL(blob)
    window.open(url, '_blank')
  }

  if (!artifact) return null

  return (
    <aside
      className="artifact-preview-panel"
      style={{
        display: 'flex',
        flexDirection: 'column',
        width: '50%',
        minWidth: '380px',
        maxWidth: '800px',
        height: '100%',
        borderLeft: '1px solid var(--border-color, #232730)',
        background: 'var(--bg-primary, #111317)',
        color: 'var(--text-primary, #f3f4f6)',
        position: 'relative',
        zIndex: 10,
        overflow: 'hidden',
      }}
    >
      {/* Header */}
      <header
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '10px 14px',
          borderBottom: '1px solid var(--border-color, #232730)',
          background: 'var(--bg-secondary, #181a20)',
          gap: '8px',
          flexShrink: 0,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
          <span
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              width: '24px',
              height: '24px',
              borderRadius: '6px',
              background: 'rgba(16, 185, 129, 0.12)',
              color: '#10b981',
              fontSize: '0.68rem',
              fontWeight: 700,
              flexShrink: 0,
            }}
          >
            {artifactType.toUpperCase().slice(0, 3)}
          </span>
          <div style={{ minWidth: 0, display: 'flex', flexDirection: 'column' }}>
            <span
              style={{
                fontWeight: 600,
                fontSize: '0.86rem',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
              title={filePath}
            >
              {fileName}
            </span>
            <button
              type="button"
              onClick={handleCopyPath}
              style={{
                background: 'none',
                border: 'none',
                color: copied ? '#10b981' : 'var(--text-muted, #9ca3af)',
                fontSize: '0.7rem',
                padding: 0,
                cursor: 'pointer',
                textAlign: 'left',
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
              }}
            >
              {copied ? 'Path Copied' : 'Copy path'}
            </button>
          </div>
        </div>

        {/* Center: Tabs & Viewport */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          {/* Preview vs Code tab toggle */}
          <div
            style={{
              display: 'inline-flex',
              padding: '2px',
              background: 'rgba(255, 255, 255, 0.05)',
              borderRadius: '6px',
              border: '1px solid rgba(255, 255, 255, 0.08)',
            }}
          >
            <button
              type="button"
              onClick={() => setActiveTab('preview')}
              style={{
                background: activeTab === 'preview' ? 'var(--accent, #10b981)' : 'transparent',
                color: activeTab === 'preview' ? '#ffffff' : 'var(--text-muted, #9ca3af)',
                border: 'none',
                borderRadius: '4px',
                padding: '3px 9px',
                fontSize: '0.75rem',
                fontWeight: 500,
                cursor: 'pointer',
                transition: 'all 0.15s ease',
              }}
            >
              Preview
            </button>
            <button
              type="button"
              onClick={() => setActiveTab('code')}
              style={{
                background: activeTab === 'code' ? 'var(--accent, #10b981)' : 'transparent',
                color: activeTab === 'code' ? '#ffffff' : 'var(--text-muted, #9ca3af)',
                border: 'none',
                borderRadius: '4px',
                padding: '3px 9px',
                fontSize: '0.75rem',
                fontWeight: 500,
                cursor: 'pointer',
                transition: 'all 0.15s ease',
              }}
            >
              Code
            </button>
          </div>

          {/* Viewport switcher for HTML */}
          {activeTab === 'preview' && artifactType === 'html' && (
            <div
              style={{
                display: 'inline-flex',
                padding: '2px',
                background: 'rgba(255, 255, 255, 0.05)',
                borderRadius: '6px',
                border: '1px solid rgba(255, 255, 255, 0.08)',
              }}
            >
              <button
                type="button"
                onClick={() => setViewport('desktop')}
                title="Desktop 100%"
                style={{
                  background: viewport === 'desktop' ? 'rgba(255, 255, 255, 0.12)' : 'transparent',
                  color: viewport === 'desktop' ? '#fff' : '#9ca3af',
                  border: 'none',
                  borderRadius: '4px',
                  padding: '3px 7px',
                  cursor: 'pointer',
                  fontSize: '0.72rem',
                }}
              >
                Desktop
              </button>
              <button
                type="button"
                onClick={() => setViewport('tablet')}
                title="Tablet (768px)"
                style={{
                  background: viewport === 'tablet' ? 'rgba(255, 255, 255, 0.12)' : 'transparent',
                  color: viewport === 'tablet' ? '#fff' : '#9ca3af',
                  border: 'none',
                  borderRadius: '4px',
                  padding: '3px 7px',
                  cursor: 'pointer',
                  fontSize: '0.72rem',
                }}
              >
                Tablet
              </button>
              <button
                type="button"
                onClick={() => setViewport('mobile')}
                title="Mobile (375px)"
                style={{
                  background: viewport === 'mobile' ? 'rgba(255, 255, 255, 0.12)' : 'transparent',
                  color: viewport === 'mobile' ? '#fff' : '#9ca3af',
                  border: 'none',
                  borderRadius: '4px',
                  padding: '3px 7px',
                  cursor: 'pointer',
                  fontSize: '0.72rem',
                }}
              >
                Mobile
              </button>
            </div>
          )}
        </div>

        {/* Right actions */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          <button
            type="button"
            onClick={fetchContent}
            disabled={loading}
            title="Reload content from file"
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--text-muted, #9ca3af)',
              padding: '5px',
              borderRadius: '4px',
              cursor: 'pointer',
              display: 'inline-flex',
              alignItems: 'center',
            }}
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              style={{ animation: loading ? 'spin 1s linear infinite' : 'none' }}
            >
              <polyline points="23 4 23 10 17 10" />
              <polyline points="1 20 1 14 7 14" />
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
            </svg>
          </button>

          <button
            type="button"
            onClick={handleOpenExternal}
            title="Open preview in new window"
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--text-muted, #9ca3af)',
              padding: '5px',
              borderRadius: '4px',
              cursor: 'pointer',
              display: 'inline-flex',
              alignItems: 'center',
            }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
              <polyline points="15 3 21 3 21 9" />
              <line x1="10" y1="14" x2="21" y2="3" />
            </svg>
          </button>

          <button
            type="button"
            onClick={onClose}
            title="Close Preview Panel"
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--text-muted, #9ca3af)',
              padding: '5px',
              borderRadius: '4px',
              cursor: 'pointer',
              display: 'inline-flex',
              alignItems: 'center',
            }}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      </header>

      {/* Main Content Area */}
      <div
        style={{
          flex: 1,
          overflow: 'auto',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'flex-start',
          background: activeTab === 'preview' && artifactType === 'html' ? '#0b0c0e' : 'transparent',
          padding: activeTab === 'preview' && artifactType === 'html' ? '16px' : '0',
        }}
      >
        {error ? (
          <div style={{ padding: '24px', textAlign: 'center', color: '#f87171' }}>
            <p style={{ fontWeight: 600, marginBottom: '8px' }}>Error loading preview</p>
            <p style={{ fontSize: '0.8rem', opacity: 0.8 }}>{error}</p>
            <button
              type="button"
              onClick={fetchContent}
              style={{
                marginTop: '12px',
                padding: '4px 12px',
                borderRadius: '4px',
                background: 'rgba(239, 68, 68, 0.15)',
                color: '#f87171',
                border: '1px solid rgba(239, 68, 68, 0.3)',
                cursor: 'pointer',
                fontSize: '0.78rem',
              }}
            >
              Retry
            </button>
          </div>
        ) : activeTab === 'code' ? (
          <div style={{ width: '100%', height: '100%', overflow: 'auto', padding: '12px' }}>
            <ChatCodeBlock code={content} language={language} />
          </div>
        ) : artifactType === 'html' ? (
          <div
            style={{
              width: viewport === 'mobile' ? '375px' : viewport === 'tablet' ? '768px' : '100%',
              maxWidth: '100%',
              height: '100%',
              background: '#ffffff',
              borderRadius: viewport === 'desktop' ? '0px' : '8px',
              overflow: 'hidden',
              boxShadow: viewport === 'desktop' ? 'none' : '0 10px 25px rgba(0,0,0,0.5)',
              border: viewport === 'desktop' ? 'none' : '1px solid rgba(255,255,255,0.15)',
              transition: 'width 0.25s cubic-bezier(0.4, 0, 0.2, 1)',
            }}
          >
            <iframe
              title={fileName}
              srcDoc={content}
              sandbox="allow-scripts allow-forms allow-modals"
              style={{
                width: '100%',
                height: '100%',
                border: 'none',
                background: '#ffffff',
              }}
            />
          </div>
        ) : artifactType === 'svg' ? (
          <div
            style={{
              width: '100%',
              height: '100%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              padding: '24px',
              backgroundImage:
                'linear-gradient(45deg, rgba(255,255,255,0.03) 25%, transparent 25%), linear-gradient(-45deg, rgba(255,255,255,0.03) 25%, transparent 25%), linear-gradient(45deg, transparent 75%, rgba(255,255,255,0.03) 75%), linear-gradient(-45deg, transparent 75%, rgba(255,255,255,0.03) 75%)',
              backgroundSize: '20px 20px',
              backgroundPosition: '0 0, 0 10px, 10px -10px, -10px 0px',
              overflow: 'auto',
            }}
          >
            <div
              style={{ maxWidth: '90%', maxHeight: '90%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
              dangerouslySetInnerHTML={{ __html: content }}
            />
          </div>
        ) : artifactType === 'markdown' ? (
          <div
            style={{
              width: '100%',
              height: '100%',
              overflow: 'auto',
              padding: '20px 24px',
              lineHeight: 1.6,
            }}
          >
            {renderFormattedMessage(content)}
          </div>
        ) : (
          <div style={{ width: '100%', height: '100%', overflow: 'auto', padding: '12px' }}>
            <ChatCodeBlock code={content} language={language} />
          </div>
        )}
      </div>
    </aside>
  )
}
