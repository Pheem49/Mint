import React from 'react'
import { resolveMediaUrl } from '../../utils/markdown'
import {
  MessageSquare,
  Mail,
  Smartphone,
  Video,
  Database,
  Code,
  Terminal,
  File,
  FileText,
  Folder,
  Cpu,
  Zap,
  Sparkles,
  Check,
  AlertCircle,
  Layers,
  Palette,
  Globe,
  Box,
  Package,
  Crop,
  Shield,
  Cloud,
  Settings,
  Download,
  Upload,
  Image,
  Play,
  Music,
  HardDrive,
  Sliders,
  ExternalLink,
  Lock,
  CreditCard,
  UserCheck,
  User,
  Repeat,
  AlertTriangle,
  Key,
  Clock,
  type LucideIcon,
} from 'lucide-react'

const ICON_MAP: Record<string, LucideIcon> = {
  'message-square': MessageSquare,
  chat: MessageSquare,
  mail: Mail,
  email: Mail,
  smartphone: Smartphone,
  phone: Smartphone,
  mobile: Smartphone,
  video: Video,
  camera: Video,
  database: Database,
  db: Database,
  code: Code,
  terminal: Terminal,
  file: File,
  'file-text': FileText,
  folder: Folder,
  cpu: Cpu,
  zap: Zap,
  fast: Zap,
  sparkles: Sparkles,
  check: Check,
  'alert-circle': AlertCircle,
  'alert-triangle': AlertTriangle,
  warning: AlertTriangle,
  lock: Lock,
  security: Shield,
  shield: Shield,
  key: Key,
  'credit-card': CreditCard,
  payment: CreditCard,
  user: User,
  'user-check': UserCheck,
  repeat: Repeat,
  sync: Repeat,
  clock: Clock,
  time: Clock,
  layers: Layers,
  palette: Palette,
  globe: Globe,
  web: Globe,
  box: Box,
  package: Package,
  crop: Crop,
  scan: Crop,
  cloud: Cloud,
  settings: Settings,
  download: Download,
  upload: Upload,
  image: Image,
  play: Play,
  music: Music,
  'hard-drive': HardDrive,
  storage: HardDrive,
  sliders: Sliders,
  filter: Sliders,
}

function resolveIcon(name?: string): LucideIcon {
  if (!name) return Sparkles
  const clean = name.toLowerCase().trim().replace(/_/g, '-')
  return ICON_MAP[clean] || Sparkles
}

export interface UiFeatureItem {
  title: string
  subtitle?: string
  badge?: string
  badgeColor?: 'green' | 'blue' | 'purple' | 'amber' | 'red'
  thumbnail?: string
  icon?: string
  details?: Record<string, string> | Array<{ label: string; value: string }>
  tags?: string[]
  link?: string
}

export interface UiFeatureData {
  title?: string
  subtitle?: string
  items?: UiFeatureItem[]
}

const BADGE_THEMES: Record<string, { bg: string; color: string; border: string }> = {
  green: { bg: 'var(--status-success-bg, var(--accent-subtle))', color: 'var(--status-success-fg, var(--interactive-fg))', border: 'var(--status-success-border, var(--accent-border))' },
  blue: { bg: 'var(--status-info-bg)', color: 'var(--status-info-fg)', border: 'var(--status-info-border)' },
  purple: { bg: 'var(--status-purple-bg)', color: 'var(--status-purple-fg)', border: 'var(--status-purple-border)' },
  amber: { bg: 'var(--status-warning-bg)', color: 'var(--status-warning-fg)', border: 'var(--status-warning-border)' },
  red: { bg: 'var(--status-error-bg)', color: 'var(--status-error-fg)', border: 'var(--status-error-border)' },
}

export default function UiFeatureCard({ data }: { data: UiFeatureData | UiFeatureItem[] | UiFeatureItem }) {
  let title: string | undefined
  let subtitle: string | undefined
  let items: UiFeatureItem[] = []

  if (Array.isArray(data)) {
    items = data
  } else if (data && typeof data === 'object') {
    if ('title' in data && Array.isArray((data as UiFeatureData).items)) {
      title = (data as UiFeatureData).title
      subtitle = (data as UiFeatureData).subtitle
      items = (data as UiFeatureData).items || []
    } else if ('title' in data) {
      items = [data as UiFeatureItem]
    }
  }

  if (items.length === 0) return null

  return (
    <div className="chat-ui-feature-container">
      {(title || subtitle) && (
        <div className="chat-ui-feature-header">
          {title && <h4 className="chat-ui-feature-header-title">{title}</h4>}
          {subtitle && <p className="chat-ui-feature-header-subtitle">{subtitle}</p>}
        </div>
      )}

      {items.map((item, idx) => {
        const badgeTheme = BADGE_THEMES[item.badgeColor || 'green'] || BADGE_THEMES.green
        const IconComp = resolveIcon(item.icon)
        const resolvedThumb = item.thumbnail ? resolveMediaUrl(item.thumbnail) : null

        let normalizedDetails: Array<{ label: string; value: string }> = []
        if (item.details) {
          if (Array.isArray(item.details)) {
            normalizedDetails = item.details
          } else if (typeof item.details === 'object') {
            normalizedDetails = Object.entries(item.details).map(([label, value]) => ({
              label,
              value: String(value),
            }))
          }
        }

        return (
          <div key={idx} className="chat-ui-feature-item">
            <div className="chat-ui-feature-inner">
              {/* Thumbnail or Left Icon */}
              {resolvedThumb ? (
                <div className="chat-ui-feature-thumb">
                  <img
                    src={resolvedThumb}
                    alt={item.title}
                    onError={(e) => {
                      ;(e.currentTarget as HTMLImageElement).style.display = 'none'
                    }}
                  />
                </div>
              ) : (
                <div className="chat-ui-feature-icon-box">
                  <IconComp size={20} strokeWidth={2.2} />
                </div>
              )}

              {/* Body Content */}
              <div className="chat-ui-feature-body">
                <div className="chat-ui-feature-top-row">
                  <span className="chat-ui-feature-title">{item.title}</span>
                  {item.badge && (
                    <span
                      className="chat-ui-feature-badge"
                      style={{
                        background: badgeTheme.bg,
                        color: badgeTheme.color,
                        border: `1px solid ${badgeTheme.border}`,
                      }}
                    >
                      {item.badge}
                    </span>
                  )}
                  {item.link && (
                    <a
                      href={item.link}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="chat-ui-feature-link"
                    >
                      <ExternalLink size={14} />
                    </a>
                  )}
                </div>

                {item.subtitle && (
                  <div className="chat-ui-feature-subtitle">{item.subtitle}</div>
                )}

                {/* Key-Value Details */}
                {normalizedDetails.length > 0 && (
                  <div className="chat-ui-feature-details">
                    {normalizedDetails.map((detail, dIdx) => (
                      <div key={dIdx} className="chat-ui-feature-detail-row">
                        <span className="chat-ui-feature-detail-label">
                          {detail.label}:
                        </span>
                        <span className="chat-ui-feature-detail-value">
                          {detail.value}
                        </span>
                      </div>
                    ))}
                  </div>
                )}

                {/* Tags */}
                {item.tags && item.tags.length > 0 && (
                  <div className="chat-ui-feature-tags">
                    {item.tags.map((tag, tIdx) => (
                      <span key={tIdx} className="chat-ui-feature-tag">
                        #{tag}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>
        )
      })}
    </div>
  )
}
