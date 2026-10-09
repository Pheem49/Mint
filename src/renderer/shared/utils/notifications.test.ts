import { afterEach, expect, it, vi } from 'vitest'
import { sendBrowserNotification, clearBrowserNotifications, deliverNotification, receiveProactiveNotification } from './notifications'

const shown: any[] = []
class BrowserNotification {
  static permission = 'granted'
  close = vi.fn()
  onclick?: () => void
  onclose?: () => void
  constructor(public title: string, public options: any) { shown.push(this) }
}
afterEach(() => { clearBrowserNotifications(); shown.length = 0; vi.unstubAllGlobals() })
it('closes every outstanding notification when returning to Mint', async () => {
  vi.stubGlobal('Notification', BrowserNotification)
  await sendBrowserNotification('One')
  await sendBrowserNotification('Two')
  expect(shown).toHaveLength(2)
  clearBrowserNotifications()
  expect(shown.every(n => n.close.mock.calls.length === 1)).toBe(true)
})
it('opens the originating chat on click and dismisses that notification', async () => {
  vi.stubGlobal('Notification', BrowserNotification)
  const opened: string[] = []
  await sendBrowserNotification('Done', 'chat-one', id => opened.push(id))
  shown[0].onclick()
  expect(opened).toEqual(['chat-one'])
  expect(shown[0].close).toHaveBeenCalledOnce()
})
it('reports a rejected delivery without propagating it into the chat run', async () => {
  const errors: string[] = []
  await expect(deliverNotification(() => Promise.reject(new Error('No service')), message => errors.push(message))).resolves.toBeUndefined()
  expect(errors).toEqual(['Could not send notification: No service'])
})
it('shows a background event in-app and sends its originating chat to the OS', async () => {
  const notices: unknown[] = []
  const deliveries: unknown[] = []
  await receiveProactiveNotification({ message: 'Task completed', type: 'info', chatId: 'task-chat' }, true,
    notice => notices.push(notice), async (body, chatId) => { deliveries.push({ body, chatId }) })
  expect(notices).toEqual([{ message: 'Task completed', type: 'info', chatId: 'task-chat' }])
  expect(deliveries).toEqual([{ body: 'Task completed', chatId: 'task-chat' }])
})
it('keeps foreground system events in-app and ignores malformed events', async () => {
  const notices: unknown[] = []
  const deliveries: unknown[] = []
  const show = (notice: unknown) => { notices.push(notice) }
  const send = async (body: string) => { deliveries.push(body) }
  await receiveProactiveNotification({ message: 'Battery low', type: 'warning' }, false, show, send)
  await receiveProactiveNotification({ message: '' }, true, show, send)
  expect(notices).toEqual([{ message: 'Battery low', type: 'warning', chatId: undefined }])
  expect(deliveries).toEqual([])
})
