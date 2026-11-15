/// <reference path="../../env.d.ts" />
import { describe, it, expect } from 'vitest'
import { parseAttachedDocument } from './attachedDocument'

describe('parseAttachedDocument', () => {
  it('returns raw text unchanged when no document attachment pattern is present', () => {
    const raw = 'สวัสดีครับ ช่วยดูโค้ดนี้หน่อย'
    const res = parseAttachedDocument(raw)
    expect(res.userText).toBe(raw)
    expect(res.attachedDocument).toBeUndefined()
  })

  it('handles null, undefined, and empty string', () => {
    expect(parseAttachedDocument(null).userText).toBe('')
    expect(parseAttachedDocument(undefined).userText).toBe('')
    expect(parseAttachedDocument('').userText).toBe('')
  })

  it('parses user text and attached document payload accurately', () => {
    const raw = `@generate_harness_pdf.py ขอปรับหน่อยอะ ไฟล์ที่ลากมาวา

Attached document: LICENSE.electron.txt
Use the extracted document text below when answering. If the user asks for a summary, summarize this document.

--- Extracted document text ---
Copyright (c) Electron contributors
Copyright (c) 2013-2020 GitHub Inc.

Permission is hereby granted, free of charge.
--- End extracted document text ---`

    const res = parseAttachedDocument(raw)
    expect(res.userText).toBe('@generate_harness_pdf.py ขอปรับหน่อยอะ ไฟล์ที่ลากมาวา')
    expect(res.attachedDocument).toBeDefined()
    expect(res.attachedDocument?.filename).toBe('LICENSE.electron.txt')
    expect(res.attachedDocument?.lineCount).toBe(4)
    expect(res.attachedDocument?.content).toContain('Copyright (c) Electron contributors')
    expect(res.attachedDocument?.content).toContain('Permission is hereby granted, free of charge.')
  })

  it('handles when the user sent only an attachment without typing any message', () => {
    const raw = `Attached document: notes.md
Use the extracted document text below when answering. If the user asks for a summary, summarize this document.

--- Extracted document text ---
# Notes
Here are the notes.
--- End extracted document text ---`

    const res = parseAttachedDocument(raw)
    expect(res.userText).toBe('')
    expect(res.attachedDocument?.filename).toBe('notes.md')
    expect(res.attachedDocument?.lineCount).toBe(2)
  })

  it('handles truncated document notice properly', () => {
    const raw = `Check this file

Attached document: large.json
Use the extracted document text below when answering. If the user asks for a summary, summarize this document.

--- Extracted document text ---
{"key": "val"}

[Document text truncated because it is long.]
--- End extracted document text ---`

    const res = parseAttachedDocument(raw)
    expect(res.userText).toBe('Check this file')
    expect(res.attachedDocument?.filename).toBe('large.json')
    expect(res.attachedDocument?.content).toContain('[Document text truncated because it is long.]')
  })
})
