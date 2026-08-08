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
        <Calculator size={16} strokeWidth={2} style={{ opacity: 0.9 }} />
        <span style={{ fontSize: '11px', fontWeight: 700, letterSpacing: '0.8px', color: 'rgba(255, 255, 255, 0.6)' }}>
          CALCULATION
        </span>
      </div>

      {/* Input Expression */}
      <div
        style={{
          background: 'rgba(255, 255, 255, 0.04)',
          border: '1px solid rgba(255, 255, 255, 0.06)',
          borderRadius: '8px',
          padding: '10px 14px',
          fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
          fontSize: '14px',
          color: '#cbd5e1',
          letterSpacing: '0.3px',
        }}
      >
        {data.expression}
      </div>

      {/* Equal Divider */}
      <div style={{ display: 'flex', justifyContent: 'center', margin: '6px 0', color: 'rgba(255, 255, 255, 0.4)', fontSize: '16px', fontWeight: 'bold' }}>
        =
      </div>

      {/* Result Display */}
      <div
        style={{
          background: 'rgba(255, 255, 255, 0.04)',
          border: '1px solid rgba(255, 255, 255, 0.08)',
          borderRadius: '10px',
          padding: '12px 18px',
        }}
      >
        <div style={{ fontSize: '30px', fontWeight: 800, color: '#ffffff', fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace', letterSpacing: '-0.5px' }}>
          {formattedResult}
        </div>
      </div>
    </div>
  )
}

