import { expect, it } from 'vitest'
import { shouldNotify } from './notificationSettings'

it('preserves existing notifications when settings have not been saved yet', () => {
  expect(shouldNotify(null, 'replies', 'os')).toBe(true)
})
it('applies master, category, and delivery switches independently', () => {
  expect(shouldNotify({ enabled: false }, 'approvals', 'inApp')).toBe(false)
  expect(shouldNotify({ replies: false }, 'replies', 'os')).toBe(false)
  expect(shouldNotify({ replies: false }, 'approvals', 'os')).toBe(true)
  expect(shouldNotify({ os: false }, 'system', 'os')).toBe(false)
  expect(shouldNotify({ os: false }, 'system', 'inApp')).toBe(true)
  expect(shouldNotify({ inApp: false }, 'system', 'inApp')).toBe(false)
})
