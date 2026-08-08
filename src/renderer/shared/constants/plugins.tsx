import React from 'react'

export function renderMcpSvgIcon(name: string, customIcon?: string) {
  if (customIcon) {
    if (customIcon.trim().startsWith('<svg')) {
      return <div dangerouslySetInnerHTML={{ __html: customIcon }} style={{ width: '22px', height: '22px', display: 'flex', alignItems: 'center', justifyContent: 'center' }} />
    }
    if (customIcon.startsWith('http://') || customIcon.startsWith('https://') || customIcon.startsWith('data:image')) {
      return <img src={customIcon} alt={name} style={{ width: '22px', height: '22px', objectFit: 'contain' }} />
    }
    const cleanCustom = customIcon.toLowerCase().trim()
    if (cleanCustom === 'search') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#3b82f6" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
        </svg>
      )
    }
    if (cleanCustom === 'database') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#a855f7" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <ellipse cx="12" cy="5" rx="9" ry="3"/>
          <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
          <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
        </svg>
      )
    }
    if (cleanCustom === 'cloud') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#38bdf8" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z"/>
        </svg>
      )
    }
    if (cleanCustom === 'code' || cleanCustom === 'terminal') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#10b981" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="16 18 22 12 16 6" />
          <polyline points="8 6 2 12 8 18" />
        </svg>
      )
    }
    if (cleanCustom === 'api' || cleanCustom === 'bolt') {
      return (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
        </svg>
      )
    }
  }

  const cleanName = name.toLowerCase().trim()
  if (cleanName === 'docker') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#0db7ed">
        <path d="M13 8.5h2v2h-2zm-3 0h2v2h-2zm-3 0h2v2H7zm-3 3h2v2H4zm3 0h2v2H7zm3 0h2v2h-2zm3 0h2v2h-2zm3 0h2v2h-2zm-12 3h2v2H4zm3 0h2v2H7zm3 0h2v2h-2zm3 0h2v2h-2zm3 0h2v2h-2zm-6.2 3.8c-.8.5-2.1.8-3.4.6-2.5-.3-4.5-2.2-4.8-4.7h-1c.4 3.4 3.1 6 6.5 6 3 0 5.6-2 6.4-4.8.6-.2 1.4-.4 2.2-.2.3.1.6.3.8.5.6.6 1.4.9 2.2.9h.4c.5-.7.8-1.5.8-2.4 0-.3 0-.6-.1-.9-.7.1-1.4 0-2.1-.3-.6-.3-1.1-.8-1.5-1.4l-.2-.3h-2.1c-.2.7-.6 1.3-1.2 1.7-.6.4-1.3.6-2 .4z"/>
      </svg>
    )
  }
  if (cleanName === 'git' || cleanName === 'gitkraken') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#f05032" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="18" cy="18" r="3" />
        <circle cx="6" cy="6" r="3" />
        <circle cx="6" cy="18" r="3" />
        <path d="M6 9v6" />
        <path d="M9 18h6" />
      </svg>
    )
  }
  if (cleanName === 'github') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#f0f6fc">
        <path fillRule="evenodd" clipRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"/>
      </svg>
    )
  }
  if (cleanName === 'node' || cleanName === 'nodejs') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#68a063">
        <path d="M12 2L2 7.5v9L12 22l10-5.5v-9L12 2zm0 2.3l7.7 4.2v7L12 19.7 4.3 15.5v-7L12 4.3zm-1 4.2v3.5l-3-1.7V8.5l3 1.7zm2 0l3-1.7v1.8l-3 1.7V8.5z"/>
      </svg>
    )
  }
  if (cleanName === 'gmail') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24">
        <path fill="#EA4335" d="M20 4H4C2.9 4 2 4.9 2 6v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2z" fillOpacity="0.1"/>
        <path fill="#EA4335" d="M20 5H4c-.6 0-1.1.3-1.5.7l9.5 7.1 9.5-7.1c-.4-.4-.9-.7-1.5-.7z"/>
        <path fill="#34A853" d="M2 7v11c0 .6.4 1 1 1h2V9.3L2 7z"/>
        <path fill="#4285F4" d="M22 7l-3 2.3V19h2c.6 0 1-.4 1-1V7z"/>
        <path fill="#FBBC05" d="M5 19h14v-9.7l-7 5.3-7-5.3V19z"/>
      </svg>
    )
  }
  if (cleanName === 'calendar' || cleanName === 'google_calendar') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24">
        <rect x="3" y="3" width="18" height="18" rx="3" fill="#ffffff" />
        <path d="M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2z" fill="#4285F4"/>
        <path d="M19 3H5a2 2 0 0 0-2 2v3h18V5a2 2 0 0 0-2-2z" fill="#185ABC"/>
        <path d="M7 2v3M17 2v3" stroke="#ffffff" strokeWidth="1.5" strokeLinecap="round"/>
        <text x="12" y="17" fontSize="8.5" fontWeight="bold" fill="#ffffff" textAnchor="middle" fontFamily="system-ui, -apple-system, sans-serif">31</text>
      </svg>
    )
  }
  if (cleanName === 'notion') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#ffffff">
        <path d="M4.459 4.208c.746.606 1.026.56 2.424.466l11.424-.701c.28 0 .093-.28-.047-.326L15.69 2.155c-.42-.326-.98-.42-1.54-.373L3.712 2.808c-.42.046-.7.326-.7.7 0 .093.047.233.094.28l1.353 1.42zm.84 3.733v12.775c0 .7.373 1.073 1.12 1.026l12.774-.746c.746-.047.886-.606.886-1.12V7.1c0-.513-.233-.746-.746-.7l-13.24.793c-.56.047-.794.28-.794.746zm11.794 1.306c.093.373 0 .746-.373.793l-1.026.14v8.583c-.42.233-.886.373-1.306.373-.513 0-.84-.14-1.26-.653l-4.153-6.53v6.067l1.353.326c.093.047.14.28.14.42 0 .28-.233.373-.7.373l-2.846.14c-.093 0-.233-.093-.233-.373 0-.233.14-.373.28-.42l1.026-.14V9.95l-1.026-.14c-.093-.047-.186-.28-.186-.42 0-.28.233-.373.746-.373l2.8.047c.56 0 .98.14 1.353.746l4.06 6.344V10.23l-1.167-.14c-.093-.047-.14-.28-.14-.42 0-.28.233-.373.746-.373l2.707-.093c.14 0 .28.093.28.42z"/>
      </svg>
    )
  }
  if (cleanName === 'youtube_music' || cleanName === 'yt_music') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="10" fill="#FF0000"/>
        <circle cx="12" cy="12" r="5.5" fill="none" stroke="#FFFFFF" strokeWidth="1.6"/>
        <polygon points="10.5 8.5 15.5 12 10.5 15.5" fill="#FFFFFF"/>
      </svg>
    )
  }
  if (cleanName === 'vercel') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#ffffff">
        <path d="M12 1L24 22H0L12 1Z"/>
      </svg>
    )
  }
  if (cleanName === 'spotify') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#1ed760">
        <path d="M12 2C6.477 2 2 6.477 2 12s4.477 10 10 10 10-4.477 10-10S17.523 2 12 2zm4.586 14.424c-.18.295-.563.387-.857.207-2.35-1.434-5.308-1.758-8.793-.963-.335.077-.67-.133-.746-.467-.077-.334.132-.67.467-.746 3.812-.871 7.102-.494 9.722 1.112.294.18.386.563.207.857zm1.233-2.743c-.226.367-.706.482-1.073.257-2.687-1.652-6.785-2.131-9.965-1.166-.413.126-.847-.106-.973-.519-.125-.413.106-.847.519-.973 3.632-1.102 8.147-.568 11.235 1.328.367.226.482.706.257 1.073zm.105-2.835C14.692 8.95 9.375 8.775 6.297 9.71c-.496.15-1.022-.132-1.173-.628-.15-.496.132-1.022.628-1.173 3.535-1.073 9.404-.871 12.984 1.254.446.265.592.844.327 1.29-.265.446-.844.592-1.29.327z"/>
      </svg>
    )
  }
  if (cleanName === 'discord') {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="#5865f2">
        <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994.021-.041.001-.09-.041-.106a13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.061 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.893.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.028zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
      </svg>
    )
  }
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#10b981" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2" width="20" height="8" rx="2" ry="2"/>
      <rect x="2" y="14" width="20" height="8" rx="2" ry="2"/>
      <line x1="6" y1="6" x2="6.01" y2="6"/>
      <line x1="6" y1="18" x2="6.01" y2="18"/>
    </svg>
  )
}

