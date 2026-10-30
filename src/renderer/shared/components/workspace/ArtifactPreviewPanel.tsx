import React, { lazy, Suspense, useState, useEffect, useCallback, useMemo, useRef } from 'react'
import {
  Image as ImageIcon,
  FileCode,
  FileText,
  Globe,
  Music,
  Video,
  File,
  Eye,
} from 'lucide-react'
import { openArtifactInBrowser } from '../../utils/artifactBrowser'
import { runtimePlatform } from '../../platform'
import { renderFormattedMessage } from '../../utils/markdown'
import { ChatCodeBlock } from '../chat/ChatCodeBlock'
import WorkspaceFileInfo from './WorkspaceFileInfo'
import WorkspaceMediaPreview from './WorkspaceMediaPreview'

const WorkspacePdfPreview = lazy(() => import('./WorkspacePdfPreview'))

import {
  type ArtifactFile,
  type ArtifactType,
  type ViewportMode,
  detectArtifactType,
  getFileLanguage,
} from '../../utils/artifacts'

export type { ArtifactFile, ArtifactType, ViewportMode }
export { detectArtifactType, getFileLanguage }

function renderArtifactTypeIcon(type: ArtifactType) {
  switch (type) {
    case 'image':
      return <ImageIcon size={13} strokeWidth={2.2} aria-hidden="true" />
    case 'html':
      return <Globe size={13} strokeWidth={2.2} aria-hidden="true" />
    case 'svg':
    case 'code':
      return <FileCode size={13} strokeWidth={2.2} aria-hidden="true" />
    case 'markdown':
    case 'pdf':
      return <FileText size={13} strokeWidth={2.2} aria-hidden="true" />
    case 'audio':
      return <Music size={13} strokeWidth={2.2} aria-hidden="true" />
    case 'video':
      return <Video size={13} strokeWidth={2.2} aria-hidden="true" />
    default:
      return <File size={13} strokeWidth={2.2} aria-hidden="true" />
  }
}

interface Props {
  artifact: ArtifactFile | null
  onClose: () => void
  workspacePath?: string
}

