import React, { useState } from 'react'
import {
  CloudUpload,
  Sliders,
  Sparkles,
  FileCheck,
  ChevronDown,
  Info,
} from 'lucide-react'

export interface UiMockupOption {
  label: string
  value: string
  options?: string[]
}

export interface UiMockupData {
  title: string
  subtitle?: string
  dropzoneText?: string
  dropzoneHint?: string
  buttonText?: string
  metrics?: Record<string, string>
  options?: UiMockupOption[] | Record<string, string>
  disclaimer?: string
}

export default function UiMockupWidget({ data }: { data: UiMockupData }) {
  if (!data || typeof data !== 'object') return null

  const title = data.title || 'App Preview'
  const subtitle = data.subtitle
  const dropzoneText = data.dropzoneText || 'ลากไฟล์มาวางที่นี่'
  const dropzoneHint = data.dropzoneHint || 'หรือเลือกไฟล์จากอุปกรณ์ของคุณ'
  const buttonText = data.buttonText || 'เลือกไฟล์'
  const disclaimer =
    data.disclaimer || 'ตัวอย่างแนวคิด UI เท่านั้น ยังไม่ใช่เครื่องมือที่ประมวลผลไฟล์ได้จริง'

  // Options state
  let initialOptions: Array<{ label: string; current: string; choices: string[] }> = []
  if (Array.isArray(data.options)) {
    initialOptions = data.options.map((opt) => ({
      label: opt.label,
      current: opt.value,
      choices: opt.options || [opt.value],
    }))
  } else if (data.options && typeof data.options === 'object') {
    initialOptions = Object.entries(data.options).map(([label, value]) => ({
      label,
      current: String(value),
      choices: [String(value)],
    }))
  }

  const [activeOptions, setActiveOptions] = useState(initialOptions)
  const [isSimulatedActive, setIsSimulatedActive] = useState(false)

  const handleOptionClick = (idx: number) => {
    setActiveOptions((prev) => {
      const next = [...prev]
      const cur = next[idx]
      if (cur && cur.choices.length > 1) {
        const curIndex = cur.choices.indexOf(cur.current)
        const nextIndex = (curIndex + 1) % cur.choices.length
        next[idx] = { ...cur, current: cur.choices[nextIndex] }
      }
      return next
    })
  }

  return (
    <div className="chat-ui-mockup-container">
      {/* Top Window Bar */}
      <div className="chat-ui-mockup-header">
        <div className="chat-ui-mockup-dots">
          <span className="chat-ui-mockup-dot dot-red" />
          <span className="chat-ui-mockup-dot dot-amber" />
          <span className="chat-ui-mockup-dot dot-green" />
        </div>
        <div className="chat-ui-mockup-tag">Mockup concept</div>
        <div style={{ width: '42px' }} />
      </div>

      {/* Main Mockup Body */}
      <div className="chat-ui-mockup-body">
        {/* Title & Subtitle */}
        <h3 className="chat-ui-mockup-title">{title}</h3>
        {subtitle && <p className="chat-ui-mockup-subtitle">{subtitle}</p>}

        {/* Dropzone Container */}
        <div
          onClick={() => setIsSimulatedActive((prev) => !prev)}
          className={`chat-ui-mockup-dropzone ${isSimulatedActive ? 'is-active' : ''}`}
        >
          <div className="chat-ui-mockup-dropzone-icon">
            <CloudUpload size={22} />
          </div>
          <div className="chat-ui-mockup-dropzone-text">{dropzoneText}</div>
          <div className="chat-ui-mockup-dropzone-hint">{dropzoneHint}</div>
          <button type="button" className="chat-ui-mockup-dropzone-btn">
            {buttonText}
          </button>
        </div>

        {/* Metrics Rows (e.g. ขนาดไฟล์ 25MB) */}
        {data.metrics && (
          <div className="chat-ui-mockup-metrics">
            {Object.entries(data.metrics).map(([key, val], mIdx) => (
              <div key={mIdx} className="chat-ui-mockup-metric-row">
                <span className="chat-ui-mockup-metric-label">{key}</span>
                <span className="chat-ui-mockup-metric-pill">{val}</span>
              </div>
            ))}
          </div>
        )}

        {/* Options / Settings row */}
        {activeOptions.length > 0 && (
          <div
            className="chat-ui-mockup-options"
            style={{
              gridTemplateColumns: `repeat(${Math.min(activeOptions.length, 2)}, 1fr)`,
            }}
          >
            {activeOptions.map((opt, oIdx) => (
              <div
                key={oIdx}
                onClick={() => handleOptionClick(oIdx)}
                className={`chat-ui-mockup-option-card ${opt.choices.length > 1 ? 'is-clickable' : ''}`}
              >
                <span className="chat-ui-mockup-option-label">{opt.label}</span>
                <span className="chat-ui-mockup-option-value">
                  {opt.current}
                  {opt.choices.length > 1 && <ChevronDown size={13} opacity={0.6} />}
                </span>
              </div>
            ))}
          </div>
        )}

        {/* Footnote / Disclaimer */}
        <div className="chat-ui-mockup-disclaimer">
          <Info size={12} />
          <span>{disclaimer}</span>
        </div>
      </div>
    </div>
  )
}
