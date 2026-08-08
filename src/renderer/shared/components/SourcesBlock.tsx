import React, { useState, useMemo } from 'react'
import type { WebSearchSource } from '../utils/agentActivity'

export interface SourcesBlockProps {
  sources: WebSearchSource[]
}

export interface DomainGroup {
  domain: string
  displayDomain: string
  faviconUrl: string
  items: WebSearchSource[]
}

/**
 * Groups web search sources by their domain name.
 * e.g., multiple results from `tmd.go.th` will be combined into 1 DomainGroup.
 */
export function groupSourcesByDomain(sources: WebSearchSource[]): DomainGroup[] {
  const groupsMap = new Map<string, DomainGroup>()

  for (const src of sources) {
    const domainKey = src.domain.toLowerCase().trim()
    if (!groupsMap.has(domainKey)) {
      // Format display domain (e.g., facebook.com -> facebook, instagram.com -> instagram)
      let displayDomain = domainKey
      if (/^[^.]+\.com$/i.test(displayDomain)) {
        displayDomain = displayDomain.replace(/\.com$/i, '')
      } else if (/^[^.]+\.org$/i.test(displayDomain)) {
        displayDomain = displayDomain.replace(/\.org$/i, '')
      } else if (/^[^.]+\.net$/i.test(displayDomain)) {
        displayDomain = displayDomain.replace(/\.net$/i, '')
      }

      groupsMap.set(domainKey, {
        domain: src.domain,
        displayDomain,
        faviconUrl: src.faviconUrl,
        items: [src],
      })
    } else {
      groupsMap.get(domainKey)!.items.push(src)
    }
  }

  return Array.from(groupsMap.values())
}

