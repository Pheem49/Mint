import React, { useState, useEffect, useRef } from 'react'
import { renderLinkedFoldersSvgIcon, renderFolderIcon } from '../constants/plugins'
import '../css/management-views.css'
import type { LinkedFolder, LinkedFolderDraft, LinkedFolderNote, LinkedFolderStatus } from '../types'

export type { LinkedFolder }

export interface LinkedFoldersViewProps {
  listLinkedFolders: () => Promise<Record<string, LinkedFolder>>
  addLinkedFolder: (draft: LinkedFolderDraft) => Promise<any>
  removeLinkedFolder: (name: string) => Promise<any>
  linkedFolderStatus: (name: string) => Promise<LinkedFolderStatus>
  refreshLinkedFolder: (name: string) => Promise<LinkedFolderStatus>
  listLinkedFolderNotes: (name: string) => Promise<LinkedFolderNote[]>
  readLinkedFolderNote: (name: string, id: string) => Promise<string>
  openLinkedFolderNote: (name: string, id: string) => Promise<void>
  canOpenNote: boolean
  /** Opens a native folder picker and resolves the chosen path, or null if
   * cancelled. Only passed in on the desktop app — browsers can't expose a
   * real filesystem path from a folder picker, so omitting this prop hides
   * the Browse button entirely on the web build. */
  selectFolder?: () => Promise<string | null>
}

