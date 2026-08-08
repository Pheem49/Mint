import React, { useState } from 'react'
import { Images } from 'lucide-react'

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
  const src = image.thumbnailUrl || image.imageUrl

  if (broken || !src) {
    return null
  }

  return (
    <a
      href={image.sourceUrl || image.imageUrl}
      target="_blank"
      rel="noopener noreferrer"
      title={image.title}
      style={{
        position: 'relative',
        display: 'block',
        borderRadius: '10px',
        overflow: 'hidden',
        aspectRatio: '1 / 1',
        background: 'rgba(255, 255, 255, 0.04)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        textDecoration: 'none',
      }}
    >
      <img
        src={src}
        alt={image.title}
        loading="lazy"
        onError={() => setBroken(true)}
        style={{
          width: '100%',
          height: '100%',
          objectFit: 'cover',
          display: 'block',
        }}
      />
      {image.title && (
        <div
          style={{
            position: 'absolute',
            left: 0,
            right: 0,
            bottom: 0,
            padding: '6px 8px',
            fontSize: '11px',
            lineHeight: 1.3,
            color: '#f8fafc',
            background: 'linear-gradient(to top, rgba(0,0,0,0.75), rgba(0,0,0,0))',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {image.title}
        </div>
      )}
    </a>
  )
}

export default function ImageSearchCard({ data }: { data: ImageSearchData }) {
  const images = (data?.images ?? []).filter((img) => img.thumbnailUrl || img.imageUrl)

  return (
    <div
      style={{
        background: 'rgba(255, 255, 255, 0.03)',
        backdropFilter: 'blur(16px)',
        WebkitBackdropFilter: 'blur(16px)',
        color: '#f8fafc',
        borderRadius: '12px',
        padding: '16px 20px',
        margin: '12px 0',
        boxShadow: '0 8px 32px 0 rgba(0, 0, 0, 0.2)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        fontFamily: 'system-ui, -apple-system, sans-serif',
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px' }}>
        <Images size={16} strokeWidth={2} style={{ opacity: 0.9 }} />
        <span
          style={{
            fontSize: '11px',
            fontWeight: 700,
            letterSpacing: '0.8px',
            color: 'rgba(255, 255, 255, 0.6)',
          }}
        >
          IMAGE SEARCH
        </span>
        {data?.query && (
          <span style={{ fontSize: '13px', color: 'rgba(255, 255, 255, 0.8)' }}>
            &ldquo;{data.query}&rdquo;
          </span>
        )}
      </div>

      {images.length === 0 ? (
        <div style={{ fontSize: '13px', color: 'rgba(255, 255, 255, 0.5)' }}>
          No images found.
        </div>
      ) : (
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fill, minmax(110px, 1fr))',
            gap: '8px',
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
