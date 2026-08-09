import React, { useState, useEffect } from 'react'
import { renderScheduledTasksSvgIcon } from '../constants/plugins'
import '../css/management-views.css'
import type { CronJob, CronJobDraft } from '../types'

export type { CronJob }

export interface ScheduledTasksViewProps {
  listCronJobs: () => Promise<CronJob[]>
  addCronJob: (draft: CronJobDraft) => Promise<CronJob>
  removeCronJob: (id: string) => Promise<any>
  setCronJobEnabled: (id: string, enabled: boolean) => Promise<any>
  workspacePath?: string
}

function formatTimestamp(value: string | null): string {
  if (!value) return 'never'
  const parsed = new Date(value)
  if (Number.isNaN(parsed.getTime())) return value
  return parsed.toLocaleString()
}

export const ScheduledTasksView: React.FC<ScheduledTasksViewProps> = React.memo(
  function ScheduledTasksView({ listCronJobs, addCronJob, removeCronJob, setCronJobEnabled, workspacePath }) {
    const [jobs, setJobs] = useState<CronJob[]>([])
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState('')
    const [searchQuery, setSearchQuery] = useState('')

    const [newName, setNewName] = useState('')
    const [newSchedule, setNewSchedule] = useState('')
    const [newTask, setNewTask] = useState('')
    const [newWorkspace, setNewWorkspace] = useState(workspacePath || '')
    const [adding, setAdding] = useState(false)
    const [showAddModal, setShowAddModal] = useState(false)

    const fetchJobs = async () => {
      setLoading(true)
      setError('')
      try {
        const list = await listCronJobs()
        setJobs(list)
      } catch (err: any) {
        console.error('Failed to fetch cron jobs:', err)
        setError('Failed to load scheduled tasks')
      } finally {
        setLoading(false)
      }
    }

    useEffect(() => {
      fetchJobs()
    }, [])

    useEffect(() => {
      setNewWorkspace(workspacePath || '')
    }, [workspacePath])

    const handleAddJob = async (e: React.FormEvent) => {
      e.preventDefault()
      if (!newName.trim() || !newSchedule.trim() || !newTask.trim()) return
      setAdding(true)
      try {
        await addCronJob({
          name: newName.trim(),
          schedule: newSchedule.trim(),
          task: newTask.trim(),
          workspace: newWorkspace.trim() || workspacePath || '.'
        })
        setNewName('')
        setNewSchedule('')
        setNewTask('')
        setShowAddModal(false)
        fetchJobs()
      } catch (err: any) {
        console.error('Failed to add scheduled task:', err)
        alert(err?.message || 'Error saving scheduled task. Check the cron expression.')
      } finally {
        setAdding(false)
      }
    }

    const handleRemoveJob = async (id: string, name: string) => {
      if (!window.confirm(`Remove scheduled task "${name}"?`)) return
      try {
        await removeCronJob(id)
        fetchJobs()
      } catch (err: any) {
        console.error('Failed to remove scheduled task:', err)
        alert('Error removing scheduled task')
      }
    }

    const handleToggleEnabled = async (job: CronJob) => {
      try {
        await setCronJobEnabled(job.id, !job.enabled)
        fetchJobs()
      } catch (err: any) {
        console.error('Failed to toggle scheduled task:', err)
        alert('Error updating scheduled task')
      }
    }

    const filteredJobs = jobs.filter((job) => {
      const q = searchQuery.toLowerCase()
      return (
        job.name.toLowerCase().includes(q) ||
        job.task.toLowerCase().includes(q) ||
        job.schedule.toLowerCase().includes(q)
      )
    })

    return (
      <div className="management-container">
        <div className="management-header">
          <div className="management-title-group">
            <h1 className="management-title">
              <span className="management-title-icon" style={{ display: 'inline-flex', alignItems: 'center' }}>
                {renderScheduledTasksSvgIcon(22, 'var(--accent)')}
              </span>
              Scheduled Tasks
            </h1>
            <p className="management-subtitle">
              Recurring agent tasks that fire automatically on a cron schedule while Mint is running (interactive
              chat, `mint api`/`mint web`, or the desktop app).
            </p>
          </div>
          <button type="button" className="management-primary-btn" onClick={() => setShowAddModal(true)}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            New Scheduled Task
          </button>
        </div>

        <div className="management-control-bar">
          <div className="management-search-wrapper">
            <input
              type="text"
              className="management-search-input"
              placeholder="Search scheduled tasks..."
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
            Loading scheduled tasks...
          </div>
        ) : filteredJobs.length === 0 ? (
          <div className="management-empty-state">
            <div className="management-empty-icon">⏰</div>
            <h3 className="management-empty-title">No scheduled tasks yet</h3>
            <p className="management-empty-desc">
              Create one above, or use <code>mint cron add</code> / <code>/cron add</code> in chat.
            </p>
          </div>
        ) : (
          <div className="management-grid">
            {filteredJobs.map((job) => (
              <div key={job.id} className="management-card">
                <div>
                  <div className="management-card-header">
                    <span className="management-card-title-badge">{job.name}</span>
                    <span className={`management-badge ${job.enabled ? 'active' : 'inactive'}`}>
                      <span className="management-dot" />
                      {job.enabled ? 'Enabled' : 'Disabled'}
                    </span>
                  </div>

                  <p className="management-card-desc">{job.task}</p>

                  <div style={{ fontSize: '0.8rem', color: 'var(--text-muted, #94a3b8)', marginTop: '8px' }}>
                    <div>
                      Schedule: <code>{job.schedule}</code>
                    </div>
                    <div>Next run: {formatTimestamp(job.nextRun)}</div>
                    <div>
                      Last run: {formatTimestamp(job.lastRunAt)}
                      {job.lastStatus && (
                        <span
                          className={`management-badge ${job.lastStatus === 'success' ? 'active' : 'inactive'}`}
                          style={{ marginLeft: '6px' }}
                        >
                          {job.lastStatus}
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                <div className="management-card-footer">
                  <label className="settings-toggle-switch">
                    <input
                      type="checkbox"
                      checked={job.enabled}
                      onChange={() => handleToggleEnabled(job)}
                    />
                    <span className="settings-toggle-slider"></span>
                  </label>
                  <button
                    type="button"
                    className="management-action-btn danger"
                    onClick={() => handleRemoveJob(job.id, job.name)}
                  >
                    Remove
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
                <h2 className="management-modal-title">New Scheduled Task</h2>
                <button type="button" className="management-modal-close" onClick={() => setShowAddModal(false)}>
                  ✕
                </button>
              </div>

              <form onSubmit={handleAddJob}>
                <div className="management-modal-body">
                  <div className="management-form-group">
                    <label className="management-label">Name</label>
                    <input
                      type="text"
                      className="management-input-field"
                      placeholder="e.g. Morning stock report"
                      value={newName}
                      onChange={(e) => setNewName(e.target.value)}
                      required
                    />
                  </div>

                  <div className="management-form-group">
                    <label className="management-label">Schedule (cron expression)</label>
                    <input
                      type="text"
                      className="management-input-field"
                      placeholder="0 8 * * *  (every day at 08:00)"
                      value={newSchedule}
                      onChange={(e) => setNewSchedule(e.target.value)}
                      required
                    />
                  </div>

                  <div className="management-form-group">
                    <label className="management-label">Task / prompt</label>
                    <textarea
                      className="management-textarea-field"
                      placeholder="Fetch today's stock prices and summarize them."
                      value={newTask}
                      onChange={(e) => setNewTask(e.target.value)}
                      required
                      rows={4}
                    />
                  </div>

                  <div className="management-form-group">
                    <label className="management-label">Workspace path</label>
                    <input
                      type="text"
                      className="management-input-field"
                      placeholder="Defaults to the current workspace"
                      value={newWorkspace}
                      onChange={(e) => setNewWorkspace(e.target.value)}
                    />
                  </div>
                </div>

                <div className="management-modal-footer">
                  <button type="button" className="management-action-btn" onClick={() => setShowAddModal(false)}>
                    Cancel
                  </button>
                  <button type="submit" disabled={adding} className="management-primary-btn">
                    {adding ? 'Saving...' : 'Create Task'}
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

export default ScheduledTasksView
