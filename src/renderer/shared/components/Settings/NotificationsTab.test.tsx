import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it } from 'vitest'
import NotificationsTab from './NotificationsTab'

it('shows notification categories with accessible switches and permission controls', () => {
  const html = renderToStaticMarkup(<NotificationsTab settings={{ enabled: false }} onChange={() => {}} />)
  expect(html).toContain('AI replies')
  expect(html).toContain('Approval requests')
  expect(html).toContain('System &amp; background tasks')
  expect(html).toContain('aria-label="Enable notifications"')
  expect(html).toContain('disabled=""')
  expect(html).toContain('Check permission')
  expect(html).toContain('Send test notification')
})
