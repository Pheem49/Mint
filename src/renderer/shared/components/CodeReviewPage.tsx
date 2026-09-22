import { useState } from 'react'
import type { FileChange } from '../types'

interface Props {
  title: string
  changes: FileChange[]
  onBack: () => void
}

function fileName(path: string) {
  return path.split('/').pop() || path
}

function diffLines(text: string) {
  if (!text) return []
  return text.replace(/\n$/, '').split('\n')
}

export default function CodeReviewPage({ title, changes, onBack }: Props) {
  const [selectedPath, setSelectedPath] = useState(changes[0]?.path ?? '')
  const selected = changes.find((change) => change.path === selectedPath) ?? changes[0]
  const additions = changes.reduce((sum, change) => sum + change.additions, 0)
  const deletions = changes.reduce((sum, change) => sum + change.deletions, 0)

  return (
    <main className="code-review-page">
      <header className="code-review-toolbar">
        <button type="button" className="code-review-back" onClick={onBack}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m18 6-12 12M6 6l12 12" /></svg>
          Close review
        </button>
        <span className="code-review-tab"><span aria-hidden="true">▣</span> Review</span>
        <span className="code-review-toolbar-title">{title}</span>
      </header>

      <div className="code-review-summary">
        <span>Last turn</span>
        <span className="file-changes-count-add">+{additions}</span>
        <span className="file-changes-count-del">-{deletions}</span>
        <span className="code-review-file-total">{changes.length} {changes.length === 1 ? 'file' : 'files'}</span>
      </div>

      <div className="code-review-layout">
        <section className="code-review-diff" aria-label="Code changes">
          {selected ? (
            <>
              <div className="code-review-file-heading">
                <span className="code-review-filetype">{fileName(selected.path).split('.').pop()?.toUpperCase() ?? 'FILE'}</span>
                <span className="code-review-path">{selected.path}</span>
                {selected.created && <span className="file-changes-badge-new">NEW FILE</span>}
                <span className="file-changes-count-add">+{selected.additions}</span>
                <span className="file-changes-count-del">-{selected.deletions}</span>
              </div>
              {selected.hunks.length > 0 ? (
                <div className="code-review-code">
                  {selected.hunks.map((hunk, hunkIndex) => {
                    const oldLines = diffLines(hunk.oldText)
                    const newLines = diffLines(hunk.newText)
                    return (
                      <div className="code-review-hunk" key={`${selected.path}-${hunkIndex}`}>
                        {oldLines.map((line, index) => (
                          <div className="code-review-line is-deleted" key={`old-${index}`}>
                            <span className="code-review-line-number">{index + 1}</span><span className="code-review-sign">−</span><code>{line || ' '}</code>
                          </div>
                        ))}
                        {newLines.map((line, index) => (
                          <div className="code-review-line is-added" key={`new-${index}`}>
                            <span className="code-review-line-number">{index + 1}</span><span className="code-review-sign">+</span><code>{line || ' '}</code>
                          </div>
                        ))}
                      </div>
                    )
                  })}
                </div>
              ) : (
                <div className="code-review-empty">No diff details were captured for this file.</div>
              )}
            </>
          ) : <div className="code-review-empty">No changed files to review.</div>}
        </section>

        <aside className="code-review-file-list" aria-label="Changed files">
          <div className="code-review-file-list-title">Changed files <span>{changes.length}</span></div>
          {changes.map((change) => (
            <button
              type="button"
              key={change.path}
              className={`code-review-file-option ${selected?.path === change.path ? 'is-selected' : ''}`}
              onClick={() => setSelectedPath(change.path)}
              title={change.path}
            >
              <span className="code-review-file-option-copy"><strong>{fileName(change.path)}</strong><small>{change.path}</small></span>
              <span className="code-review-file-option-counts"><span className="file-changes-count-add">+{change.additions}</span><span className="file-changes-count-del">-{change.deletions}</span></span>
            </button>
          ))}
        </aside>
      </div>
    </main>
  )
}
