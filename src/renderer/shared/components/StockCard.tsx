import React from 'react'

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

  const formattedPrice = data.price.toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })

  const formattedChange = (data.change >= 0 ? '+' : '') + data.change.toFixed(2)
  const formattedPercent = (data.changePercent >= 0 ? '+' : '') + data.changePercent.toFixed(2) + '%'
  const currencySymbol = data.currency === 'USD' ? '$' : data.currency === 'THB' ? '฿' : `${data.currency} `

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
        background: 'rgba(255, 255, 255, 0.03)',
        backdropFilter: 'blur(16px)',
        WebkitBackdropFilter: 'blur(16px)',
        color: '#ffffff',
        borderRadius: '12px',
        padding: '16px 20px',
        margin: '12px 0',
        boxShadow: '0 8px 32px 0 rgba(0, 0, 0, 0.2)',
        border: `1px solid ${isPositive ? 'rgba(16, 185, 129, 0.25)' : 'rgba(239, 68, 68, 0.25)'}`,
        fontFamily: 'system-ui, -apple-system, sans-serif',
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '12px' }}>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <h3 style={{ margin: 0, fontSize: '20px', fontWeight: 800, letterSpacing: '-0.3px' }}>{data.symbol}</h3>
            <span
              style={{
                fontSize: '11px',
                fontWeight: 700,
                padding: '2px 6px',
                borderRadius: '4px',
                background: 'rgba(255, 255, 255, 0.08)',
                color: 'rgba(255, 255, 255, 0.7)',
                textTransform: 'uppercase',
              }}
            >
              {data.currency || 'USD'}
            </span>
          </div>
          <p style={{ margin: '2px 0 0', fontSize: '13px', opacity: 0.7, fontWeight: 500 }}>{data.name}</p>
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
          <span>{isPositive ? '▲' : '▼'}</span>
          <span>{formattedChange} ({formattedPercent})</span>
        </div>
      </div>

      {/* Main Price */}
      <div style={{ marginBottom: '14px', borderBottom: '1px solid rgba(255, 255, 255, 0.06)', paddingBottom: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: '4px' }}>
          <span style={{ fontSize: '36px', fontWeight: 800, letterSpacing: '-1px' }}>
            {currencySymbol}{formattedPrice}
          </span>
        </div>
      </div>

      {/* Stats Grid */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '8px' }}>
        <div style={{ background: 'rgba(255, 255, 255, 0.04)', borderRadius: '8px', padding: '8px 10px', border: '1px solid rgba(255, 255, 255, 0.04)' }}>
          <p style={{ margin: 0, fontSize: '10px', opacity: 0.6, textTransform: 'uppercase' }}>Day High</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700 }}>
            {data.dayHigh ? `${currencySymbol}${data.dayHigh.toFixed(2)}` : 'N/A'}
          </p>
        </div>

        <div style={{ background: 'rgba(255, 255, 255, 0.04)', borderRadius: '8px', padding: '8px 10px', border: '1px solid rgba(255, 255, 255, 0.04)' }}>
          <p style={{ margin: 0, fontSize: '10px', opacity: 0.6, textTransform: 'uppercase' }}>Day Low</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700 }}>
            {data.dayLow ? `${currencySymbol}${data.dayLow.toFixed(2)}` : 'N/A'}
          </p>
        </div>

        <div style={{ background: 'rgba(255, 255, 255, 0.04)', borderRadius: '8px', padding: '8px 10px', border: '1px solid rgba(255, 255, 255, 0.04)' }}>
          <p style={{ margin: 0, fontSize: '10px', opacity: 0.6, textTransform: 'uppercase' }}>Mkt Cap</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700 }}>{formatLargeNum(data.marketCap)}</p>
        </div>

        <div style={{ background: 'rgba(255, 255, 255, 0.04)', borderRadius: '8px', padding: '8px 10px', border: '1px solid rgba(255, 255, 255, 0.04)' }}>
          <p style={{ margin: 0, fontSize: '10px', opacity: 0.6, textTransform: 'uppercase' }}>Volume</p>
          <p style={{ margin: '2px 0 0', fontSize: '12px', fontWeight: 700 }}>{formatLargeNum(data.volume)}</p>
        </div>
      </div>
    </div>
  )
}
