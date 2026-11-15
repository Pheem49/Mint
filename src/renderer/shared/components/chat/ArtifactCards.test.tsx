/// <reference path="../../env.d.ts" />
import { describe, it, expect, vi } from 'vitest'
// @ts-ignore react-dom/server lacks types without @types/react-dom
import { renderToStaticMarkup } from 'react-dom/server'
import React from 'react'
import { ArtifactCards, extractArtifacts } from './ArtifactCards'

describe('ArtifactCards', () => {
  it('extracts pdf and pptx files explicitly mentioned in aiText and converts to workspace-relative path', () => {
    const aiText = 'ไฟล์สไลด์: `/home/pheem49/vscode/Project/Demo/AI_and_CLI_Presentation.pdf` และ `slides.pptx` รวมถึง `index.html`'
    const artifacts = extractArtifacts([], aiText, '/home/pheem49/vscode/Project/Demo')
    // Only PDF and PPTX, index.html must be ignored
    expect(artifacts.length).toBe(2)
    // AI_and_CLI_Presentation.pdf should have relative path
    const pdf = artifacts.find(a => a.name === 'AI_and_CLI_Presentation.pdf')
    expect(pdf).toBeDefined()
    expect(pdf?.path).toBe('AI_and_CLI_Presentation.pdf')
    expect(pdf?.type).toBe('pdf')

    const pptx = artifacts.find(a => a.name === 'slides.pptx')
    expect(pptx).toBeDefined()
    expect(pptx?.type).toBe('presentation')
  })

  it('does not extract random files from terminal ls or shell output', () => {
    const progress: any[] = [
      {
        type: 'ToolEnd',
        data: {
          action: 'run_shell',
          input: { command: 'ls -la' },
          result: 'file1.pdf file2.pdf old_presentation.pdf',
        },
      },
    ]
    // If aiText doesn't mention them, no cards are created
    const artifacts = extractArtifacts(progress, 'ฉันได้ตรวจสอบโฟลเดอร์เรียบร้อยแล้วค่ะ', '/home/user')
    expect(artifacts.length).toBe(0)
  })

  it('renders card with relative file path and Open in Side Panel button', () => {
    const onOpenArtifact = vi.fn()
    const aiText = 'สร้างไฟล์เสร็จแล้ว: `/home/user/workspace/presentation.pdf`'
    const html = renderToStaticMarkup(
      <ArtifactCards
        aiText={aiText}
        workspacePath="/home/user/workspace"
        onOpenArtifact={onOpenArtifact}
      />
    )

    expect(html).toContain('presentation.pdf')
    expect(html).toContain('PDF')
    expect(html).toContain('Open in Side Panel')
  })

  it('renders cards with draggable attribute so users can drag into chat', () => {
    const aiText = 'ไฟล์สไลด์: `presentation.pdf`'
    const html = renderToStaticMarkup(
      <ArtifactCards
        aiText={aiText}
        workspacePath="/home/user/workspace"
        onOpenArtifact={() => {}}
      />
    )
    expect(html).toMatch(/class="chat-artifact-card[^"]*"[^>]*draggable="true"/)
    expect(html).toContain('width:100%')
  })
})
