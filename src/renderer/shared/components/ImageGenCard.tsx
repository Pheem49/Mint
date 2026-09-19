import React, { useState, useEffect } from 'react'
import { Sparkles } from 'lucide-react'

export interface ImageGenHit {
  url: string
}

export interface ImageGenData {
  prompt: string
  model?: string
  provider?: string
  images: ImageGenHit[]
}

// Local, self-contained variant of shared/utils/markdown.tsx's resolveMediaUrl —
// duplicated instead of imported to avoid a circular import (markdown.tsx renders this card).
function resolveUrl(url: string): string {
  if (!url) return ''
  if (url.startsWith('data:') || url.startsWith('http://') || url.startsWith('https://')) {
    return url
  }
  if (url.startsWith('/api/')) {
    const origin = typeof window !== 'undefined' && window.location.port === '9000'
      ? 'http://localhost:3000'
      : (typeof window !== 'undefined' ? window.location.origin : '')
    return `${origin}${url}`
  }
  return url
}

function GeneratedImageTile({ url, single }: { url: string; single: boolean }) {
  const [broken, setBroken] = useState(false)
  const src = resolveUrl(url)

  if (broken || !src) {
    return null
  }

  return (
    <div
      style={{
        position: 'relative',
        display: 'block',
        borderRadius: '10px',
        overflow: 'hidden',
        aspectRatio: single ? undefined : '1 / 1',
        background: 'var(--surface-strong)',
        border: '1px solid var(--border)',
      }}
    >
      <img
        src={src}
        alt="Generated"
        loading="lazy"
        onError={() => setBroken(true)}
        style={{
          width: '100%',
          height: single ? 'auto' : '100%',
          maxHeight: single ? '420px' : undefined,
          objectFit: 'cover',
          display: 'block',
        }}
      />
    </div>
  )
}

export default function ImageGenCard({ data }: { data: ImageGenData }) {
  const images = (data?.images ?? []).filter((img) => img?.url)

  const [isDark, setIsDark] = useState(
    () => (document.documentElement.getAttribute('data-theme') || 'dark') !== 'light'
  )
  useEffect(() => {
    const observer = new MutationObserver(() => {
      setIsDark((document.documentElement.getAttribute('data-theme') || 'dark') !== 'light')
    })
    observer.observe(document.documentElement, { attributeFilter: ['data-theme'] })
    return () => observer.disconnect()
  }, [])

  const cardBg = isDark ? 'rgba(255,255,255,0.03)' : 'rgba(15,23,42,0.04)'
  const cardBorder = isDark ? 'rgba(255,255,255,0.08)' : 'rgba(15,23,42,0.12)'
  const cardText = isDark ? '#f8fafc' : '#0f172a'
  const labelColor = isDark ? 'rgba(255,255,255,0.6)' : '#64748b'
  const metaColor = isDark ? 'rgba(255,255,255,0.5)' : '#94a3b8'
  const promptColor = isDark ? 'rgba(255,255,255,0.7)' : '#475569'
  const emptyColor = isDark ? 'rgba(255,255,255,0.5)' : '#94a3b8'

  return (
    <div
      style={{
        background: cardBg,
        backdropFilter: 'blur(16px)',
        WebkitBackdropFilter: 'blur(16px)',
        color: cardText,
        borderRadius: '12px',
        padding: '16px 20px',
        margin: '12px 0',
        boxShadow: isDark ? '0 8px 32px 0 rgba(0,0,0,0.2)' : '0 4px 20px 0 rgba(15,23,42,0.08)',
        border: `1px solid ${cardBorder}`,
        fontFamily: 'system-ui, -apple-system, sans-serif',
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px' }}>
        <Sparkles size={16} strokeWidth={2} style={{ opacity: 0.9, color: labelColor }} />
        <span
          style={{
            fontSize: '11px',
            fontWeight: 700,
            letterSpacing: '0.8px',
            color: labelColor,
          }}
        >
          IMAGE GENERATED
        </span>
        {data?.model && (
          <span style={{ fontSize: '11px', color: metaColor }}>
            {data.model}{data.provider ? ` · ${data.provider}` : ''}
          </span>
        )}
      </div>

      {images.length === 0 ? (
        <div style={{ fontSize: '13px', color: emptyColor }}>
          No image returned.
        </div>
      ) : images.length === 1 ? (
        <GeneratedImageTile url={images[0].url} single />
      ) : (
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))',
            gap: '8px',
          }}
        >
          {images.map((image, idx) => (
            <GeneratedImageTile key={`${image.url}-${idx}`} url={image.url} single={false} />
          ))}
        </div>
      )}

      {data?.prompt && (
        <div
          style={{
            marginTop: '10px',
            fontSize: '13px',
            lineHeight: 1.4,
            color: promptColor,
          }}
        >
          &ldquo;{data.prompt}&rdquo;
        </div>
      )}
    </div>
  )
}
