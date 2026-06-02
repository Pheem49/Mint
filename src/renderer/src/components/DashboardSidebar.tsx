export type DashboardView = 'chat' | 'pictures' | 'model'

interface DashboardSidebarProps {
  view: DashboardView
  sidebarCollapsed: boolean
  modelVisible: boolean
  sending: boolean
  expressionIndex: number
  accessoryIndex: number
  expressions: string[]
  accessories: string[]
  interactionEnabled: boolean
  showInteractionGuide: boolean
  onToggleSidebar: () => void
  onClearHistory: (action: 'New chat' | 'Clear history') => void
  onSetView: (view: DashboardView) => void
  onToggleModel: () => void
  onSetExpressionIndex: (index: number) => void
  onSetAccessoryIndex: (index: number) => void
  onSetInteractionEnabled: (enabled: boolean) => void
  onSetShowInteractionGuide: (visible: boolean) => void
  onShowToast: (message: string) => void
}

export default function DashboardSidebar({
  view,
  sidebarCollapsed,
  modelVisible,
  sending,
  expressionIndex,
  accessoryIndex,
  expressions,
  accessories,
  interactionEnabled,
  showInteractionGuide,
  onToggleSidebar,
  onClearHistory,
  onSetView,
  onToggleModel,
  onSetExpressionIndex,
  onSetAccessoryIndex,
  onSetInteractionEnabled,
  onSetShowInteractionGuide,
  onShowToast,
}: DashboardSidebarProps) {
  const toggleInteractionGuide = () => {
    const next = !showInteractionGuide
    onSetShowInteractionGuide(next)
    onShowToast(next ? 'เปิดการแสดงจุดสัมผัส (Interaction Zones) ⊹' : 'ปิดการแสดงจุดสัมผัส ⊹')
  }

  const toggleInteraction = () => {
    const next = !interactionEnabled
    onSetInteractionEnabled(next)
    onShowToast(next ? 'เปิดการโต้ตอบกับโมเดล ⦸' : 'ปิดการโต้ตอบกับโมเดล ⦸')
  }

  return (
    <aside className="workspace-sidebar">
      <div
        className="sidebar-brand clickable"
        onClick={onToggleSidebar}
        title={sidebarCollapsed ? "ขยายแถบด้านข้าง" : "ยุบแถบด้านข้าง"}
        role="button"
        tabIndex={0}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') onToggleSidebar()
        }}
      >
        <img src="./assets/icon.png" alt="Agent Mint Logo" className="sidebar-logo" />
        <span className="sidebar-brand-name">Agent Mint</span>
      </div>

      <button className="sidebar-new-chat" onClick={() => onClearHistory('New chat')}>
        <span aria-hidden="true">+</span>
        <span>New Chat</span>
      </button>

      <button className={`sidebar-top-action ${view === 'chat' ? 'is-active' : ''}`} onClick={() => onSetView('chat')}>
        <span aria-hidden="true">🗏</span>
        <span>Chat</span>
      </button>
      <button className={`sidebar-top-action ${view === 'pictures' ? 'is-active' : ''}`} onClick={() => onSetView('pictures')}>
        <span aria-hidden="true">▨</span>
        <span>Pictures</span>
      </button>

      <div className="sidebar-section">
        <div className="sidebar-section-title clickable" onClick={onToggleModel} title="เปิด/ปิดการแสดง Live2D Model">
          ☰ Model
        </div>
        <div className="sidebar-model-controls">
          <button
            className="change-expression-btn"
            onClick={() => {
              const next = (expressionIndex + 1) % expressions.length
              onSetExpressionIndex(next)
              onSetAccessoryIndex(0)
              onShowToast(`Expression: ${expressions[next]}`)
            }}
          >
            <span aria-hidden="true">☆</span>
            <span>Expression</span>
          </button>
          <button
            className="accessory-cycle-btn"
            onClick={() => {
              const next = (accessoryIndex + 1) % accessories.length
              onSetAccessoryIndex(next)
              onSetExpressionIndex(0)
              onShowToast(`Accessory: ${accessories[next]}`)
            }}
          >
            <span aria-hidden="true">∞</span>
            <span>Accessory</span>
          </button>
          <button className={`toggle-interaction-btn ${interactionEnabled ? 'active' : ''}`} onClick={toggleInteraction}>
            <span aria-hidden="true">⦸</span>
            <span className="mint-status-label">Interact</span>
          </button>
          <button className={`interaction-guide-btn ${showInteractionGuide ? 'active' : ''}`} onClick={toggleInteractionGuide}>
            <span aria-hidden="true">⊹</span>
            <span>Areas</span>
          </button>
        </div>
      </div>

      <div className="sidebar-section">
        <div className="sidebar-section-title">Assistant</div>
        <button className="sidebar-project active">
          <span aria-hidden="true">🗏</span>
          <span>Mint</span>
          <span className="mint-status-pill" data-state={sending ? "thinking" : "idle"}>
            <span className="mint-status-dot" />
            <span className="mint-status-label">{sending ? "Thinking" : "Idle"}</span>
          </span>
        </button>
        <button className={`sidebar-project ${modelVisible ? 'is-active' : ''}`} onClick={onToggleModel}>
          <span aria-hidden="true">♢</span>
          <span>Live2D Model</span>
        </button>
        <button className="sidebar-project" onClick={() => onShowToast("เปิดใช้งาน Smart Tools: พร้อมช่วยสแกนและวิเคราะห์แล้วค่ะ! ∽")}>
          <span aria-hidden="true">∽</span>
          <span>Smart Tools</span>
        </button>
      </div>

      <div className="sidebar-bottom-actions">
        <button className="clear-btn" onClick={() => onClearHistory('Clear history')}>
          <span aria-hidden="true">🗑</span>
          <span>Clear</span>
        </button>
        <button className="settings-btn" onClick={() => window.api.openSettings()}>
          <span aria-hidden="true">⚙</span>
          <span>Settings</span>
        </button>
      </div>
    </aside>
  )
}