export function ArtifactPreviewPanel({ artifact, onClose, workspacePath }: Props) {
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

  const isDualMode = artifactType === 'html' || artifactType === 'svg' || artifactType === 'markdown'

  const [activeTab, setActiveTab] = useState<'preview' | 'code'>(artifactType === 'code' ? 'code' : 'preview')
  const [viewport, setViewport] = useState<ViewportMode>('desktop')
  const [content, setContent] = useState<string>(artifact?.content || '')
  const [imageUrl, setImageUrl] = useState(artifact?.url || '')
  const [loading, setLoading] = useState<boolean>(false)
  const [error, setError] = useState<string | null>(null)
  const [openingBrowser, setOpeningBrowser] = useState(false)
  const [browserError, setBrowserError] = useState<string | null>(null)
  const browserInFlight = useRef(false)
  const currentArtifact = useRef(artifact)
  currentArtifact.current = artifact
  const [copied, setCopied] = useState<boolean>(false)
  const [imageDimensions, setImageDimensions] = useState<{ width: number; height: number } | null>(null)

  const [reloadRevision, setReloadRevision] = useState(0)
  const requestRevision = useRef(0)
  const sourceWorkspace = artifact?.workspacePath ?? workspacePath

  const language = useMemo(() => getFileLanguage(filePath), [filePath])
  const mediaPreview = artifactType === 'pdf' || artifactType === 'audio' || artifactType === 'video'

  const imageFormatLabel = useMemo(() => {
    if (artifactType !== 'image') return 'Image'
    const ext = filePath.split('.').pop()?.toUpperCase() || ''
    if (ext === 'JPG') return 'JPEG'
    if (['PNG', 'JPEG', 'WEBP', 'GIF', 'SVG', 'ICO', 'AVIF', 'BMP', 'TIFF'].includes(ext)) {
      return ext
    }
    return 'IMAGE'
  }, [artifactType, filePath])

  const codeLabel = useMemo(() => {
    if (!language || language === 'plaintext') return 'Code'
    const customNames: Record<string, string> = {
      javascript: 'JavaScript',
      typescript: 'TypeScript',
      python: 'Python',
      rust: 'Rust',
      json: 'JSON',
      html: 'HTML',
      css: 'CSS',
      cpp: 'C++',
      csharp: 'C#',
      php: 'PHP',
      ruby: 'Ruby',
      shell: 'Shell',
      bash: 'Bash',
      sql: 'SQL',
      yaml: 'YAML',
      xml: 'XML',
      toml: 'TOML',
      markdown: 'Markdown',
    }
    return customNames[language.toLowerCase()] || (language.charAt(0).toUpperCase() + language.slice(1))
  }, [language])

  const fetchContent = useCallback(async (refreshImage = false) => {
    const revision = ++requestRevision.current
    setError(null)
    setImageDimensions(null)
    setReloadRevision((value) => value + 1)
    if (!filePath) return
    if (refreshImage && activeTab === 'preview' && (artifactType === 'image' || artifactType === 'svg') && artifact?.url?.startsWith('data:image/') && sourceWorkspace) {
      setLoading(true)
      try {
        const url = await runtimePlatform.readWorkspaceImage(filePath, sourceWorkspace)
        if (requestRevision.current === revision) setImageUrl(url)
      } catch (reason) {
        if (requestRevision.current === revision) setError(reason instanceof Error ? reason.message : String(reason))
      } finally { if (requestRevision.current === revision) setLoading(false) }
      return
    }
    // Served previews load directly. Text is only needed when their Code tab is selected.
    if (mediaPreview || artifactType === 'file' || artifactType === 'image' || (artifact?.url && activeTab === 'preview')) {
      setLoading(false)
      if (mediaPreview && !artifact?.url) setError('No media preview source is available')
      if (artifactType === 'image' && !artifact?.url) setError('No image preview URL is available')
      return
    }
    setLoading(true)
    try {
      const text = await runtimePlatform.readWorkspaceFile(filePath, sourceWorkspace)
      if (requestRevision.current === revision) setContent(text)
    } catch (err: unknown) {
      if (requestRevision.current !== revision) return
      if (artifact?.content !== undefined) setContent(artifact.content)
      else setError(err instanceof Error ? err.message : 'Unable to read file content')
    } finally {
      if (requestRevision.current === revision) setLoading(false)
    }
  }, [filePath, sourceWorkspace, artifact, artifactType, activeTab, mediaPreview])

  useEffect(() => {
    setActiveTab(artifactType === 'code' ? 'code' : 'preview')
    setBrowserError(null)
    setContent(artifact?.content ?? '')
    setImageUrl(artifact?.url ?? '')
    setImageDimensions(null)
    setError(null)
  }, [artifact, artifactType])

  useEffect(() => {
    if (artifact?.content !== undefined && (activeTab === 'preview' || !filePath)) {
      setLoading(false)
      setError(null)
    } else {
      void fetchContent()
    }
    return () => { requestRevision.current += 1 }
  }, [artifact, activeTab, filePath, fetchContent])

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

  const handleOpenInMintBrowser = async () => {
    if (!artifact || browserInFlight.current) return
    browserInFlight.current = true
    setOpeningBrowser(true)
    setBrowserError(null)
    try {
      await openArtifactInBrowser(artifact, sourceWorkspace)
    } catch (reason: unknown) {
      if (currentArtifact.current === artifact) {
        setBrowserError(reason instanceof Error ? reason.message : String(reason))
      }
    } finally {
      browserInFlight.current = false
      setOpeningBrowser(false)
    }
  }

  if (!artifact) return null

  return (
    <aside
      className="artifact-preview-panel"
      style={{
        display: 'flex',
        flexDirection: 'column',
        width: 'var(--preview-width, 50%)',
        minWidth: '280px',
        maxWidth: 'none',
        height: '100%',
        background: 'var(--panel-bg, #1e1e20)',
        color: 'var(--text-main, #e8e8ea)',
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
          borderBottom: '1px solid var(--border)',
          background: 'var(--surface-bg, #252527)',
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
              borderRadius: 'var(--radius-xs, 4px)',
              background: 'color-mix(in srgb, var(--accent) 15%, transparent)',
              color: 'var(--interactive-fg)',
              flexShrink: 0,
            }}
            title={artifactType.toUpperCase()}
            aria-label={`${artifactType} artifact`}
          >
            {renderArtifactTypeIcon(artifactType)}
          </span>
          <div style={{ minWidth: 0, display: 'flex', flexDirection: 'column' }}>
            <span
              style={{
                fontWeight: 600,
                fontSize: '0.86rem',
                color: 'var(--text-main)',
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
                color: copied ? 'var(--accent)' : 'var(--text-muted)',
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
          {/* Dual-mode (Preview vs Code): Only for HTML, SVG, and Markdown */}
          {isDualMode ? (
            <div
              style={{
                display: 'inline-flex',
                padding: '2px',
                background: 'color-mix(in srgb, var(--text-main) 6%, transparent)',
                borderRadius: 'var(--radius-sm, 6px)',
                border: '1px solid var(--border)',
              }}
            >
              <button
                type="button"
                onClick={() => setActiveTab('preview')}
                style={{
                  background: activeTab === 'preview' ? 'var(--accent)' : 'transparent',
                  color: activeTab === 'preview' ? 'var(--text-on-accent)' : 'var(--text-muted)',
                  border: 'none',
                  borderRadius: 'var(--radius-xs, 4px)',
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
                  background: activeTab === 'code' ? 'var(--accent)' : 'transparent',
                  color: activeTab === 'code' ? 'var(--text-on-accent)' : 'var(--text-muted)',
                  border: 'none',
                  borderRadius: 'var(--radius-xs, 4px)',
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
          ) : (
            <div style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
              <div
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '5px',
                  padding: '3px 10px',
                  background:
                    artifactType === 'image' || artifactType === 'code'
                      ? 'color-mix(in srgb, var(--accent) 10%, transparent)'
                      : 'color-mix(in srgb, var(--text-main) 6%, transparent)',
                  borderRadius: '9999px',
                  border:
                    artifactType === 'image' || artifactType === 'code'
                      ? '1px solid color-mix(in srgb, var(--accent) 25%, transparent)'
                      : '1px solid var(--border)',
                  color: artifactType === 'image' || artifactType === 'code' ? 'var(--accent)' : 'var(--text-muted)',
                  fontSize: '0.72rem',
                  fontWeight: 500,
                  letterSpacing: '0.01em',
                }}
              >
                {artifactType === 'code' ? (
                  <FileCode size={12} strokeWidth={2.2} aria-hidden="true" />
                ) : artifactType === 'image' ? (
                  <ImageIcon size={12} strokeWidth={2.2} aria-hidden="true" />
                ) : (
                  <Eye size={12} strokeWidth={2.2} aria-hidden="true" />
                )}
                <span>
                  {artifactType === 'code'
                    ? codeLabel
                    : artifactType === 'image'
                      ? imageFormatLabel
                      : artifactType === 'file'
                        ? 'Details'
                        : 'Preview'}
                </span>
              </div>
              {artifactType === 'image' && imageDimensions && (
                <div
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    padding: '3px 9px',
                    borderRadius: '9999px',
                    background: 'color-mix(in srgb, var(--text-main) 6%, transparent)',
                    border: '1px solid var(--border)',
                    fontSize: '0.7rem',
                    color: 'var(--text-muted)',
                    fontFamily: 'var(--font-mono, monospace)',
                    letterSpacing: '0.01em',
                    whiteSpace: 'nowrap',
                  }}
                  title="Image dimensions (width × height)"
                >
                  {imageDimensions.width} × {imageDimensions.height} px
                </div>
              )}
            </div>
          )}

          {/* Viewport switcher for HTML */}
          {activeTab === 'preview' && artifactType === 'html' && (
            <div
              style={{
                display: 'inline-flex',
                padding: '2px',
                background: 'color-mix(in srgb, var(--text-main) 6%, transparent)',
                borderRadius: 'var(--radius-sm, 6px)',
                border: '1px solid var(--border)',
              }}
            >
              <button
                type="button"
                onClick={() => setViewport('desktop')}
                title="Desktop 100%"
                style={{
                  background: viewport === 'desktop' ? 'color-mix(in srgb, var(--text-main) 12%, transparent)' : 'transparent',
                  color: viewport === 'desktop' ? 'var(--text-main)' : 'var(--text-muted)',
                  border: 'none',
                  borderRadius: 'var(--radius-xs, 4px)',
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
                  background: viewport === 'tablet' ? 'color-mix(in srgb, var(--text-main) 12%, transparent)' : 'transparent',
                  color: viewport === 'tablet' ? 'var(--text-main)' : 'var(--text-muted)',
                  border: 'none',
                  borderRadius: 'var(--radius-xs, 4px)',
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
                  background: viewport === 'mobile' ? 'color-mix(in srgb, var(--text-main) 12%, transparent)' : 'transparent',
                  color: viewport === 'mobile' ? 'var(--text-main)' : 'var(--text-muted)',
                  border: 'none',
                  borderRadius: 'var(--radius-xs, 4px)',
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
          {!mediaPreview && <button
            type="button"
            onClick={() => { void fetchContent(true) }}
            disabled={loading}
            title="Reload content from file"
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--text-muted)',
              padding: '5px',
              borderRadius: 'var(--radius-xs, 4px)',
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
          </button>}

          {artifactType === 'html' && <button
            type="button"
            onClick={() => { void handleOpenInMintBrowser() }}
            disabled={openingBrowser}
            aria-label="Open preview"
            aria-busy={openingBrowser}
            title={openingBrowser ? 'Opening preview…' : 'Open preview'}
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--text-muted)',
              padding: '5px',
              borderRadius: 'var(--radius-xs, 4px)',
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
          </button>}

          <button
            type="button"
            onClick={onClose}
            title="Close preview Panel"
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--text-muted)',
              padding: '5px',
              borderRadius: 'var(--radius-xs, 4px)',
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

      {browserError && (
        <div role="alert" style={{ padding: '8px 14px', fontSize: '0.8rem', color: 'var(--status-error, #f87171)', flexShrink: 0 }}>
          Could not open preview: {browserError}
        </div>
      )}

      {/* Main Content Area */}
      <div
        style={{
          flex: 1,
          overflow: 'auto',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'flex-start',
          background: activeTab === 'preview' && artifactType === 'html' ? 'var(--input-bg, #0a0a0b)' : 'transparent',
          padding: activeTab === 'preview' && artifactType === 'html' ? '16px' : '0',
        }}
      >
        {error ? (
          <div style={{ padding: '24px', textAlign: 'center', color: 'var(--status-error, #f87171)' }}>
            <p style={{ fontWeight: 600, marginBottom: '8px' }}>Error loading preview</p>
            <p style={{ fontSize: '0.8rem', opacity: 0.8 }}>{error}</p>
            <button
              type="button"
              onClick={() => { void fetchContent(true) }}
              style={{
                marginTop: '12px',
                padding: '4px 12px',
                borderRadius: 'var(--radius-xs, 4px)',
                background: 'var(--hover-delete-bg, rgba(239, 68, 68, 0.15))',
                color: 'var(--status-error, #f87171)',
                border: '1px solid color-mix(in srgb, var(--status-error, #ef4444) 30%, transparent)',
                cursor: 'pointer',
                fontSize: '0.78rem',
              }}
            >
              Retry
            </button>
          </div>
        ) : artifactType === 'file' ? (
          <WorkspaceFileInfo key={`${sourceWorkspace}:${filePath}`} path={filePath} root={sourceWorkspace} />
        ) : artifactType === 'pdf' && artifact.url ? (
          <Suspense fallback={<p role="status">Loading PDF viewer…</p>}><WorkspacePdfPreview key={artifact.url} url={artifact.url} path={filePath} root={sourceWorkspace} byteLength={artifact.byteLength} range={artifact.range} /></Suspense>
        ) : (artifactType === 'audio' || artifactType === 'video') && artifact.url ? (
          <WorkspaceMediaPreview key={artifact.url} kind={artifactType} url={artifact.url} path={filePath} root={sourceWorkspace} mimeType={artifact.mimeType} />
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
              borderRadius: viewport === 'desktop' ? '0px' : 'var(--radius-md, 8px)',
              overflow: 'hidden',
              boxShadow: viewport === 'desktop' ? 'none' : 'var(--shadow-md, 0 10px 25px rgba(0,0,0,0.5))',
              border: viewport === 'desktop' ? 'none' : '1px solid var(--border)',
              transition: 'width 0.25s cubic-bezier(0.4, 0, 0.2, 1)',
            }}
          >
            {/* Served documents use their own loopback origin so relative assets can load. */}
            <iframe
              title={fileName}
              key={reloadRevision}
              src={artifact.url}
              srcDoc={artifact.url ? undefined : content}
              sandbox={artifact.url ? 'allow-scripts allow-forms allow-modals allow-same-origin' : 'allow-scripts allow-forms allow-modals'}
              style={{
                width: '100%',
                height: '100%',
                border: 'none',
                background: '#ffffff',
              }}
            />
          </div>
        ) : artifactType === 'image' || (artifactType === 'svg' && artifact.url) ? (
          <div style={{ width: '100%', height: '100%', padding: '24px', display: 'flex', alignItems: 'center', justifyContent: 'center', overflow: 'hidden' }}>
            <img
              key={reloadRevision}
              ref={(el) => {
                if (el && el.complete && el.naturalWidth && el.naturalHeight) {
                  setImageDimensions((prev) => {
                    if (prev?.width === el.naturalWidth && prev?.height === el.naturalHeight) return prev
                    return { width: el.naturalWidth, height: el.naturalHeight }
                  })
                }
              }}
              src={imageUrl || artifact.url}
              alt={fileName}
              style={{ width: '100%', height: '100%', objectFit: 'contain' }}
              onLoad={(e) => {
                const img = e.currentTarget
                if (img.naturalWidth && img.naturalHeight) {
                  setImageDimensions({ width: img.naturalWidth, height: img.naturalHeight })
                }
              }}
              onError={() => setError('Unable to load image. Reload the preview or check that the file is still available.')}
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
                'linear-gradient(45deg, color-mix(in srgb, var(--text-main) 4%, transparent) 25%, transparent 25%), linear-gradient(-45deg, color-mix(in srgb, var(--text-main) 4%, transparent) 25%, transparent 25%), linear-gradient(45deg, transparent 75%, color-mix(in srgb, var(--text-main) 4%, transparent) 75%), linear-gradient(-45deg, transparent 75%, color-mix(in srgb, var(--text-main) 4%, transparent) 75%)',
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
              color: 'var(--text-chat, var(--text-main))',
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