export const LinkedFoldersView: React.FC<LinkedFoldersViewProps> = React.memo(
  function LinkedFoldersView({ listLinkedFolders, addLinkedFolder, removeLinkedFolder, linkedFolderStatus, refreshLinkedFolder, listLinkedFolderNotes, readLinkedFolderNote, openLinkedFolderNote, canOpenNote, selectFolder }) {
    const [folders, setFolders] = useState<LinkedFolder[]>([])
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState('')
    const [searchQuery, setSearchQuery] = useState('')

    const [newName, setNewName] = useState('')
    const [newPath, setNewPath] = useState('')
    const [newDescription, setNewDescription] = useState('')
    const [adding, setAdding] = useState(false)
    const [showAddModal, setShowAddModal] = useState(false)
    const [formErrors, setFormErrors] = useState<{ name?: string; path?: string; form?: string }>({})
    const [nameFromPicker, setNameFromPicker] = useState(false)
    const nameInputRef = useRef<HTMLInputElement>(null)
    const addOpenerRef = useRef<HTMLElement | null>(null)
    const addDialogRef = useRef<HTMLDivElement>(null)
    const detailOpenerRef = useRef<HTMLElement | null>(null)
    const detailCloseRef = useRef<HTMLButtonElement>(null)
    const detailDialogRef = useRef<HTMLDivElement>(null)
    const [detailFolder, setDetailFolder] = useState<LinkedFolder | null>(null)
    const [status, setStatus] = useState<LinkedFolderStatus | null>(null)
    const [notes, setNotes] = useState<LinkedFolderNote[]>([])
    const [preview, setPreview] = useState('')
    const [previewNoteId, setPreviewNoteId] = useState<string | null>(null)
    const [previewLoading, setPreviewLoading] = useState(false)
    const previewRequestRef = useRef(0)
    const [detailError, setDetailError] = useState('')
    const [refreshing, setRefreshing] = useState(false)
    const [rowRefreshing, setRowRefreshing] = useState<string | null>(null)
    const [folderStatuses, setFolderStatuses] = useState<Record<string, LinkedFolderStatus>>({})
    const [folderNotes, setFolderNotes] = useState<Record<string, LinkedFolderNote[]>>({})
    const [folderErrors, setFolderErrors] = useState<Record<string, string>>({})

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

    const folderKey = folders.map((folder) => `${folder.name}\u0000${folder.path}`).join('\u0001')

    const keepTabInDialog = (event: KeyboardEvent, dialog: HTMLDivElement | null) => {
      if (event.key !== 'Tab' || !dialog) return
      const controls = Array.from(dialog.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])'))
      if (controls.length === 0) return
      const first = controls[0]
      const last = controls[controls.length - 1]
      if (event.shiftKey && (document.activeElement === first || !dialog.contains(document.activeElement))) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && (document.activeElement === last || !dialog.contains(document.activeElement))) {
        event.preventDefault()
        first.focus()
      }
    }

    useEffect(() => {
      if (folders.length === 0) return
      let active = true
      let busy = false
      const loadSummaries = async () => {
        if (busy) return
        busy = true
        try {
          const results = await Promise.all(folders.map(async (folder) => {
            const [statusResult, notesResult] = await Promise.allSettled([
              linkedFolderStatus(folder.name), listLinkedFolderNotes(folder.name)
            ])
            return { name: folder.name, statusResult, notesResult }
          }))
          if (!active) return
          const nextStatuses: Record<string, LinkedFolderStatus> = {}
          const nextNotes: Record<string, LinkedFolderNote[]> = {}
          const nextErrors: Record<string, string> = {}
          for (const result of results) {
            if (result.statusResult.status === 'fulfilled') nextStatuses[result.name] = result.statusResult.value
            else nextErrors[result.name] = 'Could not load folder status'
            if (result.notesResult.status === 'fulfilled') nextNotes[result.name] = result.notesResult.value
            else nextErrors[result.name] = 'Could not load recent notes'
          }
          setFolderStatuses(nextStatuses)
          setFolderNotes(nextNotes)
          setFolderErrors(nextErrors)
        } finally {
          busy = false
        }
      }
      void loadSummaries()
      const timer = window.setInterval(loadSummaries, 15000)
      return () => { active = false; window.clearInterval(timer) }
    }, [folderKey, linkedFolderStatus, listLinkedFolderNotes])

    useEffect(() => {
      if (!showAddModal) return
      nameInputRef.current?.focus()
    }, [showAddModal])

    useEffect(() => {
      if (!showAddModal) return
      const onKeyDown = (event: KeyboardEvent) => {
        if (event.key === 'Escape' && !adding) closeAddModal()
        keepTabInDialog(event, addDialogRef.current)
      }
      window.addEventListener('keydown', onKeyDown)
      return () => window.removeEventListener('keydown', onKeyDown)
    }, [showAddModal, adding])

    const openAddModal = () => {
      addOpenerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
      setFormErrors({})
      setShowAddModal(true)
    }

    const closeAddModal = () => {
      setShowAddModal(false)
      setFormErrors({})
      window.requestAnimationFrame(() => addOpenerRef.current?.focus())
    }

    const openDetailModal = (folder: LinkedFolder) => {
      detailOpenerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
      setDetailError('')
      setDetailFolder(folder)
    }

    const closeDetailModal = () => {
      setDetailFolder(null)
      window.requestAnimationFrame(() => detailOpenerRef.current?.focus())
    }

    useEffect(() => {
      if (!detailFolder) return
      detailCloseRef.current?.focus()
      const onKeyDown = (event: KeyboardEvent) => {
        if (event.key === 'Escape') closeDetailModal()
        keepTabInDialog(event, detailDialogRef.current)
      }
      window.addEventListener('keydown', onKeyDown)
      return () => window.removeEventListener('keydown', onKeyDown)
    }, [detailFolder?.name])

    useEffect(() => {
      if (!detailFolder) return
      setStatus(null)
      setNotes([])
      setPreview('')
      setPreviewNoteId(null)
      setPreviewLoading(false)
      previewRequestRef.current += 1
      let active = true
      const load = async () => {
        try {
          const [nextStatus, nextNotes] = await Promise.all([
            linkedFolderStatus(detailFolder.name), listLinkedFolderNotes(detailFolder.name)
          ])
          if (active) { setStatus(nextStatus); setNotes(nextNotes); setDetailError('') }
        } catch (err: any) {
          if (active) setDetailError(err?.message || 'Failed to load linked folder details')
        }
      }
      void load()
      const timer = window.setInterval(load, 10000)
      return () => { active = false; window.clearInterval(timer) }
    }, [detailFolder?.name, linkedFolderStatus, listLinkedFolderNotes])

    const handleRefresh = async () => {
      if (!detailFolder) return
      setRefreshing(true)
      try {
        const nextStatus = await refreshLinkedFolder(detailFolder.name)
        setStatus(nextStatus)
        setFolderStatuses((current) => ({ ...current, [detailFolder.name]: nextStatus }))
        setDetailError('')
      }
      catch (err: any) { setDetailError(err?.message || 'Could not refresh folder') }
      finally { setRefreshing(false) }
    }

    const handlePreview = async (note: LinkedFolderNote) => {
      if (previewNoteId === note.id) {
        previewRequestRef.current += 1
        setPreviewNoteId(null)
        setPreview('')
        setPreviewLoading(false)
        return
      }
      const request = ++previewRequestRef.current
      setPreviewNoteId(note.id)
      setPreview('')
      setPreviewLoading(true)
      try {
        const content = await readLinkedFolderNote(note.folder, note.id)
        if (request === previewRequestRef.current) { setPreview(content); setDetailError('') }
      } catch (err: any) {
        if (request === previewRequestRef.current) setDetailError(err?.message || 'Could not read note')
      } finally {
        if (request === previewRequestRef.current) setPreviewLoading(false)
      }
    }

    const handleCopyPath = async (path: string) => {
      try {
        if (navigator.clipboard?.writeText) {
          await navigator.clipboard.writeText(path)
        } else {
          const input = document.createElement('textarea')
          input.value = path
          input.style.position = 'fixed'
          input.style.opacity = '0'
          document.body.appendChild(input)
          input.select()
          const copied = document.execCommand('copy')
          input.remove()
          if (!copied) throw new Error('Could not copy path')
        }
        setDetailError('')
      } catch (err: any) { setDetailError(err?.message || 'Could not copy path') }
    }

    const handleAddFolder = async (e: React.FormEvent) => {
      e.preventDefault()
      const errors: typeof formErrors = {}
      if (!newName.trim()) errors.name = 'Enter a name for this folder.'
      else if (folders.some((folder) => folder.name === newName.trim())) errors.name = 'This name is already linked. Choose another name.'
      if (!newPath.trim()) errors.path = 'Choose or enter a folder path.'
      if (Object.keys(errors).length) { setFormErrors(errors); return }
      setFormErrors({})
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
        setNameFromPicker(false)
        closeAddModal()
        await fetchFolders()
      } catch (err: any) {
        console.error('Failed to add linked folder:', err)
        setFormErrors({ form: err?.message || 'Could not link folder. Check that the path exists and Mint can access it.' })
      } finally {
        setAdding(false)
      }
    }

    const handleBrowse = async () => {
      if (!selectFolder) return
      try {
        const picked = await selectFolder()
        if (picked) {
          setNewPath(picked)
          setFormErrors((current) => ({ ...current, path: undefined, form: undefined }))
          if (!newName.trim() || nameFromPicker) {
            const basename = picked.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || ''
            setNewName(basename)
            setNameFromPicker(true)
            setFormErrors((current) => ({ ...current, name: undefined }))
          }
        }
      } catch (err: any) {
        console.error('Failed to open folder picker:', err)
        setFormErrors((current) => ({ ...current, path: err?.message || 'Could not open folder picker.' }))
      }
    }

    const handleRowRefresh = async (name: string) => {
      setRowRefreshing(name)
      try {
        const nextStatus = await refreshLinkedFolder(name)
        setFolderStatuses((current) => ({ ...current, [name]: nextStatus }))
        setFolderErrors((current) => ({ ...current, [name]: '' }))
      } catch (err: any) {
        setFolderErrors((current) => ({ ...current, [name]: err?.message || 'Could not refresh folder.' }))
      } finally {
        setRowRefreshing(null)
      }
    }

    const handleRemoveFolder = async (name: string) => {
      if (!window.confirm(`Unlink folder "${name}"? (The folder and its notes are not deleted.)`)) return
      try {
        await removeLinkedFolder(name)
        if (detailFolder?.name === name) closeDetailModal()
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
              Linked folders
            </h1>
            <p className="management-subtitle">
              Mint reads linked folder files to find relevant chats and saves useful notes into <code>mint-notes/</code>.
            </p>
          </div>
          <button type="button" className="management-primary-btn" onClick={openAddModal}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            Link a Folder
          </button>
        </div>

        {folders.length > 0 && <div className="management-control-bar">
          <div className="management-search-wrapper">
            <input
              type="text"
              className="management-search-input"
              aria-label="Search linked folders"
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
        </div>}

        {error && folders.length > 0 && <div className="management-error-banner" role="alert">{error}</div>}

        {loading ? (
          <div className="mgmt-empty">Loading…</div>
        ) : error && folders.length === 0 ? (
          <div className="linked-load-error" role="alert">
            <p>Could not load linked folders.</p>
            <button type="button" className="management-action-btn" onClick={() => void fetchFolders()}>Try again</button>
          </div>
        ) : folders.length === 0 ? (
          <section className="linked-empty" aria-labelledby="linked-empty-title">
            <div className="linked-empty-icon" aria-hidden="true">{renderLinkedFoldersSvgIcon(30, 'var(--accent)')}</div>
            <h2 id="linked-empty-title">Give a folder a place in your conversations</h2>
            <p>Link a folder so Mint can use its supported files to find relevant context and save useful notes from your chats.</p>
            <div className="linked-empty-steps" aria-label="How linked folders work">
              <div><strong>1 · Choose a folder</strong><span>Mint reads supported files in it and its subfolders.</span></div>
              <div><strong>2 · Chat as usual</strong><span>Mint uses relevant content when a conversation matches.</span></div>
              <div><strong>3 · Find your notes</strong><span>Saved notes appear inside that folder’s <code>mint-notes/</code>.</span></div>
            </div>
            <button type="button" className="management-primary-btn" onClick={openAddModal}>Link a Folder</button>
            <p className="linked-empty-cli">You can also run <code>mint link add</code> in your terminal.</p>
          </section>
        ) : filteredFolders.length === 0 ? (
          <div className="mgmt-empty">No linked folders match your search.</div>
        ) : (
          <div className="mgmt-list linked-folder-list">
            {filteredFolders.map((folder) => {
              const folderStatus = folderStatuses[folder.name]
              const latestNote = folderNotes[folder.name]?.find((note) => note.status === 'saved')
              const rowError = folderErrors[folder.name] || folderStatus?.indexError
              const stateLabel = rowError ? 'Needs attention' : !folderStatus ? 'Checking…' : !folderStatus.indexedAt ? 'Reading files…' : 'Ready'
              return <div key={folder.name} className="mgmt-row linked-folder-row">
                <div className="mgmt-row-main">
                  <div className="linked-folder-heading">
                    <span className="mgmt-row-title">{folder.name}</span>
                    <span className={`mgmt-status ${rowError ? 'failed' : folderStatus?.indexedAt ? 'ok' : 'running'}`} role="status">{stateLabel}</span>
                  </div>
                  <div className="mgmt-row-sub" title={folder.path}>{folder.path}</div>
                  {folder.description && <div className="linked-folder-description">{folder.description}</div>}
                  <div className="linked-folder-meta">
                    <span>{folderStatus ? `${folderStatus.indexedFiles} files indexed` : 'Checking files…'}</span>
                    <span aria-hidden="true">·</span>
                    <span>{latestNote ? `Last note ${new Date(latestNote.createdAt).toLocaleString()}` : 'No notes yet'}</span>
                  </div>
                  {latestNote && <p className="linked-folder-note-snippet">{latestNote.content.slice(0, 180)}</p>}
                  {rowError && <p className="linked-folder-error" role="alert">{rowError}</p>}
                </div>
                <div className="linked-folder-actions">
                  <button type="button" className="management-action-btn" onClick={() => openDetailModal(folder)}>View notes</button>
                  <button type="button" className="management-action-btn" disabled={rowRefreshing === folder.name} onClick={() => void handleRowRefresh(folder.name)}>
                    {rowRefreshing === folder.name ? 'Refreshing…' : 'Refresh'}
                  </button>
                </div>
              </div>
            })}
          </div>
        )}

        {/* Folder Detail */}
        {detailFolder && (
          <div className="management-modal-overlay" onClick={closeDetailModal}>
            <div ref={detailDialogRef} className="management-modal linked-detail-modal" role="dialog" aria-modal="true" aria-labelledby="linked-detail-title" onClick={(e) => e.stopPropagation()}>
              <div className="management-modal-header">
                <div className="management-card-title-group">
                  {renderFolderIcon(44)}
                  <h2 id="linked-detail-title" className="management-modal-title">{detailFolder.name}</h2>
                </div>
                <button
                  type="button"
                  className="management-modal-close"
                  ref={detailCloseRef}
                  aria-label="Close folder details"
                  onClick={closeDetailModal}
                >
                  ✕
                </button>
              </div>

              <div className="management-modal-body">
                <p style={{ color: 'var(--text-soft, #d1d1d4)', lineHeight: 1.55, margin: 0 }}>
                  {detailFolder.description || 'No description'}
                </p>
                <div className="mgmt-detail-grid">
                  <span>Path</span>
                  <code>{detailFolder.path}</code>
                  <span>Notes</span>
                  <code>{detailFolder.path.replace(/\/$/, '')}/mint-notes/</code>
                  <span>Indexed files</span>
                  <span>{status ? status.indexedFiles : 'Indexing…'}</span>
                  <span>Last indexed</span>
                  <span>{status?.indexedAt ? new Date(status.indexedAt).toLocaleString() : 'Pending'}</span>
                  <span>Background notes (all folders)</span>
                  <span>{status ? `${status.pendingJobs} pending · ${status.failedJobs} failed` : 'Loading…'}</span>
                </div>
                {status?.indexError && <p role="status" className="management-error-banner">{status.indexError}</p>}
                {status?.lastJobError && <p role="status" className="management-error-banner">Last note error: {status.lastJobError}</p>}
                {detailError && <p role="alert" className="management-error-banner">{detailError}</p>}
                <button type="button" className="management-action-btn" onClick={handleRefresh} disabled={refreshing}>
                  {refreshing ? 'Refreshing…' : 'Refresh file index'}
                </button>
                <h3>Recent notes</h3>
                {notes.length === 0 ? <p>No notes saved yet.</p> : (
                  <div className="mgmt-list">
                    {notes.map((note) => <div className="mgmt-row linked-note-row" key={note.id}>
                      <div className="mgmt-row-main">
                        <div className="mgmt-row-title">{new Date(note.createdAt).toLocaleString()} · {note.status}</div>
                        <div className="mgmt-row-sub">{note.content.slice(0, 160)}</div>
                        {note.error && <p role="status">{note.error}</p>}
                        <button type="button" className="management-action-btn" disabled={note.status !== 'saved'} aria-expanded={previewNoteId === note.id} onClick={() => void handlePreview(note)}>{previewNoteId === note.id ? 'Hide note' : 'View note'}</button>
                        {canOpenNote && <button type="button" className="management-action-btn" disabled={note.status !== 'saved'} onClick={() => void openLinkedFolderNote(note.folder, note.id).catch((err) => setDetailError(String(err)))}>Open file</button>}
                        <button type="button" className="management-action-btn" onClick={() => void handleCopyPath(note.path)}>Copy path</button>
                        {previewNoteId === note.id && <div className="linked-note-preview" role="region" aria-label={`Note file for ${new Date(note.createdAt).toLocaleString()}`}>
                          {previewLoading ? <p>Loading note…</p> : preview ? <pre>{preview}</pre> : <p>Could not show this note.</p>}
                        </div>}
                      </div>
                    </div>)}
                  </div>
                )}
              </div>

              <div className="management-modal-footer">
                <span />
                <button
                  type="button"
                  className="management-action-btn danger"
                  onClick={() => handleRemoveFolder(detailFolder.name)}
                >
                  Unlink
                </button>
              </div>
            </div>
          </div>
        )}

        {showAddModal && (
          <div className="management-modal-overlay">
            <div ref={addDialogRef} className="management-modal linked-add-modal" role="dialog" aria-modal="true" aria-labelledby="linked-add-title">
              <div className="management-modal-header">
                <h2 id="linked-add-title" className="management-modal-title">Link a Folder</h2>
                <button type="button" className="management-modal-close" aria-label="Close" disabled={adding} onClick={closeAddModal}>
                  ✕
                </button>
              </div>

              <form onSubmit={handleAddFolder} noValidate>
                <div className="management-modal-body">
                  <div className="management-form-group">
                    <label className="management-label" htmlFor="linked-folder-name">Name</label>
                    <input
                      ref={nameInputRef}
                      id="linked-folder-name"
                      type="text"
                      className="management-input-field"
                      placeholder="e.g. Food"
                      value={newName}
                      aria-invalid={Boolean(formErrors.name)}
                      aria-describedby={formErrors.name ? 'linked-name-error' : undefined}
                      onChange={(e) => {
                        setNewName(e.target.value)
                        setNameFromPicker(false)
                        setFormErrors((current) => ({ ...current, name: undefined, form: undefined }))
                      }}
                      required
                    />
                    {formErrors.name && <span id="linked-name-error" className="linked-field-error" role="alert">{formErrors.name}</span>}
                  </div>

                  <div className="management-form-group">
                    <label className="management-label" htmlFor="linked-folder-path">Folder path</label>
                    <div className="linked-path-row">
                      <input
                        id="linked-folder-path"
                        type="text"
                        className="management-input-field"
                        placeholder="~/notes/food"
                        value={newPath}
                        aria-invalid={Boolean(formErrors.path)}
                        aria-describedby={formErrors.path ? 'linked-path-error' : undefined}
                        onChange={(e) => {
                          setNewPath(e.target.value)
                          setFormErrors((current) => ({ ...current, path: undefined, form: undefined }))
                        }}
                        required
                      />
                      {selectFolder && (
                        <button type="button" className="management-action-btn" onClick={handleBrowse}>
                          Browse...
                        </button>
                      )}
                    </div>
                    {formErrors.path && <span id="linked-path-error" className="linked-field-error" role="alert">{formErrors.path}</span>}
                  </div>

                  <div className="management-form-group">
                    <label className="management-label" htmlFor="linked-folder-description">What should Mint remember here? <span className="linked-optional">Optional</span></label>
                    <textarea
                      id="linked-folder-description"
                      className="management-textarea-field"
                      placeholder="e.g. Restaurant reviews, recipes, and places I want to try"
                      value={newDescription}
                      onChange={(e) => setNewDescription(e.target.value)}
                      rows={3}
                    />
                    <p className="linked-field-hint">Describe the topics that belong in this folder. Mint uses this when deciding where a chat note belongs.</p>
                  </div>
                  <p className="linked-scope-note">Mint reads supported files in this folder and its subfolders (such as Markdown, PDF, Word, spreadsheets and code), then saves useful chat notes in <code>mint-notes/</code> inside the folder.</p>
                  {formErrors.form && <p className="management-error-banner" role="alert">{formErrors.form}</p>}
                </div>

                <div className="management-modal-footer">
                  <button type="button" className="management-action-btn" disabled={adding} onClick={closeAddModal}>
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
