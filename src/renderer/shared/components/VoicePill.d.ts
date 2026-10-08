import type { ReactNode } from 'react'
export type VoiceStopReason = 'release' | 'tap' | 'key' | 'escape' | 'blur' | 'cancel' | 'disabled' | 'mic-denied' | 'unmount'
export interface VoicePillProps {
  accentColor?: string; iconColor?: string; background?: string; size?: number; shape?: 'pill' | 'rounded'; reach?: number
  showTime?: boolean; waveform?: boolean; slideToCancel?: boolean; cancelDistance?: number; attack?: number; release?: number
  sensitivity?: number; floor?: number; openDuration?: number; pressScale?: number; mode?: 'auto' | 'hold' | 'toggle'; holdAfter?: number
  reactive?: 'simulated' | 'mic'; disabled?: boolean; ariaLabel?: string; className?: string
  onStart?: (event: { source: 'simulated' | 'mic' }) => void | boolean | Promise<void | boolean>
  onStop?: (event: { reason: VoiceStopReason; duration: number }) => void
  active?: boolean; id?: string; title?: string
}
export default function VoicePill(props: VoicePillProps): ReactNode
