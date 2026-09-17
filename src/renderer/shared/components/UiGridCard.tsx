import React from 'react'
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
  package: Box,
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
  link: ExternalLink,
}

export interface UiGridItem {
  icon?: string
  title: string
  desc?: string
  badge?: string
  link?: string
}

export interface UiGridData {
  title?: string
  subtitle?: string
  columns?: number
  items?: UiGridItem[]
}

function resolveIcon(name?: string): LucideIcon {
  if (!name) return Sparkles
  const clean = name.toLowerCase().trim().replace(/_/g, '-')
  return ICON_MAP[clean] || Sparkles
}

export default function UiGridCard({ data }: { data: UiGridData | UiGridItem[] }) {
  let title: string | undefined
  let subtitle: string | undefined
  let items: UiGridItem[] = []
  let columns: number | undefined

  if (Array.isArray(data)) {
    items = data
  } else if (data && typeof data === 'object') {
    title = data.title
    subtitle = data.subtitle
    columns = data.columns
    items = Array.isArray(data.items) ? data.items : []
  }

  if (items.length === 0) return null

  const gridCols = columns ? `repeat(${columns}, 1fr)` : 'repeat(auto-fit, minmax(200px, 1fr))'

  return (
    <div className="chat-ui-grid-wrapper">
      {(title || subtitle) && (
        <div className="chat-ui-grid-header">
          {title && <h4 className="chat-ui-grid-header-title">{title}</h4>}
          {subtitle && <p className="chat-ui-grid-header-subtitle">{subtitle}</p>}
        </div>
      )}

      <div className="chat-ui-grid-grid" style={{ gridTemplateColumns: gridCols }}>
        {items.map((item, idx) => {
          const IconComp = resolveIcon(item.icon)
          return (
            <div key={idx} className="chat-ui-grid-item">
              <div className="chat-ui-grid-item-top">
                <div className="chat-ui-grid-item-icon">
                  <IconComp size={18} strokeWidth={2.2} />
                </div>
                {item.badge && (
                  <span
                    style={{
                      fontSize: '0.7rem',
                      fontWeight: 600,
                      padding: '2px 7px',
                      borderRadius: '999px',
                      background: 'rgba(16, 185, 129, 0.15)',
                      color: '#34d399',
                      border: '1px solid rgba(16, 185, 129, 0.3)',
                    }}
                  >
                    {item.badge}
                  </span>
                )}
              </div>

              <div className="chat-ui-grid-item-title">{item.title}</div>

              {item.desc && <div className="chat-ui-grid-item-desc">{item.desc}</div>}
            </div>
          )
        })}
      </div>
    </div>
  )
}
