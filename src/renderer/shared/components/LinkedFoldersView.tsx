import React, { useState, useEffect } from 'react'
import { renderLinkedFoldersSvgIcon } from '../constants/plugins'
import '../css/management-views.css'
import type { LinkedFolder, LinkedFolderDraft } from '../types'

export type { LinkedFolder }

export interface LinkedFoldersViewProps {
  listLinkedFolders: () => Promise<Record<string, LinkedFolder>>
  addLinkedFolder: (draft: LinkedFolderDraft) => Promise<any>
  removeLinkedFolder: (name: string) => Promise<any>
  /** Opens a native folder picker and resolves the chosen path, or null if
   * cancelled. Only passed in on the desktop app — browsers can't expose a
   * real filesystem path from a folder picker, so omitting this prop hides
   * the Browse button entirely on the web build. */
  selectFolder?: () => Promise<string | null>
}

export const LinkedFoldersView: React.FC<LinkedFoldersViewProps> = React.memo(
  function LinkedFoldersView({ listLinkedFolders, addLinkedFolder, removeLinkedFolder, selectFolder }) {
    const [folders, setFolders] = useState<LinkedFolder[]>([])
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState('')
    const [searchQuery, setSearchQuery] = useState('')

    const [newName, setNewName] = useState('')
    const [newPath, setNewPath] = useState('')
    const [newDescription, setNewDescription] = useState('')
    const [adding, setAdding] = useState(false)
    const [showAddModal, setShowAddModal] = useState(false)

    const fetchFolders = async () => {
      setLoading(true)
      setError('')
      try {
        const map = await listLinkedFolders()
        setFolders(Object.values(map || {}))
      } catch (err: any) {
        console.error('Failed to fetch linked folders:', err)
        setError('Failed to load linked folders')
      } finally {
        setLoading(false)
      }
    }

    useEffect(() => {
      fetchFolders()
    }, [])

    const handleAddFolder = async (e: React.FormEvent) => {
      e.preventDefault()
      if (!newName.trim() || !newPath.trim()) return
      setAdding(true)
      try {
        await addLinkedFolder({
          name: newName.trim(),
          path: newPath.trim(),
          description: newDescription.trim() || undefined
        })
        setNewName('')
        setNewPath('')
        setNewDescription('')
        setShowAddModal(false)
        fetchFolders()
      } catch (err: any) {
        console.error('Failed to add linked folder:', err)
        alert(err?.message || 'Error linking folder. Check the path is allowed and exists.')
      } finally {
        setAdding(false)
      }
    }

    const handleBrowse = async () => {
      if (!selectFolder) return
      try {
        const picked = await selectFolder()
        if (picked) setNewPath(picked)
      } catch (err: any) {
        console.error('Failed to open folder picker:', err)
      }
    }

    const handleRemoveFolder = async (name: string) => {
      if (!window.confirm(`Unlink folder "${name}"? (The folder and its notes are not deleted.)`)) return
      try {
        await removeLinkedFolder(name)
        fetchFolders()
      } catch (err: any) {
        console.error('Failed to remove linked folder:', err)
        alert('Error unlinking folder')
      }
    }

    const filteredFolders = folders.filter((folder) => {
      const q = searchQuery.toLowerCase()
      return (
        folder.name.toLowerCase().includes(q) ||
        folder.path.toLowerCase().includes(q) ||
        (folder.description || '').toLowerCase().includes(q)
      )
    })

    return (
      <div className="management-container">
        <div className="management-header">
          <div className="management-title-group">
            <h1 className="management-title">
              <span className="management-title-icon" style={{ display: 'inline-flex', alignItems: 'center' }}>
                {renderLinkedFoldersSvgIcon(22, 'var(--accent)')}
              </span>
              Linked Folders
            </h1>
            <p className="management-subtitle">
              Folders you've linked to Mint. When chat touches on a linked folder's topic, the agent may write a
              short note into its <code>mint-notes/</code> subfolder automatically.
            </p>
          </div>
          <button type="button" className="management-primary-btn" onClick={() => setShowAddModal(true)}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            Link a Folder
          </button>
        </div>

        <div className="management-control-bar">
          <div className="management-search-wrapper">
            <input
              type="text"
              className="management-search-input"
              placeholder="Search linked folders..."
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
        </div>

        {error && <div className="management-error-banner">{error}</div>}

        {loading ? (
          <div style={{ padding: '40px', textAlign: 'center', color: 'var(--text-muted, #94a3b8)' }}>
            Loading linked folders...
          </div>
        ) : filteredFolders.length === 0 ? (
          <div className="management-empty-state">
            <div className="management-empty-icon">📁</div>
            <h3 className="management-empty-title">No linked folders yet</h3>
            <p className="management-empty-desc">
              Link one above, or use <code>mint link add</code> / <code>/link add</code> in chat.
            </p>
          </div>
        ) : (
          <div className="management-grid">
            {filteredFolders.map((folder) => (
              <div key={folder.name} className="management-card">
                <div>
                  <div className="management-card-header">
                    <span className="management-card-title-badge">{folder.name}</span>
                  </div>

                  <p className="management-card-desc">{folder.description || 'No description'}</p>

                  <div style={{ fontSize: '0.8rem', color: 'var(--text-muted, #94a3b8)', marginTop: '8px' }}>
                    {folder.path}
                  </div>
                </div>

                <div className="management-card-footer">
                  <span />
                  <button
                    type="button"
                    className="management-action-btn danger"
                    onClick={() => handleRemoveFolder(folder.name)}
                  >
                    Unlink
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {showAddModal && (
          <div className="management-modal-overlay">
            <div className="management-modal">
              <div className="management-modal-header">
                <h2 className="management-modal-title">Link a Folder</h2>
                <button type="button" className="management-modal-close" onClick={() => setShowAddModal(false)}>
                  ✕
                </button>
              </div>

              <form onSubmit={handleAddFolder}>
                <div className="management-modal-body">
                  <div className="management-form-group">
                    <label className="management-label">Name</label>
                    <input
                      type="text"
                      className="management-input-field"
                      placeholder="e.g. Food"
                      value={newName}
                      onChange={(e) => setNewName(e.target.value)}
                      required
                    />
                  </div>

                  <div className="management-form-group">
                    <label className="management-label">Folder path</label>
                    <div style={{ display: 'flex', gap: '8px' }}>
                      <input
                        type="text"
                        className="management-input-field"
                        placeholder="~/notes/food"
                        value={newPath}
                        onChange={(e) => setNewPath(e.target.value)}
                        required
                        style={{ flex: 1 }}
                      />
                      {selectFolder && (
                        <button type="button" className="management-action-btn" onClick={handleBrowse}>
                          Browse...
                        </button>
                      )}
                    </div>
                  </div>

                  <div className="management-form-group">
                    <label className="management-label">Description (helps topic matching)</label>
                    <textarea
                      className="management-textarea-field"
                      placeholder="restaurant reviews and recipes"
                      value={newDescription}
                      onChange={(e) => setNewDescription(e.target.value)}
                      rows={3}
                    />
                  </div>
                </div>

                <div className="management-modal-footer">
                  <button type="button" className="management-action-btn" onClick={() => setShowAddModal(false)}>
                    Cancel
                  </button>
                  <button type="submit" disabled={adding} className="management-primary-btn">
                    {adding ? 'Linking...' : 'Link Folder'}
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}
      </div>
    )
  }
)

export default LinkedFoldersView