export interface BuiltinPluginDefinition {
  key: string
  name: string
  desc: string
  icon: string
  enabledField: string
  hasCredentials: boolean
  isOAuth?: boolean
  oauthProvider?: string
  fields?: Array<{ label: string; field: string; type: string; placeholder: string }>
}

export const BUILTIN_PLUGINS_LIST: BuiltinPluginDefinition[] = [
  {
    key: 'spotify',
    name: 'Spotify',
    desc: 'Control playback with AI. Requires playerctl locally or Spotify OAuth.',
    icon: '🎵',
    enabledField: 'pluginSpotifyEnabled',
    hasCredentials: true,
    isOAuth: true,
    oauthProvider: 'spotify',
    fields: [
      { label: 'Spotify Client ID', field: 'spotifyClientId', type: 'text', placeholder: 'Enter Spotify Client ID...' },
      { label: 'Spotify Client Secret', field: 'spotifyClientSecret', type: 'password', placeholder: 'Enter Spotify Client Secret...' },
    ]
  },
  {
    key: 'discord',
    name: 'Discord RPC',
    desc: 'Show "Using Mint Assistant" status in your local Discord client.',
    icon: '💬',
    enabledField: 'pluginDiscordEnabled',
    hasCredentials: false
  },
  {
    key: 'gmail',
    name: 'Gmail',
    desc: 'Read, summarize, draft, and send emails via Google OAuth / API credentials.',
    icon: '📧',
    enabledField: 'pluginGmailEnabled',
    hasCredentials: true,
    isOAuth: true,
    oauthProvider: 'google',
    fields: [
      { label: 'Google Client ID', field: 'gmailClientId', type: 'text', placeholder: 'your_client_id.apps.googleusercontent.com' },
      { label: 'Google Client Secret', field: 'gmailClientSecret', type: 'password', placeholder: 'your_client_secret' },
      { label: 'Gmail Refresh Token', field: 'gmailRefreshToken', type: 'password', placeholder: 'your_refresh_token' },
    ]
  },
  {
    key: 'calendar',
    name: 'Google Calendar',
    desc: 'Read schedule, check availability, and create calendar events.',
    icon: '📅',
    enabledField: 'pluginCalendarEnabled',
    hasCredentials: true,
    isOAuth: true,
    oauthProvider: 'google',
    fields: [
      { label: 'Google Client ID', field: 'googleCalendarClientId', type: 'text', placeholder: 'your_client_id.apps.googleusercontent.com' },
      { label: 'Google Client Secret', field: 'googleCalendarClientSecret', type: 'password', placeholder: 'your_client_secret' },
      { label: 'Google Refresh Token', field: 'googleCalendarRefreshToken', type: 'password', placeholder: 'your_refresh_token' },
    ]
  },
  {
    key: 'notion',
    name: 'Notion',
    desc: 'Search pages, query databases, and create notes in Notion.',
    icon: '📝',
    enabledField: 'pluginNotionEnabled',
    hasCredentials: true,
    isOAuth: true,
    oauthProvider: 'notion',
    fields: [
      { label: 'Notion Integration API Key', field: 'notionApiKey', type: 'password', placeholder: 'secret_...' },
      { label: 'Default Database ID', field: 'notionDatabaseId', type: 'text', placeholder: '32-character database ID' },
    ]
  },
  {
    key: 'youtube_music',
    name: 'YouTube Music',
    desc: 'Access personal playlists and listening history via Google OAuth.',
    icon: '🎬',
    enabledField: 'pluginYoutubeMusicEnabled',
    hasCredentials: true,
    isOAuth: true,
    oauthProvider: 'google',
    fields: [
      { label: 'Google Client ID', field: 'gmailClientId', type: 'text', placeholder: 'your_client_id.apps.googleusercontent.com' },
      { label: 'Google Client Secret', field: 'gmailClientSecret', type: 'password', placeholder: 'your_client_secret' },
    ]
  },
  {
    key: 'vercel',
    name: 'Vercel',
    desc: 'Manage web app deployments and projects via Vercel OAuth.',
    icon: '🌐',
    enabledField: 'pluginVercelEnabled',
    hasCredentials: true,
    isOAuth: true,
    oauthProvider: 'vercel',
    fields: [
      { label: 'Vercel Client ID', field: 'vercelClientId', type: 'text', placeholder: 'oai_...' },
      { label: 'Vercel Access Token (Optional)', field: 'vercelToken', type: 'password', placeholder: 'your_vercel_token' },
    ]
  },
  {
    key: 'github',
    name: 'GitHub',
    desc: 'Access repositories, pull requests, and issues via GitHub OAuth.',
    icon: '🐱',
    enabledField: 'pluginGithubEnabled',
    hasCredentials: true,
    isOAuth: true,
    oauthProvider: 'github',
    fields: [
      { label: 'GitHub Client ID', field: 'githubClientId', type: 'text', placeholder: 'Ov23...' },
      { label: 'GitHub Client Secret', field: 'githubClientSecret', type: 'password', placeholder: 'your_github_client_secret' },
      { label: 'GitHub Personal Access Token (Optional)', field: 'githubToken', type: 'password', placeholder: 'ghp_...' },
    ]
  },
]

export function renderSkillsSvgIcon(size = 20, color = 'currentColor') {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M22 10v6M2 10l10-5 10 5-10 5z" />
      <path d="M6 12v5c3 3 9 3 12 0v-5" />
    </svg>
  )
}

export function renderMcpHubSvgIcon(size = 20, color = 'currentColor') {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 2v8M18 10V4M6 10V4" />
      <path d="M4 10h16v3a4 4 0 0 1-4 4h-8a4 4 0 0 1-4-4v-3z" />
      <line x1="12" y1="17" x2="12" y2="22" />
    </svg>
  )
}

export function renderPluginsSvgIcon(size = 20, color = 'currentColor') {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
      <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
      <line x1="12" y1="22.08" x2="12" y2="12" />
    </svg>
  )
}
