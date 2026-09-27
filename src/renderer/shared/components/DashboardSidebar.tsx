import { useState, useEffect, useRef, useMemo, type MouseEvent as ReactMouseEvent, type DragEvent as ReactDragEvent } from 'react'
import { renderSkillsSvgIcon, renderMcpHubSvgIcon, renderPluginsSvgIcon, renderScheduledTasksSvgIcon, renderLinkedFoldersSvgIcon } from '../constants/plugins'
import { useAuthUser } from './AuthGate'
import { runtimePlatform } from '../platform'

export type DashboardView = 'chat' | 'pictures' | 'model' | 'workspace' | 'imagine' | 'veo' | 'skills' | 'mcp' | 'plugins' | 'cron' | 'link' | 'code'

interface ChatSessionItem {
  id: string
  title: string
  kind?: string
  createdAt?: string
  updatedAt?: string
  workspacePath?: string | null
}

interface DashboardSidebarProps {
  view: DashboardView
  sidebarCollapsed: boolean
  sending: boolean
  chatSessions: ChatSessionItem[]
  activeConversationId: string
  onToggleSidebar: () => void
  /** Live width in px while the user drags the resize handle. */
  onSidebarResize?: (width: number) => void
  /** Fires once on drag release with the final width the user let go at. */
  onSidebarResizeEnd?: (width: number) => void
  onClearHistory: (action: 'New chat' | 'Clear history') => void
  onSelectConversation: (id: string) => void
  onDeleteConversation: (id: string) => void
  onRenameConversation?: (id: string, newTitle: string) => void
  onSetView: (view: DashboardView) => void
  isSearchOpen: boolean
  onSetSearchOpen: (open: boolean) => void
  /** Desktop only — web has no local workspace-folder concept to browse. */
  showWorkspaceTab?: boolean
  /**
   * Web only — web surfaces Image Studio/Veo Studio as top-level sidebar
   * buttons; desktop tucks them inside the "More" popover instead. Real UX
   * difference between the two, not something to unify.
   */
  promoteMediaStudios?: boolean
  /** Desktop only — the inline Live2D companion character doesn't exist on web. */
  modelVisible?: boolean
  expressionIndex?: number
  accessoryIndex?: number
  expressions?: string[]
  accessories?: string[]
  interactionEnabled?: boolean
  showInteractionGuide?: boolean
  onToggleModel?: () => void
  onSetExpressionIndex?: (index: number) => void
  onSetAccessoryIndex?: (index: number) => void
  onSetInteractionEnabled?: (enabled: boolean) => void
  onSetShowInteractionGuide?: (visible: boolean) => void
  onShowToast?: (message: string) => void
  activeWorkspacePath?: string
  recentWorkspacePaths?: string[]
  onBrowseFolder?: () => Promise<string | null>
  onUpdateSessionWorkspace?: (sessionId: string, workspacePath: string | null) => void
  onNewChatInProject?: (workspacePath: string) => void
}

