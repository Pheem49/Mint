import React, { useState, useEffect } from 'react'
import { Images } from 'lucide-react'
import { resolveMediaUrl } from '../utils/markdown'

export interface ImageSearchHit {
  title: string
  imageUrl: string
  thumbnailUrl: string
  sourceUrl: string
  width?: number | null
  height?: number | null
}

export interface ImageSearchData {
  query: string
  provider?: string
  images: ImageSearchHit[]
}

function ImageTile({ image }: { image: ImageSearchHit }) {
  const [broken, setBroken] = useState(false)
  const initialSrc = resolveMediaUrl(image.thumbnailUrl || image.imageUrl)
  const [src, setSrc] = useState(initialSrc)

  const sourceUrl = /^https?:\/\//.test(image.sourceUrl) ? image.sourceUrl : image.imageUrl

  return (
    <article
      style={{
        display: 'flex',
        flex: '0 0 116px',
        flexDirection: 'column',
        borderRadius: '10px',
        overflow: 'hidden',
        background: 'var(--surface-strong)',
        border: '1px solid var(--border)',
      }}
    >
      <a href={sourceUrl} target="_blank" rel="noopener noreferrer" title={image.title} style={{ display: 'block', width: '100%', aspectRatio: '1 / 1', flexShrink: 0, textDecoration: 'none' }}>
        {broken || !src ? (
          <span style={{ width: '100%', height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', padding: '12px', boxSizing: 'border-box', color: 'var(--text-muted, #94a3b8)', fontSize: '11px', lineHeight: 1.4, textAlign: 'center' }}>
            Preview unavailable · Open source
          </span>
        ) : (
          <img
            src={src}
            alt={image.title}
            loading="lazy"
            referrerPolicy="no-referrer"
            onError={() => {
              const fallback = resolveMediaUrl(image.imageUrl)
              if (src !== fallback && fallback) {
                setSrc(fallback)
              } else {
                setBroken(true)
              }
            }}
            style={{ width: '100%', height: '100%', objectFit: 'cover', display: 'block' }}
          />
        )}
      </a>
      <div style={{ minWidth: 0, padding: '7px 8px', display: 'grid', gap: '5px' }}>
        {image.title && (
          <div title={image.title} style={{ fontSize: '11px', lineHeight: 1.3, color: 'var(--text-primary, inherit)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {image.title}
          </div>
        )}
        <a
          href={resolveMediaUrl(image.imageUrl)}
          target="_blank"
          rel="noopener noreferrer"
          style={{ justifySelf: 'start', color: 'var(--interactive-fg-hover)', fontSize: '10px', lineHeight: 1.4, textDecoration: 'underline', textUnderlineOffset: '2px' }}
        >
          View full image
        </a>
      </div>
    </article>
  )
}

export default function ImageSearchCard({ data }: { data: ImageSearchData }) {
  const images = (data?.images ?? []).filter((img) => img.thumbnailUrl || img.imageUrl)

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
  const queryColor = isDark ? 'rgba(255,255,255,0.8)' : '#334155'
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
        <Images size={16} strokeWidth={2} style={{ opacity: 0.9, color: labelColor }} />
        <span
          style={{
            fontSize: '11px',
            fontWeight: 700,
            letterSpacing: '0.8px',
            color: labelColor,
          }}
        >
          IMAGE SEARCH
        </span>
        {data?.query && (
          <span title={data.query} style={{ minWidth: 0, fontSize: '13px', color: queryColor, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            &ldquo;{data.query}&rdquo;
          </span>
        )}
      </div>

      {images.length === 0 ? (
        <div style={{ fontSize: '13px', color: emptyColor }}>
          No images found.
        </div>
      ) : (
        <div
          style={{
            display: 'flex',
            gap: '8px',
            overflowX: 'auto',
            paddingBottom: '4px',
            scrollbarWidth: 'thin',
          }}
        >
          {images.map((image, idx) => (
            <ImageTile key={`${image.imageUrl}-${idx}`} image={image} />
          ))}
        </div>
      )}
    </div>
  )
}
