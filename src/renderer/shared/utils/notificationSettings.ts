export const DEFAULT_NOTIFICATION_SETTINGS = {
  enabled: true, replies: true, approvals: true, system: true, os: true, inApp: true,
}
export type NotificationSettings = typeof DEFAULT_NOTIFICATION_SETTINGS
export function readNotificationSettings(settings: unknown): NotificationSettings {
  const data = settings && typeof settings === 'object' ? settings as Record<string, unknown> : {}
  return Object.fromEntries(Object.entries(DEFAULT_NOTIFICATION_SETTINGS).map(([key, fallback]) =>
    [key, typeof data[key] === 'boolean' ? data[key] : fallback])) as NotificationSettings
}
export function shouldNotify(settings: unknown, kind: 'replies' | 'approvals' | 'system', channel: 'os' | 'inApp'): boolean {
  const preferences = readNotificationSettings(settings)
  return preferences.enabled && preferences[kind] && preferences[channel]
}

export async function notificationPermission(request = false): Promise<NotificationPermission | 'unsupported'> {
  if ((window as any).__TAURI_INTERNALS__) {
    const api = await import('@tauri-apps/plugin-notification')
    if (await api.isPermissionGranted()) return 'granted'
    return request ? api.requestPermission() : 'default'
  }
  if (typeof Notification === 'undefined') return 'unsupported'
  return request ? Notification.requestPermission() : Notification.permission
}
