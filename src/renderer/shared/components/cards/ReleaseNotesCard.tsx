import React, { useMemo } from 'react'
import { Rocket } from 'lucide-react'

export interface ReleaseNotesCardProps {
  markdown: string
}

interface ReleaseSection {
  title: string
  items: { title: string; desc: string; subItems?: string[] }[]
}

export function ReleaseNotesCard({ markdown }: ReleaseNotesCardProps) {
  const { version, sections } = useMemo(() => {
    let version = 'Latest'
    const sections: ReleaseSection[] = []
    let currentSection: ReleaseSection | null = null
    let currentItem: { title: string; desc: string; subItems?: string[] } | null = null

    for (const rawLine of markdown.split('\n')) {
      const line = rawLine.trim()
      if (!line) continue

      if (line.startsWith('# ')) {
        const vMatch = line.match(/v\d+\.\d+(?:\.\d+)?/i)
        if (vMatch) version = vMatch[0]
        continue
      }

      if (line.startsWith('## ')) {
        if (currentItem && currentSection) {
          currentSection.items.push(currentItem)
          currentItem = null
        }
        if (currentSection) {
          sections.push(currentSection)
        }
        currentSection = {
          title: line.replace(/^##\s+/, '').trim(),
          items: [],
        }
        continue
      }

      const indent = rawLine.length - rawLine.trimStart().length
      if (line.startsWith('- ') || line.startsWith('* ')) {
        const content = line.slice(2).trim()
        if (indent >= 2 && currentItem) {
          if (!currentItem.subItems) currentItem.subItems = []
          currentItem.subItems.push(content)
          continue
        }
        if (currentItem && currentSection) {
          currentSection.items.push(currentItem)
        }
        const boldMatch = content.match(/^\*\*([^*]+)\*\*:\s*(.*)$/)
        if (boldMatch) {
          currentItem = {
            title: boldMatch[1],
            desc: boldMatch[2],
          }
        } else {
          currentItem = {
            title: '',
            desc: content,
          }
        }
        continue
      }

      if (currentItem) {
        currentItem.desc += ' ' + line
      }
    }

    if (currentItem && currentSection) {
      currentSection.items.push(currentItem)
    }
    if (currentSection) {
      sections.push(currentSection)
    }

    return { version, sections }
  }, [markdown])

  return (
    <div className="release-notes-card">
      <div className="release-notes-header">
        <div className="release-notes-title">
          <Rocket size={18} className="release-notes-icon" />
          <span>Release Notes</span>
          <span className="release-notes-version-badge">{version}</span>
        </div>
        <span className="release-notes-count-badge">
          {sections.length} Highlights
        </span>
      </div>
      <div className="release-notes-track">
        <div className="release-notes-bar" />
      </div>
      <div className="release-notes-body">
        {sections.map((section, sIdx) => (
          <div key={sIdx} className="release-notes-section">
            <div className="release-notes-section-title">
              <span className="release-notes-bullet">●</span>
              <span>{section.title}</span>
            </div>
            <div className="release-notes-items">
              {section.items.map((item, iIdx) => (
                <div key={iIdx} className="release-notes-item">
                  <div className="release-notes-item-header">
                    <span className="release-notes-item-bullet">•</span>
                    {item.title ? (
                      <span className="release-notes-item-title">{item.title}:</span>
                    ) : null}
                    <span className="release-notes-item-desc">{item.desc}</span>
                  </div>
                  {item.subItems && item.subItems.length > 0 && (
                    <div className="release-notes-subitems">
                      {item.subItems.map((sub, subIdx) => (
                        <div key={subIdx} className="release-notes-subitem">
                          <span className="release-notes-subitem-bullet">›</span>
                          <span>{sub.replace(/\*\*/g, '')}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
export default ReleaseNotesCard
