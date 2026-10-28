import React, { useState } from 'react'
import { Images } from 'lucide-react'
import { resolveMediaUrl } from '../../utils/markdown'

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

  const cardBg = 'var(--bg-surface-soft)'
  const cardBorder = 'var(--border-subtle)'
  const cardText = 'var(--text-primary)'
  const labelColor = 'var(--text-muted)'
  const queryColor = 'var(--text-secondary)'
  const emptyColor = 'var(--text-muted)'

  return (
    <div
      style={{
        background: cardBg,
        color: cardText,
        borderRadius: '12px',
        padding: '16px 20px',
        margin: '12px 0',
        boxShadow: 'var(--shadow-md)',
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
