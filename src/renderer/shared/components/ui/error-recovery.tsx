import { ArrowLeft, RefreshCw } from 'lucide-react'
import { Button } from './button'
import { VoicePoweredOrb } from './voice-powered-orb'
import './error-recovery.css'

export interface ErrorRecoveryProps {
  onBackToChat: () => void
  onRefresh: () => void
}

/** The application fallback must work independently of the loaded app's theme. */
export function ErrorRecovery({ onBackToChat, onRefresh }: ErrorRecoveryProps) {
  return (
    <main className="error-recovery" style={{ flex: 1, minHeight: 0, overflowY: 'auto', color: '#f4f4f5' }}>
      <div className="error-recovery-content">
        <div className="error-recovery-orb" style={{ width: 'min(280px, 60vw, 34vh)', aspectRatio: '1' }}>
          <VoicePoweredOrb animate={false} enableVoiceControl={false} hue={0} />
        </div>
        <div className="error-recovery-copy" role="alert">
          <p className="error-recovery-label">APPLICATION ERROR</p>
          <h1>Mint needs to refresh</h1>
          <p>An asset failed to load or a new version was deployed. Refreshing will load the latest version.</p>
        </div>
        <div className="error-recovery-actions">
          <Button type="button" variant="outline" size="lg" onClick={onBackToChat}>
            <ArrowLeft size={16} aria-hidden="true" />Back to Chat
          </Button>
          <Button type="button" size="lg" onClick={onRefresh}>
            <RefreshCw size={16} aria-hidden="true" />Refresh &amp; Update
          </Button>
        </div>
      </div>
    </main>
  )
}
