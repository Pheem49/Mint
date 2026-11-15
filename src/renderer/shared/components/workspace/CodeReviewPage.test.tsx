import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import CodeReviewPage from './CodeReviewPage'
import type { FileChange } from '../../types'

describe('CodeReviewPage', () => {
  const sampleChanges: FileChange[] = [
    {
      path: 'src/generate_harness_pdf.py',
      created: false,
      additions: 10,
      deletions: 2,
      hunks: [
        {
          oldStart: 1,
          oldLines: 1,
          newStart: 1,
          newLines: 2,
          oldText: 'print("hello")',
          newText: 'print("hello")\nprint("world")',
        },
      ],
    },
  ]

  it('renders tree items with draggable attributes and drag tooltip for chat input', () => {
    const html = renderToStaticMarkup(
      <CodeReviewPage
        changes={sampleChanges}
        workspacePath="/home/user/project"
      />
    )

    // Tree file button is draggable
    expect(html).toContain('class="code-review-tree-file')
    expect(html).toMatch(/class="code-review-tree-file[^"]*"[^>]*draggable="true"/)
    expect(html).toContain('title="src/generate_harness_pdf.py (Drag to chat)"')

    // Tree folder button is draggable
    expect(html).toContain('class="code-review-tree-folder-btn"')
    expect(html).toMatch(/class="code-review-tree-folder-btn"[^>]*draggable="true"/)
    expect(html).toContain('title="src (Drag to chat)"')

    // Diff view file bar path is draggable
    expect(html).toContain('class="code-review-file-bar-path"')
    expect(html).toMatch(/class="code-review-file-bar-path"[^>]*draggable="true"/)
    expect(html).toContain('title="src/generate_harness_pdf.py (Drag to chat)"')
  })
})
