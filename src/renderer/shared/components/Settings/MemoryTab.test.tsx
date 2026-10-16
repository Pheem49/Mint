import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import MemoryTab from './MemoryTab'

const render = (preferences: string) => renderToStaticMarkup(<MemoryTab
  userName="Pheem"
  setUserName={() => { throw new Error('Rendering must not rewrite the name') }}
  userPreferences={preferences}
  setUserPreferences={() => { throw new Error('Rendering must not rewrite preferences') }}
/>)

describe('Memory & Profile reading view', () => {
  it('preserves the original text and line breaks in a continuous reading view', () => {
    const preferences = 'Talk in Thai; Keep explanations concise;\nPrefer TypeScript.'
    const html = render(preferences)
    expect(html).toContain(preferences)
    expect(html).not.toContain('<li')
    expect(html).toContain('Edit instructions')
    expect(html).not.toContain('<textarea')
  })

  it('offers an explicit way to add instructions when memory is empty', () => {
    const html = render('  ')
    expect(html).toContain('Add instructions')
    expect(html).not.toContain('<li')
  })

  it('associates the editable name with a label and explains its scope', () => {
    const html = render('Keep it short.')
    expect(html).toMatch(/<label[^>]* for="([^"]+)"[^>]*>What should Mint call you\?<\/label>/)
    const id = html.match(/<label[^>]* for="([^"]+)"/)![1]
    expect(html).toContain(`id="${id}"`)
    expect(html).toContain('value="Pheem"')
    expect(html).toContain('Across all sessions and projects')
  })
})
