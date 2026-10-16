const active = new Set<Notification>()
let generation = 0

export async function sendBrowserNotification(body: string, chatId?: string, openChat?: (id: string) => void): Promise<void> {
  if (typeof Notification === 'undefined') throw new Error('Notifications are not supported here.')
  const started = generation
  let permission = Notification.permission
  if (permission === 'default') permission = await Notification.requestPermission()
  if (permission !== 'granted') throw new Error('Notification permission is not granted.')
  // Returning to Mint while a permission prompt was pending cancels this delivery.
  if (started !== generation) return
  const notification = new Notification('Mint Agent', { body })
  active.add(notification)
  notification.onclose = () => active.delete(notification)
  notification.onclick = () => {
    notification.close()
    active.delete(notification)
    if (chatId) openChat?.(chatId)
    else if (typeof window !== 'undefined') window.focus()
  }
}

export function clearBrowserNotifications(): void {
  generation++
  for (const notification of active) notification.close()
  active.clear()
}

export async function deliverNotification(send: () => unknown, report: (message: string) => void): Promise<void> {
  try { await send() }
  catch (error) { report('Could not send notification: ' + (error instanceof Error ? error.message : String(error))) }
}

export function openNotificationChat(chatId: string): void {
  window.focus()
  window.dispatchEvent(new CustomEvent('mint:notification-click', { detail: { chatId } }))
}

export type NotificationNotice = { message: string; chatId?: string; type?: string }
export async function receiveProactiveNotification(
  payload: unknown, background: boolean, show: (notice: NotificationNotice) => void,
  send: (body: string, chatId?: string) => Promise<void>,
): Promise<void> {
  if (!payload || typeof payload !== 'object') return
  const data = payload as Record<string, unknown>
  if (typeof data.message !== 'string' || !data.message.trim()) return
  const notice = {
    message: data.message,
    chatId: typeof data.chatId === 'string' ? data.chatId : undefined,
    type: typeof data.type === 'string' ? data.type : undefined,
  }
  show(notice)
  if (background) await send(notice.message, notice.chatId)
}
