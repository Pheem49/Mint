import React, { useState, useEffect } from 'react'
import { listLearnedSkills, addLearnedSkill, deleteLearnedSkill, LearnedSkill } from '../../tauri'

interface MemoryTabProps {
  userName: string
  setUserName: (val: string) => void
  userPreferences: string
  setUserPreferences: (val: string) => void
}

export default function MemoryTab({
  userName,
  setUserName,
  userPreferences,
  setUserPreferences
}: MemoryTabProps) {
  const [skills, setSkills] = useState<LearnedSkill[]>([])
  const [newSkillName, setNewSkillName] = useState('')
  const [newSkillContent, setNewSkillContent] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  // Load skills on mount
  useEffect(() => {
    fetchSkills()
  }, [])

  const fetchSkills = async () => {
    try {
      const list = await listLearnedSkills()
      setSkills(list)
    } catch (e) {
      console.error("Failed to load skills:", e)
    }
  }

  const handleAddSkill = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!newSkillName.trim() || !newSkillContent.trim()) {
      setError('Please fill in both name and content.')
      return
    }

    const cleanName = newSkillName.trim().replace(/\s+/g, '-').toLowerCase()
    setLoading(true)
    setError('')

    try {
      await addLearnedSkill(cleanName, newSkillContent)
      setNewSkillName('')
      setNewSkillContent('')
      await fetchSkills()
    } catch (e: any) {
      setError(e.message || String(e))
    } finally {
      setLoading(false)
    }
  }

  const handleDeleteSkill = async (name: string) => {
    if (confirm(`Forget skill "${name}"?`)) {
      try {
        await deleteLearnedSkill(name)
        await fetchSkills()
      } catch (e) {
        console.error("Failed to delete skill:", e)
      }
    }
  }

  return (
    <div className="tab-pane active animate-fade-in" style={{ display: 'flex', flexDirection: 'column', gap: '24px' }}>
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Cross-session Memory</p>
            <h2 className="section-title">User Profile & Preferences</h2>
          </div>
          <p className="section-description">
            Information and preferences stored here will be remembered across all conversations. Mint automatically learns and updates your profile details from your chats in the background, but you can also edit them manually.
          </p>
        </div>

        <div className="form-grid single">
          <div className="setting-row">
            <label>Your Name / Nickname</label>
            <div className="memory-field-container">
              <input 
                type="text" 
                value={userName} 
                onChange={(e) => setUserName(e.target.value)} 
                placeholder="e.g. Pheem, Jane" 
              />
              <p className="hint">Used by the assistant to address you in conversation.</p>
            </div>
          </div>

          <div className="setting-row">
            <label>Custom Instructions & Preferences</label>
            <div className="memory-field-container">
              <textarea 
                value={userPreferences} 
                onChange={(e) => setUserPreferences(e.target.value)} 
                placeholder="e.g. Explain coding concepts step-by-step. Prefer TypeScript. Talk in Thai. Keep explanations concise." 
                style={{ minHeight: '120px', resize: 'vertical' }}
              />
              <p className="hint">Preferences, guidelines, or persona instructions for the assistant.</p>
            </div>
          </div>
        </div>
      </section>

      <section className="setting-section" style={{ borderTop: '1px solid var(--border)', paddingTop: '24px' }}>
        <div className="section-heading">
          <div>
            <p className="section-kicker">Knowledge Base</p>
            <h2 className="section-title">Learned AI Skills</h2>
          </div>
          <p className="section-description">
            Skills are special instructions or guides taught to Mint (equivalent to <code>mint learn</code> in CLI). The AI reads active skills before every prompt to align with your guidelines.
          </p>
        </div>

        {skills.length === 0 ? (
          <div className="empty-skills-notice" style={{
            padding: '24px',
            border: '1px dashed var(--border)',
            borderRadius: '12px',
            textAlign: 'center',
            color: 'var(--text-muted)',
            background: 'rgba(0,0,0,0.06)',
            fontSize: '0.9rem',
            marginBottom: '20px'
          }}>
            No learned skills found. Teach Mint a skill below or run <code>/learn &lt;path&gt;</code> in chat!
          </div>
        ) : (
          <div className="skills-list" style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginBottom: '20px' }}>
            {skills.map((s) => (
              <div className="skill-card" key={s.id} style={{
                display: 'flex',
                alignItems: 'flex-start',
                justifyContent: 'space-between',
                padding: '16px 20px',
                border: '1px solid var(--border)',
                borderRadius: '12px',
                background: 'var(--surface-bg)',
                gap: '16px'
              }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '6px' }}>
                    <span className="badge" style={{
                      background: 'var(--accent)',
                      color: '#fff',
                      fontSize: '0.75rem',
                      fontWeight: '600',
                      padding: '2px 8px',
                      borderRadius: '4px'
                    }}>{s.name}</span>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-muted)', textOverflow: 'ellipsis', overflow: 'hidden', whiteSpace: 'nowrap' }}>
                      Source: {s.sourcePath}
                    </span>
                  </div>
                  <div style={{
                    fontSize: '0.85rem',
                    color: 'var(--text-main)',
                    whiteSpace: 'pre-wrap',
                    maxHeight: '120px',
                    overflowY: 'auto',
                    background: 'rgba(0,0,0,0.12)',
                    padding: '10px 14px',
                    borderRadius: '8px',
                    fontFamily: 'monospace',
                    lineHeight: '1.4'
                  }}>
                    {s.content}
                  </div>
                </div>
                <button 
                  className="btn btn-danger" 
                  onClick={() => handleDeleteSkill(s.name)}
                  style={{ padding: '6px 12px', fontSize: '0.8rem', height: '34px', flexShrink: 0 }}
                >
                  Forget
                </button>
              </div>
            ))}
          </div>
        )}

        <form className="add-skill-box" onSubmit={handleAddSkill} style={{
          padding: '20px',
          border: '1px solid var(--border)',
          borderRadius: '12px',
          background: 'var(--surface-bg)'
        }}>
          <h3 style={{ fontSize: '1rem', fontWeight: '600', color: 'var(--text-main)', marginBottom: '14px' }}>Teach New Skill</h3>
          {error && <div style={{ color: '#ef4444', fontSize: '0.85rem', marginBottom: '12px', fontWeight: '500' }}>{error}</div>}
          
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '12px' }}>
            <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Skill Name (e.g. coding-guidelines)</label>
            <input
              type="text"
              placeholder="e.g. angular-standard"
              value={newSkillName}
              onChange={(e) => setNewSkillName(e.target.value)}
              style={{ padding: '10px 14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.9rem' }}
            />
          </div>
          
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '14px' }}>
            <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Skill Instructions / Content</label>
            <textarea
              placeholder="# Instructions&#10;Write only clean TypeScript. Use async/await. Avoid let where const is possible."
              value={newSkillContent}
              onChange={(e) => setNewSkillContent(e.target.value)}
              style={{ padding: '10px 14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--input-bg)', color: 'var(--text-main)', fontSize: '0.9rem', width: '100%', height: '100px', resize: 'vertical' }}
            />
          </div>

          <button 
            type="submit" 
            className="btn-primary" 
            disabled={loading}
            style={{ width: '100%', padding: '10px 16px', borderRadius: '8px', border: 'none', background: 'var(--accent)', color: '#fff', fontWeight: '600', cursor: 'pointer' }}
          >
            {loading ? 'Learning...' : 'Teach Skill'}
          </button>
        </form>
      </section>
    </div>
  )
}
