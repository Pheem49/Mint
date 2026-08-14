import React, { useState, useEffect } from 'react'
import { renderScheduledTasksSvgIcon, renderTaskLogoIcon } from '../constants/plugins'
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
    const [detailJob, setDetailJob] = useState<CronJob | null>(null)

    // Picker inputs for the "New Scheduled Task" form — build the cron
    // expression `newSchedule` from a clock/calendar UI instead of asking
    // everyone to type raw cron syntax. "custom" leaves newSchedule as a
    // free-text field for anyone who wants to type cron directly.
    const [repeatMode, setRepeatMode] = useState<'daily' | 'weekly' | 'monthly' | 'once' | 'custom'>('daily')
    const [timeOfDay, setTimeOfDay] = useState('08:00')
    const [weekdays, setWeekdays] = useState<number[]>([1])
    const [monthDay, setMonthDay] = useState(1)
    const [onceDate, setOnceDate] = useState('')

    useEffect(() => {
      if (repeatMode === 'custom') return
      const [hh, mm] = timeOfDay.split(':').map((v) => parseInt(v, 10))
      const H = Number.isFinite(hh) ? hh : 8
      const M = Number.isFinite(mm) ? mm : 0

      if (repeatMode === 'daily') {
        setNewSchedule(`${M} ${H} * * *`)
      } else if (repeatMode === 'weekly') {
        setNewSchedule(`${M} ${H} * * ${weekdays.length ? [...weekdays].sort().join(',') : '*'}`)
      } else if (repeatMode === 'monthly') {
        setNewSchedule(`${M} ${H} ${monthDay} * *`)
      } else if (repeatMode === 'once') {
        if (!onceDate) {
          setNewSchedule('')
          return
        }
        const [y, mo, d] = onceDate.split('-').map((v) => parseInt(v, 10))
        // 7-field form (sec min hour day month dow year) with the year
        // pinned — the schedule has no occurrence after that date, so the
        // scheduler runs it exactly once and then disables it.
        setNewSchedule(`0 ${M} ${H} ${d} ${mo} * ${y}`)
      }
    }, [repeatMode, timeOfDay, weekdays, monthDay, onceDate])

    const toggleWeekday = (day: number) => {
      setWeekdays((current) =>
        current.includes(day) ? current.filter((d) => d !== day) : [...current, day].sort()
      )
    }

    const resetAddForm = () => {
      setNewName('')
      setNewSchedule('')
      setNewTask('')
      setRepeatMode('daily')
      setTimeOfDay('08:00')
      setWeekdays([1])
      setMonthDay(1)
      setOnceDate('')
    }

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
        resetAddForm()
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
        setDetailJob((current) => (current?.id === id ? null : current))
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
              <div
                key={job.id}
                className="management-card"
                onClick={() => setDetailJob(job)}
                style={{ cursor: 'pointer' }}
              >
                <div>
                  <div className="management-card-header">
                    <div className="management-card-title-group">
                      {renderTaskLogoIcon()}
                      <h3 className="management-card-title">{job.name}</h3>
                    </div>
                    <span className={`management-badge ${job.enabled ? 'active' : 'inactive'}`}>
                      <span className="management-dot" />
                      {job.enabled ? 'Enabled' : 'Disabled'}
                    </span>
                  </div>

                  <p className="management-card-desc">{job.task}</p>
                </div>

                <div className="management-card-footer">
                  <code style={{ fontSize: '0.78rem', color: 'var(--text-muted, #94a3b8)' }}>
                    {job.schedule}
                  </code>
                  <label className="settings-toggle-switch" onClick={(e) => e.stopPropagation()}>
                    <input
                      type="checkbox"
                      checked={job.enabled}
                      onChange={() => handleToggleEnabled(job)}
                    />
                    <span className="settings-toggle-slider"></span>
                  </label>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Scheduled Task Detail */}
        {detailJob && (
          <div className="management-modal-overlay" onClick={() => setDetailJob(null)}>
            <div className="management-modal" onClick={(e) => e.stopPropagation()}>
              <div className="management-modal-header">
                <div className="management-card-title-group">
                  {renderTaskLogoIcon(44)}
                  <h2 className="management-modal-title">{detailJob.name}</h2>
                </div>
                <button type="button" className="management-modal-close" onClick={() => setDetailJob(null)}>
                  ✕
                </button>
              </div>

              <div className="management-modal-body">
                <span className={`management-badge ${detailJob.enabled ? 'active' : 'inactive'}`}>
                  <span className="management-dot" />
                  {detailJob.enabled ? 'Enabled' : 'Disabled'}
                </span>

                <p style={{ color: 'var(--text-soft, #d1d1d4)', lineHeight: 1.55, marginTop: '14px' }}>
                  {detailJob.task}
                </p>

                <div style={{ fontSize: '0.85rem', color: 'var(--text-muted, #94a3b8)', marginTop: '14px', display: 'grid', gap: '6px' }}>
                  <div>
                    Schedule: <code>{detailJob.schedule}</code>
                  </div>
                  <div>Next run: {formatTimestamp(detailJob.nextRun)}</div>
                  <div>
                    Last run: {formatTimestamp(detailJob.lastRunAt)}
                    {detailJob.lastStatus && (
                      <span
                        className={`management-badge ${detailJob.lastStatus === 'success' ? 'active' : 'inactive'}`}
                        style={{ marginLeft: '6px' }}
                      >
                        {detailJob.lastStatus}
                      </span>
                    )}
                  </div>
                </div>

                {detailJob.lastSummary && (
                  <div style={{ marginTop: '16px' }}>
                    <label className="management-label" style={{ display: 'block', marginBottom: '6px' }}>
                      Last response
                    </label>
                    <div
                      className="management-code-snippet"
                      style={{ whiteSpace: 'pre-wrap', fontFamily: 'inherit', maxHeight: '260px', overflowY: 'auto' }}
                    >
                      {detailJob.lastSummary}
                    </div>
                  </div>
                )}
              </div>

              <div className="management-modal-footer">
                <label className="settings-toggle-switch">
                  <input
                    type="checkbox"
                    checked={detailJob.enabled}
                    onChange={() => handleToggleEnabled(detailJob)}
                  />
                  <span className="settings-toggle-slider"></span>
                </label>
                <button
                  type="button"
                  className="management-action-btn danger"
                  onClick={() => handleRemoveJob(detailJob.id, detailJob.name)}
                >
                  Remove
                </button>
              </div>
            </div>
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
                    <label className="management-label">Repeat</label>
                    <div className="management-filter-pills" style={{ flexWrap: 'wrap' }}>
                      {(['daily', 'weekly', 'monthly', 'once', 'custom'] as const).map((mode) => (
                        <button
                          key={mode}
                          type="button"
                          className={`management-pill-btn ${repeatMode === mode ? 'active' : ''}`}
                          onClick={() => setRepeatMode(mode)}
                        >
                          {mode === 'once' ? 'One-time' : mode}
                        </button>
                      ))}
                    </div>
                  </div>

                  {repeatMode !== 'custom' && (
                    <div className="management-form-group">
                      <label className="management-label">Time</label>
                      <input
                        type="time"
                        className="management-input-field"
                        value={timeOfDay}
                        onChange={(e) => setTimeOfDay(e.target.value)}
                        required
                      />
                    </div>
                  )}

                  {repeatMode === 'weekly' && (
                    <div className="management-form-group">
                      <label className="management-label">On these days</label>
                      <div className="management-filter-pills" style={{ flexWrap: 'wrap' }}>
                        {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'].map((label, idx) => (
                          <button
                            key={label}
                            type="button"
                            className={`management-pill-btn ${weekdays.includes(idx) ? 'active' : ''}`}
                            onClick={() => toggleWeekday(idx)}
                          >
                            {label}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  {repeatMode === 'monthly' && (
                    <div className="management-form-group">
                      <label className="management-label">Day of month</label>
                      <input
                        type="number"
                        min={1}
                        max={31}
                        className="management-input-field"
                        value={monthDay}
                        onChange={(e) => setMonthDay(Math.min(31, Math.max(1, parseInt(e.target.value, 10) || 1)))}
                        required
                      />
                    </div>
                  )}

                  {repeatMode === 'once' && (
                    <div className="management-form-group">
                      <label className="management-label">Date</label>
                      <input
                        type="date"
                        className="management-input-field"
                        value={onceDate}
                        onChange={(e) => setOnceDate(e.target.value)}
                        required
                      />
                    </div>
                  )}

                  {repeatMode === 'custom' && (
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
                  )}

                  {repeatMode !== 'custom' && (
                    <div style={{ fontSize: '0.78rem', color: 'var(--text-muted, #94a3b8)' }}>
                      Cron: <code>{newSchedule || '—'}</code>
                    </div>
                  )}

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
