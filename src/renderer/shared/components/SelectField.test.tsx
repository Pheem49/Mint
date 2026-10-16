import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import SelectField from './SelectField'

describe('Mint dropdown field', () => {
  it('preserves a controlled numeric checkpoint and its accessible label', async () => {
    const html = renderToStaticMarkup(<SelectField value={2} id="checkpoint" aria-label="Target checkpoint" disabled>
      <option value={1}>Step 1</option><option value={2}>Step 2</option>
    </SelectField>)
    expect(html).toContain('Step 2')
    expect(html).toContain('role="combobox"')
    expect(html).toContain('id="checkpoint"')
    expect(html).toContain('aria-label="Target checkpoint"')
    expect(html).toContain('disabled=""')
  })
  it('preserves conditional and mapped labels, including a custom model entry', async () => {
    const html = renderToStaticMarkup(<SelectField value="custom">
      {['a','b'].map(v=><option key={v} value={v}>{v}</option>)}
      {false && <option value="hidden">Hidden</option>}
      <option value="custom">Custom model ID…</option>
    </SelectField>)
    expect(html).toContain('Custom model ID…')
    expect(html).not.toContain('Hidden')
  })
})