export const SourcesBlock: React.FC<SourcesBlockProps> = React.memo(function SourcesBlock({ sources }) {
  const [isExpanded, setIsExpanded] = useState(false)
  const [openDropdownDomain, setOpenDropdownDomain] = useState<string | null>(null)

  const domainGroups = useMemo(() => {
    const groups = groupSourcesByDomain(sources)
    // Show groups whose card will render a thumbnail first (stable sort keeps
    // original ordering within each bucket).
    return [...groups].sort((a, b) => {
      const aHasImage = a.items[0]?.imageUrl ? 1 : 0
      const bHasImage = b.items[0]?.imageUrl ? 1 : 0
      return bHasImage - aHasImage
    })
  }, [sources])

  if (!sources || sources.length === 0) return null

  const MAX_INITIAL_CARDS = 3

  const visibleGroups = isExpanded ? domainGroups : domainGroups.slice(0, MAX_INITIAL_CARDS)

  // Calculate remaining total sources count for "View X more" badge
  const shownSourcesCount = domainGroups
    .slice(0, MAX_INITIAL_CARDS)
    .reduce((sum, g) => sum + g.items.length, 0)
  const remainingSourcesCount = sources.length - shownSourcesCount
  const hiddenGroups = domainGroups.slice(MAX_INITIAL_CARDS)

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginBottom: '12px', marginTop: '4px' }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          style={{ color: '#94a3b8' }}
        >
          <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" />
        </svg>
        <span style={{ fontSize: '0.85rem', fontWeight: 600, color: '#f1f5f9', letterSpacing: '0.01em' }}>
          Sources
        </span>
      </div>

      {/* Domain Cards Container (wraps to next line if exceeding frame width) */}
      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          gap: '8px',
        }}
      >
        {visibleGroups.map((group, groupIdx) => {
          const primaryItem = group.items[0]
          const extraCount = group.items.length - 1
          const isDropdownOpen = openDropdownDomain === group.domain

          return (
            <div
              key={group.domain}
              style={{
                position: 'relative',
                flex: '1 1 180px',
                maxWidth: '240px',
                minWidth: '160px',
                boxSizing: 'border-box',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  justifyContent: 'space-between',
                  borderRadius: '10px',
                  background: 'rgba(255, 255, 255, 0.04)',
                  border: '1px solid rgba(255, 255, 255, 0.08)',
                  transition: 'all 0.15s ease',
                  cursor: 'pointer',
                  minHeight: '68px',
                  height: '100%',
                  boxSizing: 'border-box',
                  overflow: 'hidden',
                }}
                onClick={(e) => {
                  // If extra items exist and click was on the badge or dropdown toggle, toggle dropdown
                  const target = e.target as HTMLElement
                  if (extraCount > 0 && target.closest('.extra-badge-btn')) {
                    e.stopPropagation()
                    setOpenDropdownDomain(isDropdownOpen ? null : group.domain)
                    return
                  }
                  // Otherwise open primary link
                  window.open(primaryItem.url, '_blank', 'noopener,noreferrer')
                }}
                onMouseEnter={(e) => {
                  const el = e.currentTarget
                  el.style.background = 'rgba(255, 255, 255, 0.07)'
                  el.style.borderColor = 'rgba(255, 255, 255, 0.16)'
                  el.style.transform = 'translateY(-1px)'
                }}
                onMouseLeave={(e) => {
                  const el = e.currentTarget
                  el.style.background = 'rgba(255, 255, 255, 0.04)'
                  el.style.borderColor = 'rgba(255, 255, 255, 0.08)'
                  el.style.transform = 'translateY(0)'
                }}
              >
                {/* Thumbnail (if this source has an associated image) */}
                {primaryItem.imageUrl && (
                  <div style={{ width: '100%', height: '80px', overflow: 'hidden', background: 'rgba(255,255,255,0.06)', flexShrink: 0 }}>
                    <img
                      src={primaryItem.imageUrl}
                      alt={primaryItem.title}
                      loading="lazy"
                      style={{ width: '100%', height: '100%', objectFit: 'cover', display: 'block' }}
                      onError={(e) => { (e.currentTarget as HTMLImageElement).parentElement!.style.display = 'none' }}
                    />
                  </div>
                )}

                <div style={{ display: 'flex', flexDirection: 'column', flex: 1, padding: '10px 12px' }}>
                  {/* Title */}
                  <div
                    title={primaryItem.title}
                    style={{
                      fontSize: '0.8rem',
                      fontWeight: 500,
                      color: '#f1f5f9',
                      lineHeight: '1.3',
                      display: '-webkit-box',
                      WebkitLineClamp: 2,
                      WebkitBoxOrient: 'vertical',
                      overflow: 'hidden',
                      marginBottom: '8px',
                      wordBreak: 'break-word',
                    }}
                  >
                    {primaryItem.title}
                  </div>

                  {/* Bottom Row */}
                  <div
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      marginTop: 'auto',
                      paddingTop: '2px',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px', overflow: 'hidden', marginRight: '6px' }}>
                      <img
                        src={group.faviconUrl}
                        alt=""
                        width={14}
                        height={14}
                        style={{ borderRadius: '3px', flexShrink: 0 }}
                        onError={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none' }}
                      />
                      <span
                        style={{
                          fontSize: '0.73rem',
                          color: '#94a3b8',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {group.displayDomain}
                      </span>
                    </div>

                    {extraCount > 0 ? (
                      <button
                        type="button"
                        className="extra-badge-btn"
                        title={`${extraCount} extra page${extraCount > 1 ? 's' : ''} from ${group.domain} — click to view`}
                        style={{
                          fontSize: '0.68rem',
                          fontWeight: 600,
                          color: '#38bdf8',
                          background: 'rgba(56, 189, 248, 0.12)',
                          border: '1px solid rgba(56, 189, 248, 0.3)',
                          padding: '1px 6px',
                          borderRadius: '4px',
                          flexShrink: 0,
                          cursor: 'pointer',
                          display: 'flex',
                          alignItems: 'center',
                          gap: '2px',
                        }}
                        onClick={(e) => {
                          e.stopPropagation()
                          setOpenDropdownDomain(isDropdownOpen ? null : group.domain)
                        }}
                      >
                        +{extraCount}
                      </button>
                    ) : (
                      <span style={{ fontSize: '0.7rem', color: '#64748b', fontWeight: 500, flexShrink: 0 }}>
                        •{groupIdx + 1}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Sub-links Popover Dropdown when multi-item domain card clicked */}
              {isDropdownOpen && extraCount > 0 && (
                <div
                  style={{
                    position: 'absolute',
                    top: '100%',
                    left: 0,
                    width: '260px',
                    marginTop: '6px',
                    background: '#18181b',
                    border: '1px solid rgba(255, 255, 255, 0.15)',
                    borderRadius: '8px',
                    boxShadow: '0 10px 25px rgba(0,0,0,0.6)',
                    zIndex: 40,
                    padding: '6px',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '4px',
                  }}
                >
                  <div
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      padding: '4px 6px',
                      borderBottom: '1px solid rgba(255,255,255,0.06)',
                    }}
                  >
                    <span style={{ fontSize: '0.7rem', fontWeight: 600, color: '#94a3b8' }}>
                      All links from {group.domain} ({group.items.length})
                    </span>
                    <button
                      type="button"
                      style={{
                        background: 'none',
                        border: 'none',
                        color: '#64748b',
                        cursor: 'pointer',
                        fontSize: '0.8rem',
                        padding: '0 2px',
                      }}
                      onClick={() => setOpenDropdownDomain(null)}
                    >
                      ✕
                    </button>
                  </div>
                  {group.items.map((item, itemIdx) => (
                    <a
                      key={itemIdx}
                      href={item.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      style={{
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '2px',
                        padding: '6px 8px',
                        borderRadius: '6px',
                        color: '#e2e8f0',
                        textDecoration: 'none',
                        fontSize: '0.75rem',
                        background: 'rgba(255,255,255,0.02)',
                        transition: 'background 0.15s',
                      }}
                      onMouseEnter={(e) => { (e.currentTarget as HTMLAnchorElement).style.background = 'rgba(255,255,255,0.08)' }}
                      onMouseLeave={(e) => { (e.currentTarget as HTMLAnchorElement).style.background = 'rgba(255,255,255,0.02)' }}
                    >
                      <div style={{ fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {itemIdx + 1}. {item.title}
                      </div>
                      <div style={{ fontSize: '0.68rem', color: '#64748b', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {item.url}
                      </div>
                    </a>
                  ))}
                </div>
              )}
            </div>
          )
        })}

        {/* Overflow "View X more" Card */}
        {!isExpanded && remainingSourcesCount > 0 && (
          <div
            onClick={() => setIsExpanded(true)}
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '10px 14px',
              borderRadius: '10px',
              background: 'rgba(255, 255, 255, 0.03)',
              border: '1px solid rgba(255, 255, 255, 0.08)',
              cursor: 'pointer',
              transition: 'all 0.15s ease',
              minHeight: '68px',
              height: '100%',
              flex: '1 1 160px',
              maxWidth: '200px',
              boxSizing: 'border-box',
            }}
            onMouseEnter={(e) => {
              const el = e.currentTarget
              el.style.background = 'rgba(255, 255, 255, 0.06)'
              el.style.borderColor = 'rgba(255, 255, 255, 0.16)'
              el.style.transform = 'translateY(-1px)'
            }}
            onMouseLeave={(e) => {
              const el = e.currentTarget
              el.style.background = 'rgba(255, 255, 255, 0.03)'
              el.style.borderColor = 'rgba(255, 255, 255, 0.08)'
              el.style.transform = 'translateY(0)'
            }}
          >
            {/* Stacked Favicons */}
            <div style={{ display: 'flex', alignItems: 'center' }}>
              {hiddenGroups.slice(0, 3).map((grp, idx) => (
                <img
                  key={idx}
                  src={grp.faviconUrl}
                  alt=""
                  width={18}
                  height={18}
                  style={{
                    borderRadius: '50%',
                    border: '1.5px solid #18181b',
                    marginLeft: idx === 0 ? 0 : '-6px',
                    background: '#27272a',
                    objectFit: 'cover',
                  }}
                  onError={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none' }}
                />
              ))}
            </div>

            <span style={{ fontSize: '0.78rem', fontWeight: 500, color: '#94a3b8', marginLeft: '8px' }}>
              View {remainingSourcesCount} more
            </span>
          </div>
        )}
      </div>

      {/* Collapse button when expanded */}
      {isExpanded && domainGroups.length > MAX_INITIAL_CARDS && (
        <div style={{ display: 'flex', justifyContent: 'flex-start', marginTop: '4px' }}>
          <button
            type="button"
            onClick={() => {
              setIsExpanded(false)
              setOpenDropdownDomain(null)
            }}
            style={{
              background: 'rgba(255,255,255,0.05)',
              border: '1px solid rgba(255,255,255,0.1)',
              borderRadius: '6px',
              padding: '4px 10px',
              color: '#94a3b8',
              fontSize: '0.73rem',
              fontWeight: 500,
              cursor: 'pointer',
              transition: 'background 0.15s',
            }}
            onMouseEnter={(e) => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.1)' }}
            onMouseLeave={(e) => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.05)' }}
          >
            Show less
          </button>
        </div>
      )}
    </div>
  )
})

export default SourcesBlock
