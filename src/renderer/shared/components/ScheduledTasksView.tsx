import React, { useState, useEffect, useMemo } from 'react'
import { DateTime } from 'luxon'
import { renderScheduledTasksSvgIcon, renderTaskLogoIcon } from '../constants/plugins'
import { renderFormattedMessage } from '../utils/markdown'
import '../css/management-views.css'
import type { CronJob, CronJobDraft } from '../types'

export type { CronJob }

// A curated fallback for engines that don't implement
// `Intl.supportedValuesOf('timeZone')` (older WebKitGTK on Linux, notably —
// the desktop build runs inside the OS's native webview, not a bundled
// Chromium, so this can't be assumed present). Covers one representative
// zone per UTC offset/region so the picker still works, just with a shorter
// list, instead of throwing and breaking the whole form.
const FALLBACK_TIMEZONES = [
  'UTC', 'Pacific/Midway', 'Pacific/Honolulu', 'America/Anchorage', 'America/Los_Angeles',
  'America/Denver', 'America/Chicago', 'America/New_York', 'America/Sao_Paulo',
  'Atlantic/Azores', 'Europe/London', 'Europe/Paris', 'Europe/Berlin', 'Europe/Moscow',
  'Africa/Cairo', 'Africa/Johannesburg', 'Asia/Dubai', 'Asia/Karachi', 'Asia/Kolkata',
  'Asia/Dhaka', 'Asia/Bangkok', 'Asia/Jakarta', 'Asia/Shanghai', 'Asia/Singapore',
  'Asia/Tokyo', 'Asia/Seoul', 'Australia/Sydney', 'Pacific/Auckland'
]

function listTimezones(): string[] {
  try {
    const zones = Intl.supportedValuesOf?.('timeZone')
    if (zones && zones.length) return zones
  } catch {
    // fall through to the static list below
  }
  return FALLBACK_TIMEZONES
}

function deviceTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  } catch {
    return 'UTC'
  }
}

// Cron's day-of-week field is 0=Sun..6=Sat; Luxon's `.weekday` is
// ISO-style 1=Mon..7=Sun. `% 7` maps Luxon's 7 (Sun) back to 0 and leaves
// 1..6 (Mon..Sat) unchanged, so it's a two-way conversion.
const cronDowToLuxon = (cronDow: number) => (cronDow === 0 ? 7 : cronDow)
const luxonWeekdayToCron = (luxonWeekday: number) => luxonWeekday % 7

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

/** "in 5 hours" / "3 days ago" (in the runtime locale) — falls back to the
 * absolute local time. */
function formatRelative(value: string | null): string {
  if (!value) return '—'
  const dt = DateTime.fromISO(value)
  if (!dt.isValid) return formatTimestamp(value)
  return dt.toRelative() ?? formatTimestamp(value)
}

/** The status word shown on a task row: a live run, or the outcome of the
 * last one. `null` for a task that has never run. */
