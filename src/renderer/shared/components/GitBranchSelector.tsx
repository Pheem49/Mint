import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from 'react'
import { workspacePlatform } from '../platform'
import type { GitBranchChangeOutcome, GitBranchInfo } from '../types'

type MenuView = 'branches' | 'create' | 'graph'
type MenuDirection = 'up' | 'down'

interface GitBranchSelectorProps {
  workspacePath: string
  disabled?: boolean
  onBranchChanged?: () => void
}

function errorMessage(reason: unknown): string {
  if (reason instanceof Error) return reason.message
  if (typeof reason === 'string') return reason
  return 'Git could not complete that action.'
}

export default function GitBranchSelector({ workspacePath, disabled = false, onBranchChanged }: GitBranchSelectorProps) {
  const [info, setInfo] = useState<GitBranchInfo | null>(null)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [error, setError] = useState('')
  const [switchingBranch, setSwitchingBranch] = useState('')
  const [menuView, setMenuView] = useState<MenuView>('branches')
  const [newBranchName, setNewBranchName] = useState('')
  const [graphLines, setGraphLines] = useState<string[]>([])
  const [graphLoading, setGraphLoading] = useState(false)
  const [menuDirection, setMenuDirection] = useState<MenuDirection>('up')
  const [menuAlignment, setMenuAlignment] = useState<'left' | 'right'>('left')
  const [menuMaxHeight, setMenuMaxHeight] = useState(480)
  const rootRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const wasDisabledRef = useRef(disabled)

  const refresh = useCallback(async () => {
    if (!workspacePath.trim()) {
      setInfo(null)
      return
    }
    try {
      const nextInfo = await workspacePlatform.getGitBranchInfo(workspacePath)
      setInfo(nextInfo)
      setError('')
    } catch (reason) {
      setInfo(null)
      setError(errorMessage(reason))
    }
  }, [workspacePath])

  useEffect(() => {
    let cancelled = false
    if (!workspacePath.trim()) {
      setInfo(null)
      setOpen(false)
      return
    }
    workspacePlatform.getGitBranchInfo(workspacePath)
      .then((nextInfo) => {
        if (cancelled) return
        setInfo(nextInfo)
        setError('')
      })
      .catch((reason) => {
        if (cancelled) return
        setInfo(null)
        setError(errorMessage(reason))
      })
    return () => {
      cancelled = true
    }
  }, [workspacePath])

  useEffect(() => {
    if (!open) return
    const closeOnOutsidePointer = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      setOpen(false)
      triggerRef.current?.focus()
    }
    document.addEventListener('mousedown', closeOnOutsidePointer)
    window.addEventListener('keydown', closeOnEscape)
    return () => {
      document.removeEventListener('mousedown', closeOnOutsidePointer)
      window.removeEventListener('keydown', closeOnEscape)
    }
  }, [open])

  useEffect(() => {
    const handleWindowFocus = () => refresh()
    window.addEventListener('focus', handleWindowFocus)
    return () => window.removeEventListener('focus', handleWindowFocus)
  }, [refresh])

  useEffect(() => {
    if (disabled) {
      setOpen(false)
    } else if (wasDisabledRef.current) {
      refresh()
    }
    wasDisabledRef.current = disabled
  }, [disabled, refresh])

  const visibleBranches = useMemo(() => {
    if (!info) return []
    const normalizedQuery = query.trim().toLocaleLowerCase()
    const matching = normalizedQuery
      ? info.branches.filter((branch) => branch.toLocaleLowerCase().includes(normalizedQuery))
      : info.branches
    return [...matching].sort((left, right) => {
      if (left === info.currentBranch) return -1
      if (right === info.currentBranch) return 1
      return left.localeCompare(right)
    })
  }, [info, query])

  const visibleRemoteBranches = useMemo(() => {
    if (!info) return []
    const normalizedQuery = query.trim().toLocaleLowerCase()
    return info.remoteBranches.filter((remoteBranch) => {
      const localName = remoteBranch.split('/').slice(1).join('/')
      if (info.branches.includes(localName)) return false
      return !normalizedQuery || remoteBranch.toLocaleLowerCase().includes(normalizedQuery)
    })
  }, [info, query])

  if (!info?.isRepository) {
    if (!error) return null
    return (
      <div className="git-branch-selector is-error">
        <button
          type="button"
          className="git-branch-trigger"
          onClick={refresh}
          title={`${error} Select to retry.`}
          aria-label="Git branch unavailable. Retry"
        >
          <span className="git-branch-name">Git unavailable</span>
        </button>
      </div>
    )
  }

  const branchLabel = info.currentBranch || (info.detachedHead ? `Detached ${info.detachedHead}` : 'No branch')

  const applyBranchResult = (nextInfo: GitBranchInfo) => {
    setInfo(nextInfo)
    setOpen(false)
    setQuery('')
    setMenuView('branches')
  }

  const executeBranchChange = async (
    description: string,
    change: (confirmedDirtyWorkspace: boolean) => Promise<GitBranchChangeOutcome>,
  ) => {
    let outcome = await change(false)
    if (outcome.status === 'confirmation_required') {
      const confirmed = window.confirm(
        `This workspace has uncommitted changes. ${description} Git will stop if the changes conflict.`,
      )
      if (!confirmed) return
      outcome = await change(true)
    }
    if (outcome.status === 'confirmation_required') {
      throw new Error('Workspace confirmation was not accepted.')
    }
    applyBranchResult(outcome.info)
    onBranchChanged?.()
  }

  const handleSwitch = async (branch: string) => {
    if (branch === info.currentBranch || switchingBranch) {
      setOpen(false)
      return
    }
    try {
      setError('')
      setSwitchingBranch(branch)
      await executeBranchChange(
        `Switch from ${branchLabel} to ${branch}?`,
        (confirmed) => workspacePlatform.switchGitBranch(workspacePath, branch, confirmed),
      )
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSwitchingBranch('')
    }
  }

  const handleRemoteCheckout = async (remoteBranch: string) => {
    if (switchingBranch) return
    try {
      setError('')
      setSwitchingBranch(remoteBranch)
      await executeBranchChange(
        `Check out ${remoteBranch}?`,
        (confirmed) => workspacePlatform.checkoutRemoteGitBranch(workspacePath, remoteBranch, confirmed),
      )
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSwitchingBranch('')
    }
  }

  const handleCreateBranch = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const branch = newBranchName.trim()
    if (!branch || switchingBranch) return
    try {
      setError('')
      setSwitchingBranch(branch)
      await executeBranchChange(
        `Create and switch to ${branch}?`,
        (confirmed) => workspacePlatform.createGitBranch(workspacePath, branch, confirmed),
      )
      setNewBranchName('')
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setSwitchingBranch('')
    }
  }

  const loadGraph = async () => {
    try {
      setError('')
      setGraphLoading(true)
      setGraphLines(await workspacePlatform.getGitGraph(workspacePath))
    } catch (reason) {
      setGraphLines([])
      setError(errorMessage(reason))
    } finally {
      setGraphLoading(false)
    }
  }

  const positionMenu = (preferredHeight: number, preferredWidth = 320) => {
    const triggerRect = triggerRef.current?.getBoundingClientRect()
    if (!triggerRect) return
    const viewportPadding = 16
    const panelRect = rootRef.current?.closest('.conversation-panel')?.getBoundingClientRect()
    const leftEdge = Math.max(viewportPadding, panelRect?.left ?? viewportPadding)
    const rightEdge = Math.min(window.innerWidth - viewportPadding, panelRect?.right ?? window.innerWidth - viewportPadding)
    const availableRight = Math.max(0, rightEdge - triggerRect.left)
    const availableLeft = Math.max(0, triggerRect.right - leftEdge)
    const menuWidth = Math.min(preferredWidth, window.innerWidth - viewportPadding * 2)
    setMenuAlignment(availableRight >= menuWidth || availableRight >= availableLeft ? 'left' : 'right')
    const availableAbove = Math.max(0, triggerRect.top - viewportPadding)
    const availableBelow = Math.max(0, window.innerHeight - triggerRect.bottom - viewportPadding)
    const direction: MenuDirection = availableAbove >= preferredHeight || availableAbove >= availableBelow
      ? 'up'
      : 'down'
    setMenuDirection(direction)
    setMenuMaxHeight(Math.max(180, direction === 'up' ? availableAbove : availableBelow))
  }

  const openGraph = () => {
    positionMenu(420, window.innerWidth <= 1100 ? 320 : 540)
    setMenuView('graph')
    loadGraph()
  }

  return (
    <div className="git-branch-selector" ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        className="git-branch-trigger"
        onClick={() => {
          setOpen((current) => {
            if (!current) {
              positionMenu(360)
            }
            return !current
          })
          setQuery('')
          setError('')
          setMenuView('branches')
        }}
        disabled={disabled || Boolean(switchingBranch)}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls="workspace-branch-menu"
        title={`${branchLabel}${info.isDirty ? ' — uncommitted changes' : ''}`}
      >
        <svg className="git-branch-icon" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <circle cx="6" cy="5" r="2" />
          <circle cx="18" cy="6" r="2" />
          <circle cx="6" cy="19" r="2" />
          <path d="M6 7v10M8 7c2 4 8 1 8-1" />
        </svg>
        <span className="git-branch-name">{switchingBranch || branchLabel}</span>
        {info.isDirty && <span className="git-branch-dirty" aria-label="Uncommitted changes" />}
        <svg className="git-branch-chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      {open && (
        <div
          className={`git-branch-menu ${menuView === 'graph' ? 'is-graph' : ''} ${menuDirection === 'down' ? 'opens-down' : ''} ${menuAlignment === 'right' ? 'align-right' : ''}`}
          id="workspace-branch-menu"
          role="dialog"
          style={{ maxHeight: `${menuMaxHeight}px` }}
          aria-label={menuView === 'graph' ? 'Git graph' : menuView === 'create' ? 'Create branch' : 'Choose branch'}
        >
          {menuView === 'branches' && (
            <>
              <label className="git-branch-search">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
                  <circle cx="11" cy="11" r="7" />
                  <path d="m20 20-4-4" />
                </svg>
                <input
                  autoFocus
                  aria-label="Search branches"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="Search branches"
                />
              </label>
              <div className="git-branch-menu-heading">Local branches</div>
              <div className="git-branch-list" role="listbox" aria-label="Local branches">
                {visibleBranches.map((branch) => {
                  const selected = branch === info.currentBranch
                  return (
                    <button
                      key={branch}
                      type="button"
                      className="git-branch-option"
                      role="option"
                      aria-selected={selected}
                      disabled={Boolean(switchingBranch)}
                      onClick={() => handleSwitch(branch)}
                    >
                      <span className="git-branch-option-icon" aria-hidden="true">⑂</span>
                      <span>{branch}</span>
                      {selected && (
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                          <path d="m5 12 4 4L19 6" />
                        </svg>
                      )}
                    </button>
                  )
                })}
              </div>
              {visibleRemoteBranches.length > 0 && (
                <>
                  <div className="git-branch-menu-heading">Remote branches</div>
                  <div className="git-branch-list is-remote" role="listbox" aria-label="Remote branches">
                    {visibleRemoteBranches.map((remoteBranch) => (
                      <button
                        key={remoteBranch}
                        type="button"
                        className="git-branch-option"
                        role="option"
                        aria-selected="false"
                        disabled={Boolean(switchingBranch)}
                        onClick={() => handleRemoteCheckout(remoteBranch)}
                        title={`Create a local tracking branch from ${remoteBranch}`}
                      >
                        <span className="git-branch-option-icon" aria-hidden="true">⇄</span>
                        <span>{remoteBranch}</span>
                        <span className="git-branch-track-label">Track</span>
                      </button>
                    ))}
                  </div>
                </>
              )}
              {visibleBranches.length === 0 && visibleRemoteBranches.length === 0 && (
                <div className="git-branch-empty">No matching branches</div>
              )}
              <div className="git-branch-actions">
                <button type="button" onClick={() => { setMenuView('create'); setError('') }}>
                  <span aria-hidden="true">＋</span>
                  Create branch
                </button>
                <button type="button" onClick={openGraph}>
                  <span aria-hidden="true">⑂</span>
                  Git graph
                </button>
              </div>
            </>
          )}

          {menuView === 'create' && (
            <form className="git-branch-create" onSubmit={handleCreateBranch}>
              <div className="git-branch-view-header">
                <button type="button" onClick={() => { setMenuView('branches'); setError('') }} aria-label="Back to branches">←</button>
                <strong>Create branch</strong>
              </div>
              <label htmlFor="new-git-branch-name">Branch name</label>
              <input
                id="new-git-branch-name"
                autoFocus
                value={newBranchName}
                onChange={(event) => setNewBranchName(event.target.value)}
                placeholder="feature/branch-name"
                spellCheck="false"
                autoComplete="off"
              />
              <p>Creates the branch from <strong>{branchLabel}</strong> and switches to it.</p>
              <button className="git-branch-create-submit" type="submit" disabled={!newBranchName.trim() || Boolean(switchingBranch)}>
                {switchingBranch ? 'Creating…' : 'Create and switch'}
              </button>
            </form>
          )}

          {menuView === 'graph' && (
            <div className="git-graph-view">
              <div className="git-branch-view-header">
                <button type="button" onClick={() => { setMenuView('branches'); setError('') }} aria-label="Back to branches">←</button>
                <strong>Git graph</strong>
                <button type="button" className="git-graph-refresh" onClick={loadGraph} disabled={graphLoading}>Refresh</button>
              </div>
              {graphLoading ? (
                <div className="git-graph-state">Reading repository history…</div>
              ) : error ? null : graphLines.length > 0 ? (
                <pre className="git-graph-output" tabIndex={0}>{graphLines.join('\n')}</pre>
              ) : (
                <div className="git-graph-state">No commits yet</div>
              )}
            </div>
          )}

          {error && (
            <div className="git-branch-error" role="alert">
              <span>{error}</span>
              {menuView === 'graph' && <button type="button" onClick={loadGraph}>Retry</button>}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
