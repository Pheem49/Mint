import React, { useEffect, useState } from 'react'
import { notificationPermission, readNotificationSettings, type NotificationSettings } from '../../utils/notificationSettings'
import '../../css/settings/notifications.css'

export default function NotificationsTab({ settings, onChange }: {
  settings: unknown; onChange: (settings: NotificationSettings) => void
}) {
  const preferences = readNotificationSettings(settings)
  const [permission, setPermission] = useState<string>('Checking…')
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  useEffect(() => {
    let disposed = false
    const refresh = () => {
      notificationPermission().then(value => { if (!disposed) setPermission(value) })
        .catch(() => { if (!disposed) setPermission('Could not check permission') })
    }
    refresh()
    window.addEventListener('focus', refresh)
    return () => { disposed = true; window.removeEventListener('focus', refresh) }
  }, [])
  const run = async (test: boolean) => {
    setBusy(true)
    setMessage('')
    try {
      const granted = await notificationPermission(true)
      setPermission(granted)
      if (granted !== 'granted') {
        setMessage(granted === 'unsupported' ? 'Notifications are unavailable in this browser.' : 'Allow notifications in your browser or system settings, then check permission again.')
      } else if (test) {
        await window.api.notifyAiResponse('Your Mint notifications are working.')
        setMessage('Test notification sent. If it does not appear, check your system’s Do Not Disturb setting.')
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error))
    } finally { setBusy(false) }
  }
  const toggle = (key: keyof NotificationSettings, title: string, description: string) => (
    <div className="toggle-row" key={key}>
      <div><label htmlFor={'notifications-' + key}>{title}</label><p className="hint">{description}</p></div>
      <label className="settings-toggle-switch">
        <input id={'notifications-' + key} type="checkbox" aria-label={title}
          checked={preferences[key]} disabled={key !== 'enabled' && !preferences.enabled}
          onChange={event => onChange({ ...preferences, [key]: event.target.checked })} />
        <span className="settings-toggle-slider" />
      </label>
    </div>
  )
  return (
    <div className="tab-pane active notifications-tab">
      <section className="setting-section">
        <div className="section-heading"><div><p className="section-kicker">Stay informed</p><h2 className="section-title">Notifications</h2></div></div>
        <p className="hint">Choose what Mint shows in Desktop and Web. Select Save Settings to apply your changes.</p>
        <div className="toggle-card">{toggle('enabled', 'Enable notifications', 'Turn Mint notifications on or off.')}</div>
      </section>
      <section className="setting-section">
        <h3>Notification types</h3>
        <div className="toggle-card">
          {toggle('replies', 'AI replies', 'Notify when a response finishes while Mint is in the background.')}
          {toggle('approvals', 'Approval requests', 'Notify when Mint needs your approval or an answer while in the background.')}
          {toggle('system', 'System & background tasks', 'Battery, connection changes, and queued task updates on Desktop.')}
        </div>
      </section>
      <section className="setting-section">
        <h3>Where notifications appear</h3>
        <div className="toggle-card">
          {toggle('os', 'System notifications', 'Show an operating system popup while Mint is in the background.')}
          {toggle('inApp', 'In-app notices', 'Show system and background task updates inside Mint.')}
        </div>
      </section>
      <section className="setting-section notifications-permission">
        <h3>Notification permission</h3>
        <p className="hint">Permission is specific to this browser or Desktop installation.</p>
        <p className="notifications-permission-state" role="status">Status: <strong>{permission}</strong></p>
        {permission === 'denied' && <p className="hint">Notifications are blocked. Allow them in your browser or system settings.</p>}
        <div className="notifications-actions">
          <button type="button" className="btn-secondary" disabled={busy || permission === 'unsupported'} onClick={() => void run(false)}>Check permission</button>
          <button type="button" className="btn-secondary" disabled={busy || !preferences.enabled || !preferences.os || permission === 'unsupported'} onClick={() => void run(true)}>Send test notification</button>
        </div>
        {message && <p className="notifications-feedback" role="status">{message}</p>}
      </section>
    </div>
  )
}
