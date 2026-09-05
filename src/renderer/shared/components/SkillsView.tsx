import React, { useState, useEffect } from 'react'
import { renderSkillsSvgIcon, renderSkillLogoIcon } from '../constants/plugins'
import { renderFormattedMessage } from '../utils/markdown'
import '../css/management-views.css'
import type { LearnedSkill } from '../types'

/** First non-empty line of a skill's content, for the one-line row preview. */
function firstLine(text: string): string {
  return text.split('\n').map((l) => l.replace(/^#+\s*/, '').trim()).find(Boolean) || ''
}

export type { LearnedSkill }

export interface SkillsViewProps {
  listSkills: (workspacePath?: string) => Promise<LearnedSkill[]>
  addSkill: (name: string, content: string) => Promise<any>
  deleteSkill: (name: string) => Promise<any>
  workspacePath?: string
}

export const SkillsView: React.FC<SkillsViewProps> = React.memo(function SkillsView({
  listSkills,
  addSkill,
  deleteSkill,
  workspacePath,
}) {
  const [skills, setSkills] = useState<LearnedSkill[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [searchQuery, setSearchQuery] = useState('')
  const [scopeFilter, setScopeFilter] = useState<'all' | 'workspace' | 'global'>('all')

  // Teach New Skill Form State
  const [newSkillName, setNewSkillName] = useState('')
  const [newSkillContent, setNewSkillContent] = useState('')
  const [adding, setAdding] = useState(false)
  const [showTeachModal, setShowTeachModal] = useState(false)
  const [detailSkill, setDetailSkill] = useState<LearnedSkill | null>(null)

  const fetchSkills = async () => {
    setLoading(true)
    setError('')
    try {
      const list = await listSkills(workspacePath)
      setSkills(list)
    } catch (err: any) {
      console.error('Failed to fetch skills:', err)
      setError('Failed to load learned skills')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchSkills()
  }, [workspacePath])

  const handleAddSkill = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!newSkillName.trim() || !newSkillContent.trim()) return
    setAdding(true)
    try {
      await addSkill(newSkillName.trim(), newSkillContent.trim())
      setNewSkillName('')
      setNewSkillContent('')
      setShowTeachModal(false)
      fetchSkills()
    } catch (err: any) {
      console.error('Failed to add learned skill:', err)
      alert('Error saving skill')
    } finally {
      setAdding(false)
    }
  }

  const handleDeleteSkill = async (name: string) => {
    if (!window.confirm(`Are you sure you want to forget skill "${name}"?`)) return
    try {
      await deleteSkill(name)
      setDetailSkill((current) => (current?.name === name ? null : current))
      fetchSkills()
    } catch (err: any) {
      console.error('Failed to delete skill:', err)
      alert('Error deleting skill')
    }
  }

  const filteredSkills = skills.filter((s) => {
    const matchesSearch =
      s.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (s.description || '').toLowerCase().includes(searchQuery.toLowerCase()) ||
      s.content.toLowerCase().includes(searchQuery.toLowerCase())

    if (!matchesSearch) return false

    if (scopeFilter === 'workspace') return Boolean(s.is_workspace)
    if (scopeFilter === 'global') return !s.is_workspace
    return true
  })

  return (
    <div className="management-container">
      {/* Header */}
      <div className="management-header">
        <div className="management-title-group">
          <h1 className="management-title">
            <span className="management-title-icon" style={{ display: 'inline-flex', alignItems: 'center' }}>
              {renderSkillsSvgIcon(22, 'var(--accent)')}
            </span>
            Skills
          </h1>
          <p className="management-subtitle">
            Instructions Mint follows automatically, reviewed before every prompt.
          </p>
        </div>
        <button
          type="button"
          className="management-primary-btn"
          onClick={() => setShowTeachModal(true)}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
          Teach New Skill
        </button>
      </div>

      {/* Filter and Search Bar */}
      <div className="management-control-bar">
        <div className="management-search-wrapper">
          <input
            type="text"
            className="management-search-input"
            placeholder="Search skills by name or instructions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          <svg
            className="management-search-icon"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
        </div>

        {/* Scope Pill Filter */}
        <div className="management-filter-pills">
          {(['all', 'workspace', 'global'] as const).map((sc) => (
            <button
              key={sc}
              type="button"
              className={`management-pill-btn ${scopeFilter === sc ? 'active' : ''}`}
              onClick={() => setScopeFilter(sc)}
            >
              {sc}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <div className="management-error-banner">
          {error}
        </div>
      )}

      {/* Skills list */}
      {loading ? (
        <div className="mgmt-empty">Loading…</div>
      ) : filteredSkills.length === 0 ? (
        <div className="mgmt-empty">
          <p>{searchQuery || scopeFilter !== 'all' ? 'No skills match.' : 'No skills yet.'}</p>
          {!searchQuery && scopeFilter === 'all' && (
            <p>
              Teach one above, or run <code>/learn</code> in chat.
            </p>
          )}
        </div>
      ) : (
        <div className="mgmt-list">
          {filteredSkills.map((s) => (
            <div key={s.name} className="mgmt-row" onClick={() => setDetailSkill(s)}>
              <div className="mgmt-row-main">
                <div className="mgmt-row-title">{s.name}</div>
                <div className="mgmt-row-sub">{s.description || firstLine(s.content)}</div>
              </div>
              <span className={`management-tag ${s.is_workspace ? 'workspace' : 'global'}`}>
                {s.is_workspace ? 'Workspace' : 'Global'}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Skill Detail */}
      {detailSkill && (
        <div className="management-modal-overlay" onClick={() => setDetailSkill(null)}>
          <div className="management-modal" onClick={(e) => e.stopPropagation()}>
            <div className="management-modal-header">
              <div className="management-card-title-group">
                {renderSkillLogoIcon(44)}
                <h2 className="management-modal-title">{detailSkill.name}</h2>
              </div>
              <button
                type="button"
                className="management-modal-close"
                onClick={() => setDetailSkill(null)}
              >
                ✕
              </button>
            </div>

            <div className="management-modal-body">
              <span className={`management-tag ${detailSkill.is_workspace ? 'workspace' : 'global'}`}>
                {detailSkill.is_workspace ? 'Workspace' : 'Global'}
              </span>

              {detailSkill.description && (
                <p style={{ color: 'var(--text-soft, #d1d1d4)', lineHeight: 1.55, margin: 0 }}>
                  {detailSkill.description}
                </p>
              )}

              <div className="mgmt-prose">{renderFormattedMessage(detailSkill.content)}</div>
            </div>

            <div className="management-modal-footer">
              <span style={{ fontSize: '0.8rem', color: 'var(--text-muted, #64748b)' }}>
                {detailSkill.updatedAt ? new Date(detailSkill.updatedAt).toLocaleDateString() : 'Learned'}
              </span>
              <button
                type="button"
                className="management-action-btn danger"
                onClick={() => handleDeleteSkill(detailSkill.name)}
              >
                Forget
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Teach New Skill Modal */}
      {showTeachModal && (
        <div className="management-modal-overlay">
          <div className="management-modal">
            <div className="management-modal-header">
              <h2 className="management-modal-title">
                Teach New Skill to Mint
              </h2>
              <button
                type="button"
                className="management-modal-close"
                onClick={() => setShowTeachModal(false)}
              >
                ✕
              </button>
            </div>

            <form onSubmit={handleAddSkill}>
              <div className="management-modal-body">
                <div className="management-form-group">
                  <label className="management-label">
                    Skill Name (e.g. typescript-standards)
                  </label>
                  <input
                    type="text"
                    className="management-input-field"
                    placeholder="e.g. angular-standard"
                    value={newSkillName}
                    onChange={(e) => setNewSkillName(e.target.value)}
                    required
                  />
                </div>

                <div className="management-form-group">
                  <label className="management-label">
                    Skill Instructions / Content (Markdown supported)
                  </label>
                  <textarea
                    className="management-textarea-field"
                    placeholder="# Guidelines&#10;Always write modular React code using TypeScript."
                    value={newSkillContent}
                    onChange={(e) => setNewSkillContent(e.target.value)}
                    required
                    rows={6}
                  />
                </div>
              </div>

              <div className="management-modal-footer">
                <button
                  type="button"
                  className="management-action-btn"
                  onClick={() => setShowTeachModal(false)}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={adding}
                  className="management-primary-btn"
                >
                  {adding ? 'Saving...' : 'Save Skill'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  )
})

export default SkillsView
