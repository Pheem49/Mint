import React from 'react'
import { Calculator } from 'lucide-react'

export interface CalculationData {
  expression: string
  result: number
}

export default function CalculationCard({ data }: { data: CalculationData }) {
  const formattedResult = typeof data.result === 'number'
    ? (Number.isInteger(data.result) ? data.result.toLocaleString() : data.result.toLocaleString(undefined, { maximumFractionDigits: 6 }))
    : String(data.result)

  const t = {
    cardBg: 'var(--bg-surface-soft)',
    cardText: 'var(--text-primary)',
    cardBorder: 'var(--border-subtle)',
    cardShadow: 'var(--shadow-md)',
    labelColor: 'var(--text-muted)',
    exprBg: 'var(--bg-surface-raised)',
    exprBorder: 'var(--border-subtle)',
    exprText: 'var(--text-secondary)',
    dividerColor: 'var(--text-muted)',
    resultBg: 'var(--bg-surface-raised)',
    resultBorder: 'var(--border-default)',
    resultText: 'var(--text-primary)',
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
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px' }}>
        <Calculator size={16} strokeWidth={2} style={{ opacity: 0.9, color: t.labelColor }} />
        <span style={{ fontSize: '11px', fontWeight: 700, letterSpacing: '0.8px', color: t.labelColor }}>
          CALCULATION
        </span>
      </div>

      {/* Input Expression */}
      <div
        style={{
          background: t.exprBg,
          border: `1px solid ${t.exprBorder}`,
          borderRadius: '8px',
          padding: '10px 14px',
          fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
          fontSize: '14px',
          color: t.exprText,
          letterSpacing: '0.3px',
        }}
      >
        {data.expression}
      </div>

      {/* Equal Divider */}
      <div style={{ display: 'flex', justifyContent: 'center', margin: '6px 0', color: t.dividerColor, fontSize: '16px', fontWeight: 'bold' }}>
        =
      </div>

      {/* Result Display */}
      <div
        style={{
          background: t.resultBg,
          border: `1px solid ${t.resultBorder}`,
          borderRadius: '10px',
          padding: '12px 18px',
        }}
      >
        <div style={{ fontSize: '30px', fontWeight: 800, color: t.resultText, fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace', letterSpacing: '-0.5px' }}>
          {formattedResult}
        </div>
      </div>
    </div>
  )
}
