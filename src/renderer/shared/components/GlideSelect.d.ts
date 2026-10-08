import type { CSSProperties, ReactNode } from 'react'
export interface GlideOption { value: string; label: ReactNode; tag?: string }
export interface GlideSelectProps {
  options?: (string | GlideOption)[]
  value?: string
  defaultValue?: string
  onChange?: (value: string, option: GlideOption) => void
  placeholder?: string
  showTags?: boolean
  accentColor?: string
  surfaceColor?: string
  highlightColor?: string
  textColor?: string
  size?: 'sm' | 'md' | 'lg'
  radius?: number
  menuWidth?: number
  placement?: 'top' | 'bottom'
  align?: 'left' | 'right'
  popDuration?: number
  glideDuration?: number
  rememberPosition?: boolean
  disabled?: boolean
  ariaLabel?: string
  className?: string
  id?: string
  labelledBy?: string
  title?: string
  style?: CSSProperties
  fullWidth?: boolean
}
export default function GlideSelect(props: GlideSelectProps): ReactNode
