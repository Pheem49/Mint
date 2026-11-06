/// <reference path="../../env.d.ts" />
import { describe, it, expect } from 'vitest'
// @ts-ignore react-dom/server lacks types without @types/react-dom
import { renderToStaticMarkup } from 'react-dom/server'
import React from 'react'
import { AttachedDocumentCard } from './AttachedDocumentCard'

describe('AttachedDocumentCard', () => {
  it('renders collapsed card header with filename, lines, and View content label', () => {
    const doc = {
      filename: 'LICENSE.electron.txt',
      content: 'Copyright (c) Electron contributors\nPermission is hereby granted.',
      lineCount: 2,
      charCount: 65,
    }

    const html = renderToStaticMarkup(<AttachedDocumentCard document={doc} defaultOpen={false} />)
    expect(html).toContain('LICENSE.electron.txt')
    expect(html).toContain('2 lines')
    expect(html).toContain('View content')
    // When defaultOpen is false, body is not rendered
    expect(html).not.toContain('Permission is hereby granted.')
  })

  it('renders expanded body with full text when defaultOpen is true', () => {
    const doc = {
      filename: 'sample.txt',
      content: 'Hello world from attached file!',
      lineCount: 1,
      charCount: 31,
    }

    const html = renderToStaticMarkup(<AttachedDocumentCard document={doc} defaultOpen={true} />)
    expect(html).toContain('sample.txt')
    expect(html).toContain('1 line')
    expect(html).toContain('Hide content')
    expect(html).toContain('Hello world from attached file!')
  })
})
