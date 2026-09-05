import React, { useState, useEffect, useRef, useMemo } from 'react'

export interface PaletteItem {
  id: string
  title: string
  subtitle?: string
  category: 'Commands' | 'Models' | 'Chats' | 'Workspace'
  icon: string
  action: () => void
}

export interface CommandPaletteProps {
  isOpen: boolean
  onClose: () => void
  onSelectChat: (chatId: string) => void
  onNewChat: () => void
  onSelectWorkspace?: () => void
  onOpenSettings?: () => void
  onChangeView?: (view: any) => void
  onChangeModel?: (model: string) => void
  onChangeProvider?: (provider: string) => void
  onExecuteSlash?: (cmd: string) => void
  chatSessions: any[]
  currentChatId?: string
  workspacePath?: string
}

export function CommandPalette({
  isOpen,
  onClose,
  onSelectChat,
  onNewChat,
  onSelectWorkspace,
  onOpenSettings,
  onChangeView,
  onChangeModel,
  onChangeProvider,
  onExecuteSlash,
  chatSessions,
  currentChatId,
  workspacePath,
}: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (isOpen) {
      setQuery('')
      setSelectedIndex(0)
      setTimeout(() => inputRef.current?.focus(), 50)
    }
  }, [isOpen])

  const allItems: PaletteItem[] = useMemo(() => {
    const items: PaletteItem[] = []

    // 1. Commands & Actions
    items.push({
      id: 'cmd-new-chat',
      title: 'New Chat',
      subtitle: 'Start a clean conversation session',
      category: 'Commands',
      icon: '+',
      action: () => {
        onNewChat()
        onClose()
      },
    })

    if (onExecuteSlash) {
      items.push({
        id: 'cmd-rewind',
        title: '/rewind - Git Checkpoint Time Machine',
        subtitle: 'Roll back workspace to a previous checkpoint',
        category: 'Commands',
        icon: '↺',
        action: () => {
          onExecuteSlash('/rewind')
          onClose()
        },
      })
      items.push({
        id: 'cmd-repomap',
        title: '/repomap - Codebase AST Map',
        subtitle: 'Generate hierarchical tree-sitter map of workspace',
        category: 'Commands',
        icon: 'AST',
        action: () => {
          onExecuteSlash('/repomap')
          onClose()
        },
      })
      items.push({
        id: 'cmd-init',
        title: '/init - Initialize AGENTS.md',
        subtitle: 'Generate project instructions and guidelines file',
        category: 'Commands',
        icon: 'DOC',
        action: () => {
          onExecuteSlash('/init')
          onClose()
        },
      })
    }

    if (onChangeView) {
      items.push({
        id: 'cmd-skills',
        title: 'Manage Skills (/skills)',
        subtitle: 'Browse and customize agent skills',
        category: 'Commands',
        icon: 'SKL',
        action: () => {
          onChangeView('skills')
          onClose()
        },
      })
      items.push({
        id: 'cmd-mcp',
        title: 'MCP Catalog & Tools (/mcp)',
        subtitle: 'Configure Model Context Protocol servers',
        category: 'Commands',
        icon: 'MCP',
        action: () => {
          onChangeView('mcp')
          onClose()
        },
      })
      items.push({
        id: 'cmd-cron',
        title: 'Scheduled Tasks (/cron)',
        subtitle: 'View and manage recurring background tasks',
        category: 'Commands',
        icon: 'CRN',
        action: () => {
          onChangeView('cron')
          onClose()
        },
      })
      items.push({
        id: 'cmd-link',
        title: 'Linked Folders (/link)',
        subtitle: 'Connect external notes and doc directories',
        category: 'Commands',
        icon: 'DIR',
        action: () => {
          onChangeView('link')
          onClose()
        },
      })
    }

    if (onOpenSettings) {
      items.push({
        id: 'cmd-settings',
        title: 'Settings',
        subtitle: 'Configure API keys, models, and preferences',
        category: 'Commands',
        icon: 'SET',
        action: () => {
          onOpenSettings()
          onClose()
        },
      })
    }

    // 2. Workspace switcher
    if (onSelectWorkspace) {
      items.push({
        id: 'ws-switch',
        title: 'Switch Workspace Directory',
        subtitle: workspacePath ? `Current: ${workspacePath}` : 'Select a project folder',
        category: 'Workspace',
        icon: 'DIR',
        action: () => {
          onSelectWorkspace()
          onClose()
        },
      })
    }

    // 3. AI Models
    if (onChangeProvider && onChangeModel) {
      items.push({
        id: 'model-claude-37',
        title: 'Claude 3.7 Sonnet',
        subtitle: 'Anthropic • Hybrid reasoning and fast coding',
        category: 'Models',
        icon: 'CL',
        action: () => {
          onChangeProvider('anthropic')
          onChangeModel('claude-3-7-sonnet-20250219')
          onClose()
        },
      })
      items.push({
        id: 'model-claude-35',
        title: 'Claude 3.5 Sonnet',
        subtitle: 'Anthropic • High precision coding agent',
        category: 'Models',
        icon: 'CL',
        action: () => {
          onChangeProvider('anthropic')
          onChangeModel('claude-3-5-sonnet-20240620')
          onClose()
        },
      })
      items.push({
        id: 'model-deepseek-v3',
        title: 'DeepSeek-V3',
        subtitle: 'DeepSeek • Fast reasoning and cost efficiency',
        category: 'Models',
        icon: 'DS',
        action: () => {
          onChangeProvider('deepseek')
          onChangeModel('deepseek-chat')
          onClose()
        },
      })
      items.push({
        id: 'model-gpt-4o',
        title: 'GPT-4o',
        subtitle: 'OpenAI • Multimodal flagship model',
        category: 'Models',
        icon: 'AI',
        action: () => {
          onChangeProvider('openai')
          onChangeModel('gpt-4o')
          onClose()
        },
      })
      items.push({
        id: 'model-gemini-flash',
        title: 'Gemini 2.0 Flash',
        subtitle: 'Google • Ultra-fast multi-turn assistant',
        category: 'Models',
        icon: 'GM',
        action: () => {
          onChangeProvider('gemini')
          onChangeModel('gemini-2.0-flash')
          onClose()
        },
      })
      items.push({
        id: 'model-gemini-pro',
        title: 'Gemini 1.5 Pro',
        subtitle: 'Google • 2M token deep context reasoning',
        category: 'Models',
        icon: 'GM',
        action: () => {
          onChangeProvider('gemini')
          onChangeModel('gemini-1.5-pro')
          onClose()
        },
      })
      items.push({
        id: 'model-ollama',
        title: 'Local Ollama Model',
        subtitle: 'Ollama • 100% offline private inference',
        category: 'Models',
        icon: 'OL',
        action: () => {
          onChangeProvider('ollama')
          onClose()
        },
      })
    }

    // 4. Recent Chats
    for (const session of chatSessions || []) {
      items.push({
        id: `chat-${session.id}`,
        title: session.title || 'Untitled Chat',
        subtitle: `Chat session • ${session.id === currentChatId ? 'Active' : 'Previous'}`,
        category: 'Chats',
        icon: '#',
        action: () => {
          onSelectChat(session.id)
          onClose()
        },
      })
    }

    return items
  }, [
    onNewChat,
    onClose,
    onExecuteSlash,
    onChangeView,
    onOpenSettings,
    onSelectWorkspace,
    workspacePath,
    onChangeProvider,
    onChangeModel,
    chatSessions,
    currentChatId,
    onSelectChat,
  ])

  const filteredItems = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return allItems
    return allItems.filter((item) => {
      const matchTitle = item.title.toLowerCase().includes(q)
      const matchSub = item.subtitle ? item.subtitle.toLowerCase().includes(q) : false
      const matchCat = item.category.toLowerCase().includes(q)
      return matchTitle || matchSub || matchCat
    })
  }, [allItems, query])

  useEffect(() => {
    setSelectedIndex(0)
  }, [filteredItems])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex((prev) => (prev + 1 < filteredItems.length ? prev + 1 : 0))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex((prev) => (prev - 1 >= 0 ? prev - 1 : filteredItems.length - 1))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const selected = filteredItems[selectedIndex]
      if (selected) {
        selected.action()
      }
    } else if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
  }

  // Scroll selected item into view
  useEffect(() => {
    if (listRef.current) {
      const activeEl = listRef.current.querySelector('.palette-item.is-selected') as HTMLElement
      if (activeEl) {
        activeEl.scrollIntoView({ block: 'nearest' })
      }
    }
  }, [selectedIndex])

  if (!isOpen) return null

  // Group filtered items by category
  const grouped = filteredItems.reduce((acc, item) => {
    if (!acc[item.category]) acc[item.category] = []
    acc[item.category].push(item)
    return acc
  }, {} as Record<string, PaletteItem[]>)

  let globalIndex = 0

  return (
    <div
      className="command-palette-backdrop"
      onClick={onClose}
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(5, 7, 12, 0.72)',
        backdropFilter: 'blur(8px)',
        zIndex: 9999,
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'center',
        paddingTop: '12vh',
      }}
    >
      <div
        className="command-palette-dialog"
        onClick={(e) => e.stopPropagation()}
        style={{
          width: '100%',
          maxWidth: '620px',
          background: 'var(--bg-primary, #111317)',
          border: '1px solid var(--border-color, #232730)',
          borderRadius: '14px',
          boxShadow: '0 24px 60px rgba(0, 0, 0, 0.65), 0 0 0 1px rgba(255, 255, 255, 0.08)',
          overflow: 'hidden',
          display: 'flex',
          flexDirection: 'column',
          maxHeight: '70vh',
          animation: 'paletteIn 0.15s cubic-bezier(0.16, 1, 0.3, 1)',
        }}
      >
        {/* Search input header */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            padding: '14px 18px',
            borderBottom: '1px solid var(--border-color, #232730)',
            gap: '12px',
            background: 'var(--bg-secondary, #181a20)',
          }}
        >
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{ color: 'var(--accent, #10b981)', flexShrink: 0 }}
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            placeholder="Type a command, model, chat, or / for slash actions..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            style={{
              flex: 1,
              background: 'transparent',
              border: 'none',
              outline: 'none',
              color: '#f3f4f6',
              fontSize: '1rem',
              fontWeight: 500,
            }}
          />
          <kbd
            style={{
              padding: '2px 7px',
              borderRadius: '4px',
              background: 'rgba(255, 255, 255, 0.08)',
              color: 'var(--text-muted, #9ca3af)',
              fontSize: '0.7rem',
              fontWeight: 600,
              border: '1px solid rgba(255, 255, 255, 0.1)',
            }}
          >
            ESC
          </kbd>
        </div>

        {/* Results List */}
        <div
          ref={listRef}
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: '10px',
            display: 'flex',
            flexDirection: 'column',
            gap: '6px',
          }}
        >
          {filteredItems.length === 0 ? (
            <div style={{ padding: '32px 16px', textAlign: 'center', color: '#9ca3af', fontSize: '0.88rem' }}>
              No matching commands, models, or chats found
            </div>
          ) : (
            Object.entries(grouped).map(([category, items]) => (
              <div key={category} style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
                <div
                  style={{
                    fontSize: '0.68rem',
                    fontWeight: 700,
                    textTransform: 'uppercase',
                    letterSpacing: '0.06em',
                    color: 'var(--text-muted, #6b7280)',
                    padding: '8px 10px 4px 10px',
                  }}
                >
                  {category}
                </div>
                {items.map((item) => {
                  const itemIndex = globalIndex++
                  const isSelected = itemIndex === selectedIndex

                  return (
                    <div
                      key={item.id}
                      className={`palette-item ${isSelected ? 'is-selected' : ''}`}
                      onClick={() => item.action()}
                      onMouseEnter={() => setSelectedIndex(itemIndex)}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        padding: '9px 12px',
                        borderRadius: '8px',
                        cursor: 'pointer',
                        background: isSelected ? 'rgba(16, 185, 129, 0.12)' : 'transparent',
                        border: `1px solid ${isSelected ? 'rgba(16, 185, 129, 0.3)' : 'transparent'}`,
                        transition: 'all 0.1s ease',
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: '10px', minWidth: 0 }}>
                        <span
                          style={{
                            fontSize: '0.68rem',
                            fontWeight: 700,
                            padding: '2px 5px',
                            borderRadius: '4px',
                            background: 'rgba(255, 255, 255, 0.08)',
                            color: isSelected ? '#10b981' : '#94a3b8',
                            fontFamily: 'monospace',
                            flexShrink: 0,
                            minWidth: '22px',
                            textAlign: 'center',
                          }}
                        >
                          {item.icon}
                        </span>
                        <div style={{ display: 'flex', flexDirection: 'column', minWidth: 0 }}>
                          <span
                            style={{
                              fontSize: '0.85rem',
                              fontWeight: 600,
                              color: isSelected ? '#ffffff' : '#e2e8f0',
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              whiteSpace: 'nowrap',
                            }}
                          >
                            {item.title}
                          </span>
                          {item.subtitle && (
                            <span
                              style={{
                                fontSize: '0.72rem',
                                color: '#94a3b8',
                                overflow: 'hidden',
                                textOverflow: 'ellipsis',
                                whiteSpace: 'nowrap',
                              }}
                            >
                              {item.subtitle}
                            </span>
                          )}
                        </div>
                      </div>

                      {isSelected && (
                        <span
                          style={{
                            fontSize: '0.7rem',
                            color: '#10b981',
                            fontWeight: 600,
                            padding: '2px 6px',
                            borderRadius: '4px',
                            background: 'rgba(16, 185, 129, 0.15)',
                          }}
                        >
                          ↵ Select
                        </span>
                      )}
                    </div>
                  )
                })}
              </div>
            ))
          )}
        </div>

        {/* Footer shortcuts */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '8px 16px',
            borderTop: '1px solid var(--border-color, #232730)',
            background: 'var(--bg-secondary, #181a20)',
            fontSize: '0.72rem',
            color: '#94a3b8',
          }}
        >
          <div style={{ display: 'flex', gap: '12px' }}>
            <span><kbd style={{ background: 'rgba(255,255,255,0.1)', padding: '1px 4px', borderRadius: '3px' }}>↑</kbd> <kbd style={{ background: 'rgba(255,255,255,0.1)', padding: '1px 4px', borderRadius: '3px' }}>↓</kbd> to navigate</span>
            <span><kbd style={{ background: 'rgba(255,255,255,0.1)', padding: '1px 4px', borderRadius: '3px' }}>↵</kbd> to select</span>
          </div>
          <span>Universal Command Palette</span>
        </div>
      </div>
    </div>
  )
}
