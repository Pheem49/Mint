import React, { useState, useEffect } from 'react'
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
  const accentColor = isPositive ? '#10b981' : '#ef4444'

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

  // Theme-aware color tokens
  const t = {
    cardBg: isDark ? 'rgba(255,255,255,0.03)' : 'rgba(15,23,42,0.04)',
    cardText: isDark ? '#ffffff' : '#0f172a',
    cardBorder: isPositive
      ? isDark ? 'rgba(16,185,129,0.25)' : 'rgba(16,185,129,0.45)'
      : isDark ? 'rgba(239,68,68,0.25)' : 'rgba(239,68,68,0.45)',
    cardShadow: isDark
      ? '0 8px 32px 0 rgba(0,0,0,0.2)'
      : '0 4px 20px 0 rgba(15,23,42,0.08)',
    tagBg: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(15,23,42,0.07)',
    tagColor: isDark ? 'rgba(255,255,255,0.7)' : '#475569',
    subText: isDark ? 'rgba(255,255,255,0.6)' : '#64748b',
    divider: isDark ? 'rgba(255,255,255,0.06)' : 'rgba(15,23,42,0.1)',
    statBg: isDark ? 'rgba(255,255,255,0.04)' : 'rgba(15,23,42,0.05)',
    statBorder: isDark ? 'rgba(255,255,255,0.04)' : 'rgba(15,23,42,0.1)',
    statLabel: isDark ? 'rgba(255,255,255,0.55)' : '#64748b',
    statValue: isDark ? '#ffffff' : '#0f172a',
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
                textTransform: 'uppercase',
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
          <p style={{ margin: 0, fontSize: '10px', color: t.statLabel, textTransform: 'uppercase' }}>Day High</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700, color: t.statValue }}>
            {data.dayHigh ? `${currencySymbol}${data.dayHigh.toFixed(2)}` : 'N/A'}
          </p>
        </div>

        <div style={{ background: t.statBg, borderRadius: '8px', padding: '8px 10px', border: `1px solid ${t.statBorder}` }}>
          <p style={{ margin: 0, fontSize: '10px', color: t.statLabel, textTransform: 'uppercase' }}>Day Low</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700, color: t.statValue }}>
            {data.dayLow ? `${currencySymbol}${data.dayLow.toFixed(2)}` : 'N/A'}
          </p>
        </div>

        <div style={{ background: t.statBg, borderRadius: '8px', padding: '8px 10px', border: `1px solid ${t.statBorder}` }}>
          <p style={{ margin: 0, fontSize: '10px', color: t.statLabel, textTransform: 'uppercase' }}>Mkt Cap</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700, color: t.statValue }}>{formatLargeNum(data.marketCap)}</p>
        </div>

        <div style={{ background: t.statBg, borderRadius: '8px', padding: '8px 10px', border: `1px solid ${t.statBorder}` }}>
          <p style={{ margin: 0, fontSize: '10px', color: t.statLabel, textTransform: 'uppercase' }}>Volume</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700, color: t.statValue }}>{formatLargeNum(data.volume)}</p>
        </div>
      </div>
    </div>
  )
}
