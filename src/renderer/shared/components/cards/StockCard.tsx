import React from 'react'
import { TrendingUp, TrendingDown } from 'lucide-react'

export interface StockData {
  symbol: string
  name: string
  price: number
  change: number
  changePercent: number
  currency?: string
  dayHigh?: number
  dayLow?: number
  marketCap?: number
  volume?: number
}

export default function StockCard({ data }: { data: StockData }) {
  const isPositive = data.change >= 0
  const accentColor = isPositive ? 'var(--status-success, #10b981)' : 'var(--status-error, #ef4444)'

  // Theme-aware color tokens
  const t = {
    cardBg: 'var(--bg-surface-soft)',
    cardText: 'var(--text-primary)',
    cardBorder: isPositive
      ? 'color-mix(in srgb, var(--status-success, #10b981) 35%, var(--border-subtle))'
      : 'color-mix(in srgb, var(--status-error, #ef4444) 35%, var(--border-subtle))',
    cardShadow: 'var(--shadow-md)',
    tagBg: 'var(--bg-surface-raised)',
    tagColor: 'var(--text-secondary)',
    subText: 'var(--text-muted)',
    divider: 'var(--border-subtle)',
    statBg: 'var(--bg-surface-raised)',
    statBorder: 'var(--border-subtle)',
    statLabel: 'var(--text-muted)',
    statValue: 'var(--text-primary)',
  }

  const formattedPrice = data.price.toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })

  const formattedChange = (data.change >= 0 ? '+' : '') + data.change.toFixed(2)
  const formattedPercent = (data.changePercent >= 0 ? '+' : '') + data.changePercent.toFixed(2) + '%'
  const currency = data.currency || 'USD'
  const currencySymbol = currency === 'USD' ? '$' : currency === 'THB' ? '฿' : `${currency} `

  const formatLargeNum = (num?: number) => {
    if (!num || num === 0) return 'N/A'
    if (num >= 1e12) return (num / 1e12).toFixed(2) + 'T'
    if (num >= 1e9) return (num / 1e9).toFixed(2) + 'B'
    if (num >= 1e6) return (num / 1e6).toFixed(2) + 'M'
    if (num >= 1e3) return (num / 1e3).toFixed(2) + 'K'
    return num.toLocaleString()
  }

  return (
    <div
      style={{
        background: t.cardBg,
        backdropFilter: 'blur(16px)',
        WebkitBackdropFilter: 'blur(16px)',
        color: t.cardText,
        borderRadius: '12px',
        padding: '16px 20px',
        margin: '12px 0',
        boxShadow: t.cardShadow,
        border: `1px solid ${t.cardBorder}`,
        fontFamily: 'system-ui, -apple-system, sans-serif',
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '12px' }}>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <h3 style={{ margin: 0, fontSize: '20px', fontWeight: 800, letterSpacing: '-0.3px', color: t.cardText }}>
              {data.symbol}
            </h3>
            <span
              style={{
                fontSize: '11px',
                fontWeight: 700,
                padding: '2px 6px',
                borderRadius: '4px',
                background: t.tagBg,
                color: t.tagColor,
                textTransform: 'none',
              }}
            >
              {currency}
            </span>
          </div>
          <p style={{ margin: '2px 0 0', fontSize: '13px', color: t.subText, fontWeight: 500 }}>{data.name}</p>
        </div>

        {/* Change Badge */}
        <div
          style={{
            background: isPositive ? 'rgba(16, 185, 129, 0.12)' : 'rgba(239, 68, 68, 0.12)',
            border: `1px solid ${isPositive ? 'rgba(16, 185, 129, 0.3)' : 'rgba(239, 68, 68, 0.3)'}`,
            padding: '4px 10px',
            borderRadius: '20px',
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            color: accentColor,
            fontWeight: 700,
            fontSize: '13px',
          }}
        >
          {isPositive ? <TrendingUp size={14} strokeWidth={2.5} /> : <TrendingDown size={14} strokeWidth={2.5} />}
          <span>{formattedChange} ({formattedPercent})</span>
        </div>
      </div>

      {/* Main Price */}
      <div style={{ marginBottom: '14px', borderBottom: `1px solid ${t.divider}`, paddingBottom: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: '4px' }}>
          <span style={{ fontSize: '36px', fontWeight: 800, letterSpacing: '-1px', color: t.cardText }}>
            {currencySymbol}{formattedPrice}
          </span>
        </div>
      </div>

      {/* Stats Grid */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '8px' }}>
        <div style={{ background: t.statBg, borderRadius: '8px', padding: '8px 10px', border: `1px solid ${t.statBorder}` }}>
          <p style={{ margin: 0, fontSize: '10px', color: t.statLabel, textTransform: 'none' }}>Day high</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700, color: t.statValue }}>
            {data.dayHigh ? `${currencySymbol}${data.dayHigh.toFixed(2)}` : 'N/A'}
          </p>
        </div>

        <div style={{ background: t.statBg, borderRadius: '8px', padding: '8px 10px', border: `1px solid ${t.statBorder}` }}>
          <p style={{ margin: 0, fontSize: '10px', color: t.statLabel, textTransform: 'none' }}>Day low</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700, color: t.statValue }}>
            {data.dayLow ? `${currencySymbol}${data.dayLow.toFixed(2)}` : 'N/A'}
          </p>
        </div>

        <div style={{ background: t.statBg, borderRadius: '8px', padding: '8px 10px', border: `1px solid ${t.statBorder}` }}>
          <p style={{ margin: 0, fontSize: '10px', color: t.statLabel, textTransform: 'none' }}>Market cap</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700, color: t.statValue }}>{formatLargeNum(data.marketCap)}</p>
        </div>

        <div style={{ background: t.statBg, borderRadius: '8px', padding: '8px 10px', border: `1px solid ${t.statBorder}` }}>
          <p style={{ margin: 0, fontSize: '10px', color: t.statLabel, textTransform: 'none' }}>Volume</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700, color: t.statValue }}>{formatLargeNum(data.volume)}</p>
        </div>
      </div>
    </div>
  )
}