export default function DashboardSidebar({
  view,
  sidebarCollapsed,
  modelVisible,
  sending,
  chatSessions,
  activeConversationId,
  expressionIndex = 0,
  accessoryIndex = 0,
  expressions = [],
  accessories = [],
  interactionEnabled,
  showInteractionGuide,
  onToggleSidebar,
  onSidebarResize,
  onSidebarResizeEnd,
  onClearHistory,
  onSelectConversation,
  onDeleteConversation,
  onRenameConversation,
  onSetView,
  onToggleModel,
  onSetExpressionIndex,
  onSetAccessoryIndex,
  onSetInteractionEnabled,
  onSetShowInteractionGuide,
  onShowToast,
  isSearchOpen,
  onSetSearchOpen,
  showWorkspaceTab,
  promoteMediaStudios,
  activeWorkspacePath,
  recentWorkspacePaths,
  onBrowseFolder,
  onUpdateSessionWorkspace,
  onNewChatInProject,
}: DashboardSidebarProps) {
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null)
  const [editTitleValue, setEditTitleValue] = useState('')
  const [isMoreOpen, setIsMoreOpen] = useState(false)
  const [isAccountMenuOpen, setIsAccountMenuOpen] = useState(false)
  const [moveMenu, setMoveMenu] = useState<{
    sessionId: string
    currentWorkspacePath?: string | null
    top: number
    left: number
  } | null>(null)
  const moveMenuRef = useRef<HTMLDivElement>(null)
  const moreContainerRef = useRef<HTMLDivElement>(null)
  const accountContainerRef = useRef<HTMLDivElement>(null)
  const asideRef = useRef<HTMLElement>(null)
  const { user, avatarUrl, logout } = useAuthUser()
  const [isResizing, setIsResizing] = useState(false)
  const dragRef = useRef<{ startX: number; startWidth: number; lastWidth: number; frame: number | null } | null>(null)

  useEffect(() => {
    if (!isResizing) return

    const handleMouseMove = (event: MouseEvent) => {
      const drag = dragRef.current
      if (!drag) return
      const nextWidth = drag.startWidth + (event.clientX - drag.startX)
      drag.lastWidth = nextWidth
      if (drag.frame != null) return
      drag.frame = requestAnimationFrame(() => {
        if (dragRef.current) {
          dragRef.current.frame = null
          onSidebarResize?.(dragRef.current.lastWidth)
        }
      })
    }

    const handleMouseUp = () => {
      const drag = dragRef.current
      if (drag?.frame != null) cancelAnimationFrame(drag.frame)
      setIsResizing(false)
      document.body.style.cursor = ''
      document.body.style.userSelect = ''
      if (drag) onSidebarResizeEnd?.(drag.lastWidth)
      dragRef.current = null
    }

    document.body.style.cursor = 'col-resize'
    document.body.style.userSelect = 'none'
    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)
    return () => {
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }, [isResizing, onSidebarResize, onSidebarResizeEnd])

  const handleResizeStart = (event: ReactMouseEvent) => {
    if (sidebarCollapsed) return
    event.preventDefault()
    const startWidth = asideRef.current?.getBoundingClientRect().width ?? 264
    dragRef.current = { startX: event.clientX, startWidth, lastWidth: startWidth, frame: null }
    setIsResizing(true)
  }

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (moreContainerRef.current && !moreContainerRef.current.contains(event.target as Node)) {
        setIsMoreOpen(false)
      }
      if (accountContainerRef.current && !accountContainerRef.current.contains(event.target as Node)) {
        setIsAccountMenuOpen(false)
      }
      if (moveMenuRef.current && !moveMenuRef.current.contains(event.target as Node)) {
        setMoveMenu(null)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const handleSaveRename = (id: string) => {
    if (editTitleValue.trim() && editTitleValue.trim() !== chatSessions.find(s => s.id === id)?.title) {
      onRenameConversation?.(id, editTitleValue.trim())
    }
    setEditingSessionId(null)
  }
  const toggleInteractionGuide = () => {
    const next = !showInteractionGuide
    onSetShowInteractionGuide?.(next)
    onShowToast?.(next ? 'Show interaction zones ⊹' : 'Hide interaction zones ⊹')
  }

  const toggleInteraction = () => {
    const next = !interactionEnabled
    onSetInteractionEnabled?.(next)
    onShowToast?.(next ? 'Enable model interaction ⦸' : 'Disable model interaction ⦸')
  }

  const conversationSessions = chatSessions.filter((session) => session.kind !== 'cli' && !session.id.startsWith('cli') && session.id !== 'conversation-default')
  const cliSessions = chatSessions.filter((session) => session.kind === 'cli' || session.id.startsWith('cli'))

  const [projectOrder, setProjectOrder] = useState<string[]>(() => {
    if (typeof window === 'undefined') return []
    try {
      const saved = localStorage.getItem('mint_project_order')
      return saved ? JSON.parse(saved) : []
    } catch {
      return []
    }
  })
  const [draggedProjectId, setDraggedProjectId] = useState<string | null>(null)
  const [dragOverProjectId, setDragOverProjectId] = useState<string | null>(null)
  const [dropPosition, setDropPosition] = useState<'before' | 'after' | null>(null)

  const handleProjectDragStart = (e: ReactDragEvent, projectId: string) => {
    e.dataTransfer.setData('text/plain', projectId)
    e.dataTransfer.effectAllowed = 'move'
    setDraggedProjectId(projectId)
  }

  const handleProjectDragOver = (e: ReactDragEvent, targetId: string) => {
    if (!draggedProjectId || draggedProjectId === targetId) return
    e.preventDefault()
    e.stopPropagation()
    e.dataTransfer.dropEffect = 'move'

    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const midY = rect.top + rect.height / 2
    const pos = e.clientY < midY ? 'before' : 'after'

    if (dragOverProjectId !== targetId || dropPosition !== pos) {
      setDragOverProjectId(targetId)
      setDropPosition(pos)
    }
  }

  const handleProjectDragLeave = (e: ReactDragEvent, targetId: string) => {
    e.stopPropagation()
    if (dragOverProjectId === targetId) {
      const related = e.relatedTarget as Node | null
      if (!related || !(e.currentTarget as HTMLElement).contains(related)) {
        setDragOverProjectId(null)
        setDropPosition(null)
      }
    }
  }

  const handleProjectDrop = (e: ReactDragEvent, targetId: string) => {
    e.preventDefault()
    e.stopPropagation()
    if (!draggedProjectId || draggedProjectId === targetId) {
      setDraggedProjectId(null)
      setDragOverProjectId(null)
      setDropPosition(null)
      return
    }

    const allIds = projectGroups.map((g) => g.id)
    let newOrder = [...projectOrder]
    for (const id of allIds) {
      if (!newOrder.includes(id)) {
        newOrder.push(id)
      }
    }

    newOrder = newOrder.filter((id) => id !== draggedProjectId)
    const targetIdx = newOrder.indexOf(targetId)
    if (targetIdx !== -1) {
      const insertIdx = dropPosition === 'after' ? targetIdx + 1 : targetIdx
      newOrder.splice(insertIdx, 0, draggedProjectId)
    } else {
      newOrder.push(draggedProjectId)
    }

    setProjectOrder(newOrder)
    try {
      localStorage.setItem('mint_project_order', JSON.stringify(newOrder))
    } catch (err) {
      console.error('Failed to save project order', err)
    }

    setDraggedProjectId(null)
    setDragOverProjectId(null)
    setDropPosition(null)
  }

  const handleProjectDragEnd = () => {
    setDraggedProjectId(null)
    setDragOverProjectId(null)
    setDropPosition(null)
  }

  // Group conversations by Workspace Project
  const { projectGroups, recentSessions } = useMemo(() => {
    const getProjectInfo = (workspacePath?: string | null): { id: string; name: string } | null => {
      if (!workspacePath) return null
      const clean = workspacePath.replace(/[\\/]+$/, '').trim()
      if (!clean) return null
      const parts = clean.split(/[\\/]/)
      const name = parts[parts.length - 1] || clean
      return { id: clean, name }
    }

    const groupsMap = new Map<string, { id: string; name: string; sessions: ChatSessionItem[] }>()
    const recents: ChatSessionItem[] = []

    // If active workspace is known, initialize its project folder
    const activeProj = getProjectInfo(activeWorkspacePath)
    if (activeProj) {
      groupsMap.set(activeProj.id, { id: activeProj.id, name: activeProj.name, sessions: [] })
    }

    for (const session of conversationSessions) {
      const effectivePath = session.workspacePath || null
      const proj = getProjectInfo(effectivePath)
      if (proj) {
        if (!groupsMap.has(proj.id)) {
          groupsMap.set(proj.id, { id: proj.id, name: proj.name, sessions: [] })
        }
        groupsMap.get(proj.id)!.sessions.push(session)
      } else {
        recents.push(session)
      }
    }

    // Sort groups stably according to user custom projectOrder, fallback to name
    const sortedGroups = Array.from(groupsMap.values()).sort((a, b) => {
      const idxA = projectOrder.indexOf(a.id)
      const idxB = projectOrder.indexOf(b.id)
      if (idxA !== -1 && idxB !== -1) return idxA - idxB
      if (idxA !== -1) return -1
      if (idxB !== -1) return 1
      return a.name.localeCompare(b.name)
    })

    return { projectGroups: sortedGroups, recentSessions: recents }
  }, [conversationSessions, activeWorkspacePath, activeConversationId, projectOrder])

  const availableProjects = useMemo(() => {
    const getProjectInfo = (workspacePath?: string | null): { id: string; name: string } | null => {
      if (!workspacePath) return null
      const clean = workspacePath.replace(/[\\/]+$/, '').trim()
      if (!clean) return null
      const parts = clean.split(/[\\/]/)
      const name = parts[parts.length - 1] || clean
      return { id: clean, name }
    }

    const map = new Map<string, string>()
    if (activeWorkspacePath) {
      const info = getProjectInfo(activeWorkspacePath)
      if (info) map.set(info.id, info.name)
    }
    for (const g of projectGroups) {
      map.set(g.id, g.name)
    }
    if (recentWorkspacePaths) {
      for (const p of recentWorkspacePaths) {
        const info = getProjectInfo(p)
        if (info && !map.has(info.id)) {
          map.set(info.id, info.name)
        }
      }
    }
    return Array.from(map.entries()).map(([path, name]) => ({ path, name }))
  }, [projectGroups, activeWorkspacePath, recentWorkspacePaths])

  const openMoveMenu = (event: ReactMouseEvent, session: ChatSessionItem) => {
    event.stopPropagation()
    event.preventDefault()
    if (moveMenu?.sessionId === session.id) {
      setMoveMenu(null)
      return
    }
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
    const popoverWidth = 220
    const left = Math.min(rect.left, Math.max(10, window.innerWidth - popoverWidth - 10))
    const top = rect.bottom + 6
    setMoveMenu({
      sessionId: session.id,
      currentWorkspacePath: session.workspacePath || (session.id === activeConversationId && activeWorkspacePath ? activeWorkspacePath : null),
      top,
      left,
    })
  }

  const [collapsedProjects, setCollapsedProjects] = useState<Record<string, boolean>>(() => {
    if (typeof window === 'undefined') return {}
    try {
      const saved = localStorage.getItem('mint_collapsed_projects')
      return saved ? JSON.parse(saved) : {}
    } catch {
      return {}
    }
  })

  const [expandedShowMore, setExpandedShowMore] = useState<Record<string, boolean>>({})

  const toggleProjectCollapse = (projectId: string) => {
    setCollapsedProjects((prev) => {
      const next = { ...prev, [projectId]: !prev[projectId] }
      try {
        localStorage.setItem('mint_collapsed_projects', JSON.stringify(next))
      } catch {
        // ignore
      }
      return next
    })
  }

  const toggleShowMore = (projectId: string) => {
    setExpandedShowMore((prev) => ({ ...prev, [projectId]: !prev[projectId] }))
  }

  // Remember the last active CLI session so it remains pinned in the sidebar
  // even when the user navigates away to a regular conversation!
  const [pinnedCliId, setPinnedCliId] = useState<string | null>(() => {
    return typeof window !== 'undefined' ? localStorage.getItem('mint_last_cli_session_id') : null
  })

  useEffect(() => {
    if (activeConversationId.startsWith('cli') || activeConversationId === 'cli') {
      setPinnedCliId(activeConversationId)
      try {
        localStorage.setItem('mint_last_cli_session_id', activeConversationId)
      } catch {
        // ignore
      }
    }
  }, [activeConversationId])

  const targetCliId = (activeConversationId.startsWith('cli') || activeConversationId === 'cli')
    ? activeConversationId
    : (pinnedCliId || cliSessions[0]?.id || null)

  const pinnedCliSession = cliSessions.find((session) => session.id === targetCliId)

  return (
    <aside className={`workspace-sidebar ${isResizing ? 'is-resizing' : ''}`} ref={asideRef}>
      {!sidebarCollapsed && (
        <div
          className={`sidebar-resize-handle ${isResizing ? 'is-active' : ''}`}
          onMouseDown={handleResizeStart}
          title="Drag to resize sidebar"
        />
      )}
      <div
        className="sidebar-brand clickable"
        onClick={onToggleSidebar}
        title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        role="button"
        tabIndex={0}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') onToggleSidebar()
        }}
      >
        <img src={runtimePlatform.appIconPath()} alt="Mint Agent Logo" className="sidebar-logo" />
        <span className="sidebar-brand-name">Mint Agent</span>
      </div>
      <button type="button" className="sidebar-mobile-close" aria-label="Close navigation menu" onClick={onToggleSidebar}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
          <path d="M5 5l14 14M19 5L5 19" />
        </svg>
      </button>

      <button className="sidebar-new-chat" onClick={() => onClearHistory('New chat')} title="New chat (Ctrl+N)">
        <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <line x1="12" y1="5" x2="12" y2="19"></line>
            <line x1="5" y1="12" x2="19" y2="12"></line>
          </svg>
        </span>
        <span>New chat</span>
      </button>

      <button className="sidebar-top-action sidebar-search-btn" onClick={() => onSetSearchOpen(true)} title="Search chats (Ctrl+K)">
        <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
        </span>
        <span>Search chats</span>
      </button>

      <button className={`sidebar-top-action ${view === 'chat' ? 'is-active' : ''}`} onClick={() => onSetView('chat')} title="Chat">
        <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
          </svg>
        </span>
        <span>Chat</span>
      </button>

      <button className={`sidebar-top-action ${view === 'pictures' ? 'is-active' : ''}`} onClick={() => onSetView('pictures')} title="Pictures">
        <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
            <circle cx="8.5" cy="8.5" r="1.5"></circle>
            <polyline points="21 15 16 10 5 21"></polyline>
          </svg>
        </span>
        <span>Pictures</span>
      </button>
      {promoteMediaStudios && (
        <>
          <button className={`sidebar-top-action ${view === 'imagine' ? 'is-active' : ''}`} onClick={() => onSetView('imagine')}>
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
              </svg>
            </span>
            <span>Image Studio</span>
          </button>
          <button className={`sidebar-top-action ${view === 'veo' ? 'is-active' : ''}`} onClick={() => onSetView('veo')}>
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
                <line x1="7" y1="2" x2="7" y2="22"></line>
                <line x1="17" y1="2" x2="17" y2="22"></line>
                <line x1="2" y1="12" x2="22" y2="12"></line>
                <line x1="2" y1="7" x2="7" y2="7"></line>
                <line x1="2" y1="17" x2="7" y2="17"></line>
                <line x1="17" y1="17" x2="22" y2="17"></line>
                <line x1="17" y1="7" x2="22" y2="7"></line>
              </svg>
            </span>
            <span>Veo Studio</span>
          </button>
        </>
      )}
      {showWorkspaceTab && (
        <button className={`sidebar-top-action ${view === 'workspace' ? 'is-active' : ''}`} onClick={() => onSetView('workspace')} title="Workspace">
          <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M3 6h7l2 2h9v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"></path>
              <path d="M3 6v12"></path>
            </svg>
          </span>
          <span>Workspace</span>
        </button>
      )}
      {onToggleModel && (
        <button className={`sidebar-top-action ${modelVisible ? 'is-active' : ''}`} onClick={onToggleModel} title="Live2D model">
          <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
              <circle cx="12" cy="7" r="4"></circle>
            </svg>
          </span>
          <span>Live2D model</span>
        </button>
      )}

      <div className="sidebar-more-container" ref={moreContainerRef}>
        <button className={`sidebar-top-action ${isMoreOpen || (!promoteMediaStudios && (view === 'imagine' || view === 'veo')) || view === 'skills' || view === 'mcp' || view === 'plugins' || view === 'cron' || view === 'link' ? 'is-active' : ''}`} onClick={() => setIsMoreOpen(!isMoreOpen)} title="More" aria-expanded={isMoreOpen} aria-controls="sidebar-more-menu">
          <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="1.5"></circle>
              <circle cx="19" cy="12" r="1.5"></circle>
              <circle cx="5" cy="12" r="1.5"></circle>
            </svg>
          </span>
          <span>More</span>
        </button>
        {isMoreOpen && (
          <div id="sidebar-more-menu" className="sidebar-more-popover" style={{ display: 'flex', flexDirection: 'column', gap: '4px', padding: '6px', minWidth: '160px' }}>
            <button className={`popover-item ${view === 'skills' ? 'active' : ''}`} onClick={() => { onSetView('skills'); setIsMoreOpen(false); }}>
              <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderSkillsSvgIcon(15)}</span>
              <span>Skills</span>
            </button>
            <button className={`popover-item ${view === 'mcp' ? 'active' : ''}`} onClick={() => { onSetView('mcp'); setIsMoreOpen(false); }}>
              <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderMcpHubSvgIcon(15)}</span>
              <span>MCP servers</span>
            </button>
            <button className={`popover-item ${view === 'plugins' ? 'active' : ''}`} onClick={() => { onSetView('plugins'); setIsMoreOpen(false); }}>
              <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderPluginsSvgIcon(15)}</span>
              <span>Plugins</span>
            </button>
            <button className={`popover-item ${view === 'cron' ? 'active' : ''}`} onClick={() => { onSetView('cron'); setIsMoreOpen(false); }}>
              <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderScheduledTasksSvgIcon(15)}</span>
              <span>Scheduled tasks</span>
            </button>
            <button className={`popover-item ${view === 'link' ? 'active' : ''}`} onClick={() => { onSetView('link'); setIsMoreOpen(false); }}>
              <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderLinkedFoldersSvgIcon(15)}</span>
              <span>Linked folders</span>
            </button>
            {!promoteMediaStudios && (
              <button className={`popover-item ${view === 'imagine' ? 'active' : ''}`} onClick={() => { onSetView('imagine'); setIsMoreOpen(false); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
                  </svg>
                </span>
                <span>Image Studio</span>
              </button>
            )}
            {!promoteMediaStudios && (
              <button className={`popover-item ${view === 'veo' ? 'active' : ''}`} onClick={() => { onSetView('veo'); setIsMoreOpen(false); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <polygon points="23 7 16 12 23 17 23 7"></polygon>
                    <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
                  </svg>
                </span>
                <span>Veo Studio</span>
              </button>
            )}
          </div>
        )}
      </div>


      {modelVisible && (
        <div className="sidebar-model-controls">
          <button
            className="change-expression-btn"
            onClick={() => {
              const next = (expressionIndex + 1) % expressions.length
              onSetExpressionIndex?.(next)
              onSetAccessoryIndex?.(0)
              onShowToast?.(`Expression: ${expressions[next]}`)
            }}
          >
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10"></circle>
                <path d="M8 14s1.5 2 4 2 4-2 4-2"></path>
                <line x1="9" y1="9" x2="9.01" y2="9"></line>
                <line x1="15" y1="9" x2="15.01" y2="9"></line>
              </svg>
            </span>
            <span>Expression</span>
          </button>
          <button
            className="accessory-cycle-btn"
            onClick={() => {
              const next = (accessoryIndex + 1) % accessories.length
              onSetAccessoryIndex?.(next)
              onSetExpressionIndex?.(0)
              onShowToast?.(`Accessory: ${accessories[next]}`)
            }}
          >
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 12c-2-2.67-4-4-6-4a4 4 0 1 0 0 8c2 0 4-1.33 6-4Zm0 0c2 2.67 4 4 6 4a4 4 0 1 0 0-8c-2 0-4 1.33-6 4Z"></path>
              </svg>
            </span>
            <span>Accessory</span>
          </button>
          <button className={`toggle-interaction-btn ${interactionEnabled ? 'active' : ''}`} onClick={toggleInteraction}>
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M18 11V6a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v5m-4 0V4a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v7m-4 0V5a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v9M4 22V13a3 3 0 0 1 6 0v0M4 22h14a2 2 0 0 0 2-2V11a2 2 0 0 0-2-2h-2"></path>
              </svg>
            </span>
            <span className="mint-status-label">Interact</span>
          </button>
          <button className={`interaction-guide-btn ${showInteractionGuide ? 'active' : ''}`} onClick={toggleInteractionGuide}>
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="22" y1="12" x2="18" y2="12"></line>
                <line x1="6" y1="12" x2="2" y2="12"></line>
                <line x1="12" y1="6" x2="12" y2="2"></line>
                <line x1="12" y1="22" x2="12" y2="18"></line>
              </svg>
            </span>
            <span>Areas</span>
          </button>
        </div>
      )}

      {sidebarCollapsed ? (
        <div className="sidebar-collapsed-sessions">
          <button
            type="button"
            className={`sidebar-collapsed-item ${view === 'code' ? 'is-active' : ''}`}
            onClick={() => onSetView('code')}
            title="Code sessions Hub"
            aria-label="Code sessions Hub"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <polyline points="4 17 10 11 4 5"></polyline>
              <line x1="12" y1="19" x2="20" y2="19"></line>
            </svg>
            {view === 'code' && <span className="sidebar-collapsed-dot" />}
          </button>
          {pinnedCliSession && (
            <button
              type="button"
              className={`sidebar-collapsed-item ${pinnedCliSession.id === activeConversationId && view !== 'code' ? 'is-active' : ''}`}
              onClick={() => onSelectConversation(pinnedCliSession.id)}
              title={pinnedCliSession.title && pinnedCliSession.title !== 'cli' ? pinnedCliSession.title : 'Terminal Session'}
              aria-label={pinnedCliSession.title && pinnedCliSession.title !== 'cli' ? pinnedCliSession.title : 'Terminal Session'}
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <polyline points="4 17 10 11 4 5"></polyline>
                <line x1="12" y1="19" x2="20" y2="19"></line>
              </svg>
              {pinnedCliSession.id === activeConversationId && view !== 'code' && <span className="sidebar-collapsed-dot" />}
            </button>
          )}
          {conversationSessions.slice(0, 25).map((session) => {
            const isActive = session.id === activeConversationId && view !== 'code'
            return (
              <button
                type="button"
                key={session.id}
                className={`sidebar-collapsed-item ${isActive ? 'is-active' : ''}`}
                onClick={() => onSelectConversation(session.id)}
                title={session.title || 'Chat'}
                aria-label={session.title || 'Chat'}
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
                </svg>
                {isActive && <span className="sidebar-collapsed-dot" />}
              </button>
            )
          })}
        </div>
      ) : (
        <div className="sidebar-section">
          <div className="sidebar-section-title">Code</div>
        <div className="sidebar-chat-list sidebar-cli-list">
          <button
            className={`sidebar-project sidebar-chat-item ${view === 'code' ? 'active' : ''}`}
            onClick={() => onSetView('code')}
            title="Code sessions Hub"
          >
            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="4 17 10 11 4 5"></polyline>
                <line x1="12" y1="19" x2="20" y2="19"></line>
              </svg>
            </span>
            <span className="sidebar-chat-title">Code hub</span>
            <span
              style={{
                marginLeft: 'auto',
                fontSize: '0.72rem',
                fontWeight: 600,
                padding: '1px 6px',
                borderRadius: '10px',
                background: 'rgba(255, 255, 255, 0.08)',
                color: 'var(--text-muted, #94a3b8)',
              }}
            >
              {cliSessions.length}
            </span>
          </button>

          {pinnedCliSession && (
            <button
              className={`sidebar-project sidebar-chat-item ${pinnedCliSession.id === activeConversationId ? 'active' : ''}`}
              onClick={() => onSelectConversation(pinnedCliSession.id)}
              title={pinnedCliSession.title || 'Terminal Session'}
              style={{ paddingLeft: '16px' }}
            >
              <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center', opacity: 0.85 }}>
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M4 17l6-6-6-6"></path>
                  <path d="M12 19h8"></path>
                </svg>
              </span>
              <span className="sidebar-chat-title">
                {pinnedCliSession.title && pinnedCliSession.title !== 'cli'
                  ? pinnedCliSession.title
                  : (pinnedCliSession.id === 'cli' ? 'Terminal Session' : `cli::${pinnedCliSession.id.replace(/^cli::/, '').slice(0, 7)}`)}
              </span>
              {pinnedCliSession.id === activeConversationId && sending && (
                <span className="sidebar-generating-text">Thinking...</span>
              )}
            </button>
          )}
        </div>
        {projectGroups.length > 0 && (
          <>
            <div className="sidebar-section-title sidebar-subsection-title">Projects</div>
            <div className="sidebar-projects-list">
              {projectGroups.map((group) => {
                const isCollapsed = !!collapsedProjects[group.id]
                const isExpanded = !!expandedShowMore[group.id]
                const maxVisible = 3
                const hasMore = group.sessions.length > maxVisible
                const visibleSessions = (isExpanded || !hasMore)
                  ? group.sessions
                  : group.sessions.slice(0, maxVisible)

                const isDragging = draggedProjectId === group.id
                const isDragOver = dragOverProjectId === group.id

                return (
                  <div
                    key={group.id}
                    className={`sidebar-project-group ${isDragging ? 'is-dragging' : ''} ${isDragOver ? (dropPosition === 'before' ? 'drop-before' : 'drop-after') : ''}`}
                    onDragOver={(e) => handleProjectDragOver(e, group.id)}
                    onDragLeave={(e) => handleProjectDragLeave(e, group.id)}
                    onDrop={(e) => handleProjectDrop(e, group.id)}
                  >
                    <div
                      role="button"
                      tabIndex={0}
                      className="sidebar-project-group-header"
                      onClick={() => toggleProjectCollapse(group.id)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault()
                          toggleProjectCollapse(group.id)
                        }
                      }}
                      title={`${group.name} (${group.id}) - Drag to reorder`}
                      draggable
                      onDragStart={(e) => handleProjectDragStart(e, group.id)}
                      onDragEnd={handleProjectDragEnd}
                    >
                      <span className="sidebar-project-group-left">
                        <span className="sidebar-project-drag-grip" title="Drag to reorder" aria-hidden="true">
                          <svg width="8" height="12" viewBox="0 0 8 12" fill="currentColor">
                            <circle cx="2" cy="2" r="1.2" />
                            <circle cx="6" cy="2" r="1.2" />
                            <circle cx="2" cy="6" r="1.2" />
                            <circle cx="6" cy="6" r="1.2" />
                            <circle cx="2" cy="10" r="1.2" />
                            <circle cx="6" cy="10" r="1.2" />
                          </svg>
                        </span>
                        <svg
                          xmlns="http://www.w3.org/2000/svg"
                          width="14"
                          height="14"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          className="sidebar-project-folder-icon"
                        >
                          {!isCollapsed ? (
                            <>
                              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                              <polygon points="3 20 6 10 23 10 20 20 3 20" fill="currentColor" fillOpacity="0.15"></polygon>
                            </>
                          ) : (
                            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                          )}
                        </svg>
                        <span className="sidebar-project-group-name">{group.name}</span>
                      </span>
                      <span className="sidebar-project-group-right">
                        {onNewChatInProject && (
                          <span
                            role="button"
                            tabIndex={0}
                            className="sidebar-project-add-btn"
                            title={`New chat in ${group.name}`}
                            onClick={(e) => {
                              e.stopPropagation()
                              setCollapsedProjects((prev) => ({ ...prev, [group.id]: false }))
                              onNewChatInProject(group.id)
                            }}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter' || e.key === ' ') {
                                e.preventDefault()
                                e.stopPropagation()
                                setCollapsedProjects((prev) => ({ ...prev, [group.id]: false }))
                                onNewChatInProject(group.id)
                              }
                            }}
                          >
                            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                              <line x1="12" y1="5" x2="12" y2="19"></line>
                              <line x1="5" y1="12" x2="19" y2="12"></line>
                            </svg>
                          </span>
                        )}
                        <span className="sidebar-project-group-count">{group.sessions.length}</span>
                        <span
                          className="sidebar-project-group-chevron"
                          style={{
                            transform: isCollapsed ? 'rotate(-90deg)' : 'rotate(0deg)',
                            transition: 'transform 0.18s ease'
                          }}
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                            <polyline points="6 9 12 15 18 9"></polyline>
                          </svg>
                        </span>
                      </span>
                    </div>
                    {!isCollapsed && (
                      <div className="sidebar-project-sessions">
                        {visibleSessions.map((session) => (
                          <button
                            key={session.id}
                            className={`sidebar-project sidebar-chat-item is-indented ${session.id === activeConversationId ? 'active' : ''}`}
                            onClick={() => onSelectConversation(session.id)}
                            title={session.title}
                          >
                            <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                              <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                                <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
                              </svg>
                            </span>
                            {editingSessionId === session.id ? (
                              <input
                                type="text"
                                className="sidebar-chat-rename-input"
                                value={editTitleValue}
                                onChange={(e) => setEditTitleValue(e.target.value)}
                                onBlur={() => handleSaveRename(session.id)}
                                onKeyDown={(e) => {
                                  if (e.key === 'Enter') {
                                    handleSaveRename(session.id)
                                  } else if (e.key === 'Escape') {
                                    setEditingSessionId(null)
                                  }
                                }}
                                autoFocus
                                onClick={(e) => e.stopPropagation()}
                              />
                            ) : (
                              <span className="sidebar-chat-title">{session.title || 'New chat'}</span>
                            )}
                            {session.id === activeConversationId && sending && (
                              <span className="sidebar-generating-text">Thinking...</span>
                            )}
                            {editingSessionId !== session.id && (
                              <>
                                <span
                                  className="sidebar-chat-edit"
                                  role="button"
                                  tabIndex={0}
                                  title="Rename conversation"
                                  onClick={(event) => {
                                    event.stopPropagation()
                                    setEditingSessionId(session.id)
                                    setEditTitleValue(session.title || '')
                                  }}
                                  onKeyDown={(event) => {
                                    if (event.key === 'Enter' || event.key === ' ') {
                                      event.preventDefault()
                                      event.stopPropagation()
                                      setEditingSessionId(session.id)
                                      setEditTitleValue(session.title || '')
                                    }
                                  }}
                                >
                                  <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                                    <path d="M12 20h9"></path>
                                    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
                                  </svg>
                                </span>
                                {onUpdateSessionWorkspace && (
                                  <span
                                    className="sidebar-chat-edit"
                                    role="button"
                                    tabIndex={0}
                                    title="Move conversation..."
                                    onClick={(event) => openMoveMenu(event, session)}
                                    onKeyDown={(event) => {
                                      if (event.key === 'Enter' || event.key === ' ') {
                                        event.preventDefault()
                                        openMoveMenu(event as any, session)
                                      }
                                    }}
                                  >
                                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                                      <polyline points="12 11 12 17"></polyline>
                                      <polyline points="9 14 12 11 15 14"></polyline>
                                    </svg>
                                  </span>
                                )}
                                <span
                                  className="sidebar-chat-delete"
                                  role="button"
                                  tabIndex={0}
                                  title="Delete conversation"
                                  onClick={(event) => {
                                    event.stopPropagation()
                                    onDeleteConversation(session.id)
                                  }}
                                  onKeyDown={(event) => {
                                    if (event.key === 'Enter' || event.key === ' ') {
                                      event.preventDefault()
                                      event.stopPropagation()
                                      onDeleteConversation(session.id)
                                    }
                                  }}
                                >
                                  <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                                    <polyline points="3 6 5 6 21 6"></polyline>
                                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"></path>
                                    <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                                  </svg>
                                </span>
                              </>
                            )}
                          </button>
                        ))}
                        {hasMore && (
                          <button
                            type="button"
                            className="sidebar-show-more-btn"
                            onClick={() => toggleShowMore(group.id)}
                          >
                            {isExpanded
                              ? 'Show less ⌃'
                              : `Show more (${group.sessions.length - maxVisible}) ⌄`}
                          </button>
                        )}
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          </>
        )}

        {(recentSessions.length > 0 || projectGroups.length === 0) && (
          <>
            <div className="sidebar-section-title sidebar-subsection-title">
              {projectGroups.length > 0 ? 'Recents' : 'Conversations'}
            </div>
            <div className="sidebar-chat-list">
              {recentSessions.map((session) => (
                <button
                  key={session.id}
                  className={`sidebar-project sidebar-chat-item ${session.id === activeConversationId ? 'active' : ''}`}
                  onClick={() => onSelectConversation(session.id)}
                  title={session.title}
                >
                  <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
                    </svg>
                  </span>
                  {editingSessionId === session.id ? (
                    <input
                      type="text"
                      className="sidebar-chat-rename-input"
                      value={editTitleValue}
                      onChange={(e) => setEditTitleValue(e.target.value)}
                      onBlur={() => handleSaveRename(session.id)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          handleSaveRename(session.id)
                        } else if (e.key === 'Escape') {
                          setEditingSessionId(null)
                        }
                      }}
                      autoFocus
                      onClick={(e) => e.stopPropagation()}
                    />
                  ) : (
                    <span className="sidebar-chat-title">{session.title || 'New chat'}</span>
                  )}
                  {session.id === activeConversationId && sending && (
                    <span className="sidebar-generating-text">Thinking...</span>
                  )}
                  {editingSessionId !== session.id && (
                    <>
                      <span
                        className="sidebar-chat-edit"
                        role="button"
                        tabIndex={0}
                        title="Rename conversation"
                        onClick={(event) => {
                          event.stopPropagation()
                          setEditingSessionId(session.id)
                          setEditTitleValue(session.title || '')
                        }}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter' || event.key === ' ') {
                            event.preventDefault()
                            event.stopPropagation()
                            setEditingSessionId(session.id)
                            setEditTitleValue(session.title || '')
                          }
                        }}
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M12 20h9"></path>
                          <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
                        </svg>
                      </span>
                      {onUpdateSessionWorkspace && (
                        <span
                          className="sidebar-chat-edit"
                          role="button"
                          tabIndex={0}
                          title="Move conversation..."
                          onClick={(event) => openMoveMenu(event, session)}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter' || event.key === ' ') {
                              event.preventDefault()
                              openMoveMenu(event as any, session)
                            }
                          }}
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                            <polyline points="12 11 12 17"></polyline>
                            <polyline points="9 14 12 11 15 14"></polyline>
                          </svg>
                        </span>
                      )}
                      <span
                        className="sidebar-chat-delete"
                        role="button"
                        tabIndex={0}
                        title="Delete conversation"
                        onClick={(event) => {
                          event.stopPropagation()
                          onDeleteConversation(session.id)
                        }}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter' || event.key === ' ') {
                            event.preventDefault()
                            event.stopPropagation()
                            onDeleteConversation(session.id)
                          }
                        }}
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                          <polyline points="3 6 5 6 21 6"></polyline>
                          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"></path>
                          <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                        </svg>
                      </span>
                    </>
                  )}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
      )}

      {user && (
        <div className="sidebar-account-container" ref={accountContainerRef}>
          {isAccountMenuOpen && (
            <div className="sidebar-account-menu">
              {user.email && (
                <div className="sidebar-account-menu-header">{user.email}</div>
              )}
              <button className="popover-item" onClick={() => { window.api.openSettings(); setIsAccountMenuOpen(false); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                  <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="12" cy="12" r="3"></circle>
                    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
                  </svg>
                </span>
                <span>Settings</span>
              </button>
              <button className={`popover-item ${view === 'skills' ? 'active' : ''}`} onClick={() => { onSetView('skills'); setIsAccountMenuOpen(false); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderSkillsSvgIcon(15)}</span>
                <span>Skills</span>
              </button>
              <button className={`popover-item ${view === 'mcp' ? 'active' : ''}`} onClick={() => { onSetView('mcp'); setIsAccountMenuOpen(false); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderMcpHubSvgIcon(15)}</span>
                <span>MCP servers</span>
              </button>
              <button className={`popover-item ${view === 'plugins' ? 'active' : ''}`} onClick={() => { onSetView('plugins'); setIsAccountMenuOpen(false); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderPluginsSvgIcon(15)}</span>
                <span>Plugins</span>
              </button>
              <button className={`popover-item ${view === 'cron' ? 'active' : ''}`} onClick={() => { onSetView('cron'); setIsAccountMenuOpen(false); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderScheduledTasksSvgIcon(15)}</span>
                <span>Scheduled tasks</span>
              </button>
              <button className={`popover-item ${view === 'link' ? 'active' : ''}`} onClick={() => { onSetView('link'); setIsAccountMenuOpen(false); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>{renderLinkedFoldersSvgIcon(15)}</span>
                <span>Linked folders</span>
              </button>
              <div className="sidebar-account-menu-divider" />
              <button className="popover-item" onClick={() => { setIsAccountMenuOpen(false); logout(); }}>
                <span aria-hidden="true" style={{ display: 'inline-flex', alignItems: 'center' }}>
                  <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
                    <polyline points="16 17 21 12 16 7"></polyline>
                    <line x1="21" y1="12" x2="9" y2="12"></line>
                  </svg>
                </span>
                <span>Log out</span>
              </button>
            </div>
          )}
          <button className="sidebar-account" onClick={() => setIsAccountMenuOpen((open) => !open)}>
            <div className="sidebar-account-avatar">
              {avatarUrl ? (
                <img src={avatarUrl} alt={user.name || 'User'} />
              ) : (
                (user.name?.[0] || user.email?.[0] || 'U').toUpperCase()
              )}
            </div>
            <div className="sidebar-account-info">
              <span className="sidebar-account-name">{user.name || user.email}</span>
            </div>
            <span className="sidebar-account-chevron" aria-hidden="true">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="18 15 12 9 6 15"></polyline>
              </svg>
            </span>
          </button>
        </div>
      )}

      {moveMenu && (
        <div
          ref={moveMenuRef}
          className="sidebar-move-popover"
          style={{
            position: 'fixed',
            top: `${moveMenu.top}px`,
            left: `${moveMenu.left}px`,
            zIndex: 99999,
          }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="sidebar-move-header">Move to Project</div>
          <div className="sidebar-move-list">
            {availableProjects.map((p) => {
              const isCurrent = moveMenu.currentWorkspacePath === p.path
              return (
                <button
                  key={p.path}
                  type="button"
                  className={`sidebar-move-item ${isCurrent ? 'is-current' : ''}`}
                  onClick={() => {
                    onUpdateSessionWorkspace?.(moveMenu.sessionId, p.path)
                    setMoveMenu(null)
                  }}
                  title={p.path}
                >
                  <span className="sidebar-move-item-icon">📁</span>
                  <span className="sidebar-move-item-name">{p.name}</span>
                  {isCurrent && <span className="sidebar-move-item-check">✓</span>}
                </button>
              )
            })}
          </div>

          <div className="sidebar-move-divider" />

          <button
            type="button"
            className={`sidebar-move-item ${!moveMenu.currentWorkspacePath ? 'is-current' : ''}`}
            onClick={() => {
              onUpdateSessionWorkspace?.(moveMenu.sessionId, null)
              setMoveMenu(null)
            }}
          >
            <span className="sidebar-move-item-icon">
              <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="9 14 4 9 9 4"></polyline>
                <path d="M20 20v-7a4 4 0 0 0-4-4H4"></path>
              </svg>
            </span>
            <span className="sidebar-move-item-name">Recents (No project)</span>
            {!moveMenu.currentWorkspacePath && <span className="sidebar-move-item-check">✓</span>}
          </button>

          {onBrowseFolder && (
            <button
              type="button"
              className="sidebar-move-item"
              onClick={async () => {
                const picked = await onBrowseFolder()
                if (picked) {
                  onUpdateSessionWorkspace?.(moveMenu.sessionId, picked)
                }
                setMoveMenu(null)
              }}
            >
              <span className="sidebar-move-item-icon">
                <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
              </span>
              <span className="sidebar-move-item-name">Choose folder...</span>
            </button>
          )}
        </div>
      )}
    </aside>
  )
}
