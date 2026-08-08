import React, { createContext, useContext, useEffect, useState } from 'react'
import type { AuthUser } from '../types'
import {
  APP_ICON_PATH,
  authGetCurrentUser,
  authLogin,
  authLogout,
  authRegister,
  resolveAvatarUrl,
} from '@/tauri'
import '../css/auth-gate.css'

interface AuthContextValue {
  user: AuthUser | null
  avatarUrl: string | null
  logout: () => Promise<void>
  /** Update the in-memory user (e.g. after saving profile changes) without
   * a full page reload or extra round-trip to re-fetch the session. */
  refreshUser: (updated: AuthUser) => void
}

const AuthUserContext = createContext<AuthContextValue>({
  user: null,
  avatarUrl: null,
  logout: async () => {},
  refreshUser: () => {},
})

/** Read the currently signed-in shared Mint user (and a logout action)
 * from anywhere under <AuthGate>, e.g. the sidebar account card. */
export function useAuthUser(): AuthContextValue {
  return useContext(AuthUserContext)
}

export default function AuthGate({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<'loading' | 'signed-out' | 'signed-in'>('loading')
  const [user, setUser] = useState<AuthUser | null>(null)

  useEffect(() => {
    let cancelled = false
    authGetCurrentUser()
      .then((current) => {
        if (cancelled) return
        setUser(current)
        setStatus(current ? 'signed-in' : 'signed-out')
      })
      .catch(() => {
        if (!cancelled) setStatus('signed-out')
      })
    return () => {
      cancelled = true
    }
  }, [])

  const handleLogout = async () => {
    await authLogout().catch(() => {})
    setUser(null)
    setStatus('signed-out')
  }

  if (status === 'loading') {
    return <div className="auth-gate-loading">Loading Mint…</div>
  }

  if (status === 'signed-out') {
    return (
      <AuthForm
        onSuccess={(loggedInUser) => {
          setUser(loggedInUser)
          setStatus('signed-in')
        }}
      />
    )
  }

  return (
    <AuthUserContext.Provider
      value={{
        user,
        avatarUrl: resolveAvatarUrl(user?.image),
        logout: handleLogout,
        refreshUser: setUser,
      }}
    >
      {children}
    </AuthUserContext.Provider>
  )
}

function AuthForm({ onSuccess }: { onSuccess: (user: AuthUser) => void }) {
  const [mode, setMode] = useState<'login' | 'register'>('login')
  const [name, setName] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setLoading(true)
    setError(null)
    try {
      const user =
        mode === 'login'
          ? await authLogin(email, password)
          : await authRegister(name || undefined, email, password)
      onSuccess(user)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Something went wrong.')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="auth-gate-overlay">
      <form className="auth-gate-card" onSubmit={handleSubmit}>
        <img src={APP_ICON_PATH} alt="" className="auth-gate-logo" />
        <h1 className="auth-gate-title">
          {mode === 'login' ? 'Sign in to Mint' : 'Create your Mint account'}
        </h1>
        <p className="auth-gate-subtitle">
          Uses the same account as Mint search — sign in once, use everywhere.
        </p>

        {mode === 'register' && (
          <label className="auth-gate-field">
            <span>Name</span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Your name"
            />
          </label>
        )}

        <label className="auth-gate-field">
          <span>Email</span>
          <input
            type="email"
            required
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="you@example.com"
          />
        </label>

        <label className="auth-gate-field">
          <span>Password</span>
          <input
            type="password"
            required
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="••••••••"
          />
        </label>

        {error && <div className="auth-gate-error">{error}</div>}

        <button type="submit" className="auth-gate-submit" disabled={loading}>
          {loading
            ? mode === 'login'
              ? 'Signing in…'
              : 'Creating account…'
            : mode === 'login'
              ? 'Sign in'
              : 'Create account'}
        </button>

        <button
          type="button"
          className="auth-gate-switch"
          onClick={() => {
            setMode(mode === 'login' ? 'register' : 'login')
            setError(null)
          }}
        >
          {mode === 'login' ? "Don't have an account? Create one" : 'Already have an account? Sign in'}
        </button>
      </form>
    </div>
  )
}
