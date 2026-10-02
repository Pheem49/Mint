import React, { useState, useEffect } from 'react'
import { Calculator } from 'lucide-react'

export interface CalculationData {
  expression: string
  result: number
}

export default function CalculationCard({ data }: { data: CalculationData }) {
  const formattedResult = typeof data.result === 'number'
    ? (Number.isInteger(data.result) ? data.result.toLocaleString() : data.result.toLocaleString(undefined, { maximumFractionDigits: 6 }))
    : String(data.result)

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

  const t = {
    cardBg: isDark ? 'rgba(255,255,255,0.03)' : 'rgba(15,23,42,0.04)',
    cardText: isDark ? '#f8fafc' : '#0f172a',
    cardBorder: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(15,23,42,0.12)',
    cardShadow: isDark ? '0 8px 32px 0 rgba(0,0,0,0.2)' : '0 4px 20px 0 rgba(15,23,42,0.08)',
    labelColor: isDark ? 'rgba(255,255,255,0.6)' : '#64748b',
    exprBg: isDark ? 'rgba(255,255,255,0.04)' : 'rgba(15,23,42,0.05)',
    exprBorder: isDark ? 'rgba(255,255,255,0.06)' : 'rgba(15,23,42,0.1)',
    exprText: isDark ? '#cbd5e1' : '#334155',
    dividerColor: isDark ? 'rgba(255,255,255,0.4)' : '#94a3b8',
    resultBg: isDark ? 'rgba(255,255,255,0.04)' : 'rgba(15,23,42,0.05)',
    resultBorder: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(15,23,42,0.12)',
    resultText: isDark ? '#ffffff' : '#0f172a',
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
