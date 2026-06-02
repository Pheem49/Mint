import Live2DStage from './Live2DStage'
import type { DashboardView } from './DashboardSidebar'

type LayoutPreset = 'chat-wide' | 'model-wide'

interface ModelPanelProps {
  scale: number
  expressionIndex: number
  accessoryIndex: number
  isLocked: boolean
  isActive: boolean
  layoutPreset: LayoutPreset
  sending: boolean
  showInteractionGuide: boolean
  toastMessage: string
  onSetScale: (scale: number) => void
  onSetLocked: (locked: boolean) => void
  onSetView: (view: DashboardView) => void
  onChangeLayoutPreset: (preset: LayoutPreset) => void
  onDismissToast: () => void
  onPokeCheek: () => void
}

export default function ModelPanel({
  scale,
  expressionIndex,
  accessoryIndex,
  isLocked,
  isActive,
  layoutPreset,
  sending,
  showInteractionGuide,
  toastMessage,
  onSetScale,
  onSetLocked,
  onSetView,
  onChangeLayoutPreset,
  onDismissToast,
  onPokeCheek,
}: ModelPanelProps) {
  return (
    <section className="model-stage">
      <div className={`model-shell ${showInteractionGuide ? 'show-interaction-guide' : ''} model-bg-stage`}>
        <div className="model-panel-controls">
          <button
            className={`model-panel-control ${isLocked ? 'is-active' : ''}`}
            onClick={() => onSetLocked(!isLocked)}
            title={isLocked ? 'Unlock stage interactions' : 'Lock stage interactions'}
          >
            {isLocked ? (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
              </svg>
            ) : (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                <path d="M7 11V7a5 5 0 0 1 9.9-1"></path>
              </svg>
            )}
          </button>
          <button className="model-panel-control" onClick={() => window.api.maximizeWindow()} title="Maximize Window">⤢</button>
          <div className="model-scale-control">
            <input
              type="range"
              min="0.5"
              max="2.0"
              step="0.05"
              value={scale}
              onChange={(event) => onSetScale(parseFloat(event.target.value))}
              disabled={isLocked}
            />
            <span className="model-scale-value">{scale.toFixed(2)}x</span>
            <button className="model-scale-reset" onClick={() => onSetScale(1.00)} disabled={isLocked} title="Reset Scale">⟲</button>
          </div>
          <button className="model-panel-control" onClick={() => onSetView('pictures')} title="Saved Pictures">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
              <circle cx="8.5" cy="8.5" r="1.5"></circle>
              <polyline points="21 15 16 10 5 21"></polyline>
            </svg>
          </button>
          <div className="layout-preset-group">
            <button className={`layout-preset-btn ${layoutPreset === 'chat-wide' ? 'is-active' : ''}`} onClick={() => onChangeLayoutPreset('chat-wide')} title="ขยายหน้าต่างแชท (ย่อโมเดล)">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <path d="M9 3v18" />
              </svg>
            </button>
            <button className={`layout-preset-btn ${layoutPreset === 'model-wide' ? 'is-active' : ''}`} onClick={() => onChangeLayoutPreset('model-wide')} title="ขยายหน้าต่างโมเดล (ย่อแชท)">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <path d="M15 3v18" />
              </svg>
            </button>
          </div>
        </div>

        {toastMessage && (
          <div className="mint-notification" style={{ position: 'absolute', left: '14px', top: '62px', zIndex: 10, margin: 0, padding: '8px 14px', borderRadius: '8px', background: 'rgba(18, 18, 22, 0.85)', border: '1px solid rgba(255, 255, 255, 0.08)', backdropFilter: 'blur(12px)', color: '#f8fafc', fontSize: '0.82rem', boxShadow: '0 8px 22px rgba(0,0,0,0.2)', display: 'flex', alignItems: 'center', gap: '12px', transition: 'all 0.22s ease' }}>
            <span>{toastMessage}</span>
            <button onClick={onDismissToast} style={{ background: 'transparent', border: 0, color: 'white', cursor: 'pointer', fontSize: '0.9rem' }}>✕</button>
          </div>
        )}

        <div className="model-activity-badge" data-state={sending ? "thinking" : "idle"}>
          <span className="mint-status-dot" />
          <span>{sending ? "Thinking" : "Idle"}</span>
        </div>

        <div className="model-mount" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', pointerEvents: isLocked ? 'none' : 'auto' }}>
          <Live2DStage scale={scale} expressionIndex={expressionIndex} accessoryIndex={accessoryIndex} isLocked={isLocked} isActive={isActive} />
          <div className="interaction-guide">
            <div className="interaction-zone zone-head" onClick={() => alert("Poked head!")}><span>Head</span></div>
            <div className="interaction-zone zone-face" onClick={onPokeCheek}><span>Cheek</span></div>
            <div className="interaction-zone zone-left-hand" onClick={() => alert("Poked left hand!")}><span>Hand</span></div>
            <div className="interaction-zone zone-right-hand" onClick={() => alert("Poked right hand!")}><span>Hand</span></div>
            <div className="interaction-zone zone-body" onClick={() => alert("Poked body!")}><span>Body</span></div>
            <div className="interaction-zone zone-lower" onClick={() => alert("Poked legs!")}><span>Lower</span></div>
          </div>
        </div>
      </div>
    </section>
  )
}
