/**
 * shared/utils/oauthManager.ts
 * Centralized OAuth 2.0 PKCE & Integration Manager for Desktop UI and Web UI.
 * 
 * Handles popup sign-in initiation, connection polling, revoking tokens, and broadcasting
 * real-time `mint:oauth-changed` events.
 */

import { getLocalApiBase } from '../platform'

export interface OAuthStatus {
  provider: string
  connected: boolean
  accountEmail?: string
  accountName?: string
}

const OAUTH_EVENT_NAME = 'mint:oauth-changed'

export async function getOAuthStatuses(): Promise<OAuthStatus[]> {
  try {
    const API_BASE = getLocalApiBase()
    const res = await fetch(`${API_BASE}/oauth/status`)
    const data = await res.json()
    if (data && Array.isArray(data.statuses)) {
      return data.statuses.map((s: any) => ({
        provider: s.provider,
        connected: Boolean(s.connected),
        accountEmail: s.account_email || undefined,
        accountName: s.account_name || undefined,
      }))
    }
  } catch (e) {
    console.warn('[OAuthManager] Failed to fetch OAuth statuses:', e)
  }
  return []
}

export async function startOAuthFlow(provider: string): Promise<boolean> {
  try {
    const API_BASE = getLocalApiBase()
    const res = await fetch(`${API_BASE}/oauth/start?provider=${encodeURIComponent(provider)}`)
    const data = await res.json()

    if (data.status === 'ok' && data.auth_url) {
      // Open authorization URL in popup or system browser
      const width = 600
      const height = 700
      const left = typeof window !== 'undefined' ? (window.screen.width - width) / 2 : 100
      const top = typeof window !== 'undefined' ? (window.screen.height - height) / 2 : 100

      const popup = window.open(
        data.auth_url,
        `Mint_OAuth_${provider}`,
        `width=${width},height=${height},top=${top},left=${left},scrollbars=yes`
      )

      // Poll for connection status until completed or timed out
      return new Promise<boolean>((resolve) => {
        let attempts = 0
        const interval = setInterval(async () => {
          attempts++
          const statuses = await getOAuthStatuses()
          const matched = statuses.find((s) => s.provider === provider)

          if (matched && matched.connected) {
            clearInterval(interval)
            if (popup && !popup.closed) {
              popup.close()
            }
            broadcastOAuthChange(provider, true)
            resolve(true)
            return
          }

          if (attempts > 60 || (popup && popup.closed)) {
            clearInterval(interval)
            resolve(false)
          }
        }, 2000)
      })
    }
  } catch (e) {
    console.error(`[OAuthManager] Failed to start OAuth flow for ${provider}:`, e)
  }
  return false
}

export async function revokeOAuth(provider: string): Promise<boolean> {
  try {
    const API_BASE = getLocalApiBase()
    const res = await fetch(`${API_BASE}/oauth/revoke`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ provider }),
    })
    const data = await res.json()
    if (data.status === 'ok') {
      broadcastOAuthChange(provider, false)
      return true
    }
  } catch (e) {
    console.error(`[OAuthManager] Failed to revoke OAuth for ${provider}:`, e)
  }
  return false
}

function broadcastOAuthChange(provider: string, connected: boolean) {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(
      new CustomEvent(OAUTH_EVENT_NAME, {
        detail: { provider, connected },
      })
    )
  }
}

export function subscribeOAuthChange(callback: (detail: { provider: string; connected: boolean }) => void): () => void {
  if (typeof window === 'undefined') return () => {}
  const handler = (ev: Event) => {
    const customEv = ev as CustomEvent<{ provider: string; connected: boolean }>
    if (customEv.detail) {
      callback(customEv.detail)
    }
  }
  window.addEventListener(OAUTH_EVENT_NAME, handler)
  return () => {
    window.removeEventListener(OAUTH_EVENT_NAME, handler)
  }
}
