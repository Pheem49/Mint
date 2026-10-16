import React, { createContext, useContext, useEffect, useId, useState } from 'react'
import { Eye, EyeOff, LoaderCircle } from 'lucide-react'
import type { AuthUser } from '../types'
import { authPlatform, runtimePlatform } from '../platform'
import '../css/auth-gate.css'

const { authGetCurrentUser, authLogin, authLogout, authRegister, resolveAvatarUrl } = authPlatform

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
    const fallbackTimer = setTimeout(() => {
      if (!cancelled) {
        setStatus('signed-out')
      }
    }, 6000)

    authGetCurrentUser()
      .then((current) => {
        if (cancelled) return
        clearTimeout(fallbackTimer)
        setUser(current)
        setStatus(current ? 'signed-in' : 'signed-out')
      })
      .catch(() => {
        if (!cancelled) {
          clearTimeout(fallbackTimer)
          setStatus('signed-out')
        }
      })
    return () => {
      cancelled = true
      clearTimeout(fallbackTimer)
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
  const [showPassword, setShowPassword] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const passwordId = useId()
  const errorId = useId()

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (loading) return
    setLoading(true)
    setError(null)
    try {
      const user =
        mode === 'login'
          ? await authLogin(email, password)
          : await authRegister(name || undefined, email, password)
      onSuccess(user)
    } catch (err) {
      setError(err instanceof Error && err.message.trim() ? err.message : 'Unable to continue. Please try again.')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="auth-gate-overlay">
      <form className="auth-gate-card" onSubmit={handleSubmit}>
        <img src={runtimePlatform.appIconPath()} alt="" className="auth-gate-logo" />
        <div className="auth-gate-heading">
          <h1 className="auth-gate-title">
            {mode === 'login' ? 'Sign in to Mint' : 'Create your Mint account'}
          </h1>
          <p className="auth-gate-subtitle">
            {mode === 'login' ? 'Sign in to continue to Mint' : 'Create an account to get started with Mint'}
          </p>
        </div>

        {mode === 'register' && (
          <label className="auth-gate-field">
            <span>Name</span>
            <input
              type="text"
              autoComplete="name"
              disabled={loading}
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
            autoComplete="username"
            disabled={loading}
            aria-describedby={error ? errorId : undefined}
            required
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="you@example.com"
          />
        </label>

        <div className="auth-gate-field">
          <label htmlFor={passwordId}>Password</label>
          <div className="auth-gate-password">
            <input
              id={passwordId}
              type={showPassword ? 'text' : 'password'}
              autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              disabled={loading}
              aria-describedby={error ? errorId : undefined}
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••"
            />
            <button
              type="button"
              className="auth-gate-password-toggle"
              aria-label={showPassword ? 'Hide password' : 'Show password'}
              aria-controls={passwordId}
              disabled={loading}
              onClick={() => setShowPassword((visible) => !visible)}
            >
              {showPassword ? <EyeOff size={18} aria-hidden="true" /> : <Eye size={18} aria-hidden="true" />}
            </button>
          </div>
        </div>

        {error && <div id={errorId} className="auth-gate-error" role="alert">{error}</div>}

        <button type="submit" className="auth-gate-submit" disabled={loading} aria-busy={loading}>
          {loading && <LoaderCircle size={18} className="auth-gate-spinner" aria-hidden="true" />}
          <span role="status">
            {loading
              ? mode === 'login'
                ? 'Signing in…'
                : 'Creating account…'
              : mode === 'login'
                ? 'Sign in'
                : 'Create account'}
          </span>
        </button>

        <div className="auth-gate-account-prompt">
          <span>{mode === 'login' ? "Don't have an account?" : 'Already have an account?'}</span>
          <button
            type="button"
            className="auth-gate-switch"
            disabled={loading}
            onClick={() => {
              setMode(mode === 'login' ? 'register' : 'login')
              setShowPassword(false)
              setError(null)
            }}
          >
            {mode === 'login' ? 'Create account' : 'Sign in'}
          </button>
        </div>
      </form>
    </div>
  )
}