function runState(job: CronJob): 'running' | 'ok' | 'failed' | null {
  if (job.runningSince) return 'running'
  if (job.lastStatus === 'success') return 'ok'
  if (job.lastStatus === 'failed') return 'failed'
  return null
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
    // Explicit, not tied to whichever device happens to be viewing the form —
    // defaults to the device's own zone so existing behavior is unchanged
    // until someone picks a different one.
    const [taskTimezone, setTaskTimezone] = useState(deviceTimezone)
    const timezoneOptions = useMemo(listTimezones, [])

    useEffect(() => {
      if (repeatMode === 'custom') return
      const [hh, mm] = timeOfDay.split(':').map((v) => parseInt(v, 10))
      const H = Number.isFinite(hh) ? hh : 8
      const M = Number.isFinite(mm) ? mm : 0

      // The cron scheduler evaluates the saved expression in UTC (see
      // `crates/mint-core/src/cron/schedule.rs`), but these pickers collect
      // a wall-clock time in `taskTimezone` (an explicit IANA zone, not
      // whichever device happens to be viewing this form). Luxon resolves
      // that zone's real UTC offset for the actual calendar date involved —
      // including DST — so every field is built by constructing the picked
      // time in `taskTimezone` and reading its UTC components back out,
      // rather than assuming a fixed offset by hand.
      const nowInZone = DateTime.now().setZone(taskTimezone)
      if (!nowInZone.isValid) return

      if (repeatMode === 'daily') {
        const utc = nowInZone.set({ hour: H, minute: M, second: 0, millisecond: 0 }).toUTC()
        setNewSchedule(`${utc.minute} ${utc.hour} * * *`)
      } else if (repeatMode === 'weekly') {
        if (!weekdays.length) {
          const utc = nowInZone.set({ hour: H, minute: M, second: 0, millisecond: 0 }).toUTC()
          setNewSchedule(`${utc.minute} ${utc.hour} * * *`)
          return
        }
        // A UTC offset can shift the local weekday by at most one calendar
        // day, so each selected weekday is converted individually (rather
        // than assuming they all shift the same way) by anchoring to the
        // next real calendar date — in `taskTimezone` — that actually falls
        // on it.
        const converted = weekdays.map((cronDow) => {
          const targetLuxonDow = cronDowToLuxon(cronDow)
          const daysToAdd = (targetLuxonDow - nowInZone.weekday + 7) % 7
          return nowInZone
            .plus({ days: daysToAdd })
            .set({ hour: H, minute: M, second: 0, millisecond: 0 })
            .toUTC()
        })
        const utcWeekdays = [...new Set(converted.map((d) => luxonWeekdayToCron(d.weekday)))].sort(
          (a, b) => a - b
        )
        setNewSchedule(`${converted[0].minute} ${converted[0].hour} * * ${utcWeekdays.join(',')}`)
      } else if (repeatMode === 'monthly') {
        const utc = nowInZone
          .set({ day: monthDay, hour: H, minute: M, second: 0, millisecond: 0 })
          .toUTC()
        if (!utc.isValid) return
        setNewSchedule(`${utc.minute} ${utc.hour} ${utc.day} * *`)
      } else if (repeatMode === 'once') {
        if (!onceDate) {
          setNewSchedule('')
          return
        }
        const [y, mo, d] = onceDate.split('-').map((v) => parseInt(v, 10))
        const utc = DateTime.fromObject(
          { year: y, month: mo, day: d, hour: H, minute: M },
          { zone: taskTimezone }
        ).toUTC()
        if (!utc.isValid) return
        // 7-field form (sec min hour day month dow year) with the year
        // pinned — the schedule has no occurrence after that date, so the
        // scheduler runs it exactly once and then disables it.
        setNewSchedule(`0 ${utc.minute} ${utc.hour} ${utc.day} ${utc.month} * ${utc.year}`)
      }
    }, [repeatMode, timeOfDay, weekdays, monthDay, onceDate, taskTimezone])

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
              Tasks that run on a schedule in the background.
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
          <div className="mgmt-empty">Loading…</div>
        ) : filteredJobs.length === 0 ? (
          <div className="mgmt-empty">
            <p>{searchQuery ? 'No scheduled tasks match your search.' : 'No scheduled tasks.'}</p>
            {!searchQuery && (
              <p>
                Add one above, or run <code>mint cron add</code> in chat.
              </p>
            )}
          </div>
        ) : (
          <div className="mgmt-list">
            {filteredJobs.map((job) => {
              const state = runState(job)
              return (
                <div
                  key={job.id}
                  className={`mgmt-row${job.enabled ? '' : ' is-disabled'}`}
                  onClick={() => setDetailJob(job)}
                >
                  <div className="mgmt-row-main">
                    <div className="mgmt-row-title">{job.name}</div>
                    <div className="mgmt-row-meta">
                      <code>{job.schedule}</code>
                      <span className="mgmt-row-sep">·</span>
                      <span>next {formatRelative(job.nextRun)}</span>
                      {state && (
                        <>
                          <span className="mgmt-row-sep">·</span>
                          <span className={`mgmt-status ${state}`}>{state}</span>
                        </>
                      )}
                    </div>
                  </div>
                  <label className="settings-toggle-switch" onClick={(e) => e.stopPropagation()}>
                    <input
                      type="checkbox"
                      checked={job.enabled}
                      onChange={() => handleToggleEnabled(job)}
                    />
                    <span className="settings-toggle-slider"></span>
                  </label>
                </div>
              )
            })}
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
                <p style={{ color: 'var(--text-soft, #d1d1d4)', lineHeight: 1.55, margin: 0 }}>
                  {detailJob.task}
                </p>

                <div className="mgmt-detail-grid">
                  <span>Schedule</span>
                  <code>{detailJob.schedule} <span style={{ opacity: 0.55 }}>UTC</span></code>
                  <span>Next run</span>
                  <span>{formatTimestamp(detailJob.nextRun)}</span>
                  {detailJob.runningSince && (
                    <>
                      <span>Running since</span>
                      <span>{formatTimestamp(detailJob.runningSince)}</span>
                    </>
                  )}
                  <span>Last run</span>
                  <span>
                    {formatTimestamp(detailJob.lastRunAt)}
                    {runState(detailJob) && (
                      <span className={`mgmt-status ${runState(detailJob)}`} style={{ marginLeft: '8px' }}>
                        {runState(detailJob)}
                      </span>
                    )}
                  </span>
                </div>

                {detailJob.lastSummary && (
                  <div>
                    <label className="management-label" style={{ display: 'block', marginBottom: '8px' }}>
                      Last response
                    </label>
                    <div className="mgmt-prose">{renderFormattedMessage(detailJob.lastSummary)}</div>
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
                    <label className="management-label">When to run</label>
                    <div className="schedule-inline-row">
                      <select
                        className="management-input-field schedule-repeat-select"
                        value={repeatMode}
                        onChange={(e) => setRepeatMode(e.target.value as typeof repeatMode)}
                      >
                        <option value="daily">Daily</option>
                        <option value="weekly">Weekly</option>
                        <option value="monthly">Monthly</option>
                        <option value="once">One-time</option>
                        <option value="custom">Custom (cron)</option>
                      </select>

                      {repeatMode === 'weekly' && (
                        <>
                          <span className="schedule-inline-word">on</span>
                          <div className="schedule-weekday-circles">
                            {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'].map((day, idx) => (
                              <button
                                key={day}
                                type="button"
                                className={`management-pill-btn-circle ${weekdays.includes(idx) ? 'active' : ''}`}
                                onClick={() => toggleWeekday(idx)}
                                title={day}
                              >
                                {day[0]}
                              </button>
                            ))}
                          </div>
                        </>
                      )}

                      {repeatMode === 'monthly' && (
                        <>
                          <span className="schedule-inline-word">on day</span>
                          <input
                            type="number"
                            min={1}
                            max={31}
                            className="management-input-field schedule-day-input"
                            value={monthDay}
                            onChange={(e) =>
                              setMonthDay(Math.min(31, Math.max(1, parseInt(e.target.value, 10) || 1)))
                            }
                            required
                          />
                        </>
                      )}

                      {repeatMode === 'once' && (
                        <input
                          type="date"
                          className="management-input-field schedule-day-input"
                          value={onceDate}
                          onChange={(e) => setOnceDate(e.target.value)}
                          required
                        />
                      )}
                    </div>

                    {repeatMode !== 'custom' && (
                      <div className="schedule-inline-row" style={{ marginTop: '10px' }}>
                        <span className="schedule-inline-word">around</span>
                        <input
                          type="time"
                          className="management-input-field schedule-time-input"
                          value={timeOfDay}
                          onChange={(e) => setTimeOfDay(e.target.value)}
                          required
                        />
                        <select
                          className="management-input-field schedule-timezone-select"
                          value={taskTimezone}
                          onChange={(e) => setTaskTimezone(e.target.value)}
                        >
                          {timezoneOptions.map((tz) => (
                            <option key={tz} value={tz}>
                              {tz}
                            </option>
                          ))}
                        </select>
                      </div>
                    )}
                  </div>

                  {repeatMode === 'custom' && (
                    <div className="management-form-group">
                      <label className="management-label">Schedule (cron expression, UTC)</label>
                      <input
                        type="text"
                        className="management-input-field"
                        placeholder="0 8 * * *  (every day at 08:00 UTC)"
                        value={newSchedule}
                        onChange={(e) => setNewSchedule(e.target.value)}
                        required
                      />
                    </div>
                  )}

                  {repeatMode !== 'custom' && (
                    <div style={{ fontSize: '0.78rem', color: 'var(--text-muted, #94a3b8)' }}>
                      Cron (UTC): <code>{newSchedule || '—'}</code>
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
