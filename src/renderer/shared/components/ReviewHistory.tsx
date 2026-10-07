import { useEffect, useRef } from 'react'
import type { WorkspaceHistoryEntry } from '../types'

export function undoDescription(entry: WorkspaceHistoryEntry) {
  switch (entry.kind) {
    case 'edit': return entry.backup ? 'Restore the saved content from before this edit.' : 'Move the file created by this edit to Trash.'
    case 'create': return 'Move this created file or folder to Trash.'
    case 'trash': return 'Restore this file or folder to its original location.'
    case 'move': return `Move ${entry.destination || entry.path} back to ${entry.path}.`
    default: return entry.label
  }
}

export function HistoryDetails({ entry }: { entry: WorkspaceHistoryEntry }) {
  return <dl className="review-history-details">
    <dt>File</dt><dd>{entry.path}</dd>
    <dt>Action</dt><dd>{entry.label.replace(/^Undo /, '')}</dd>
    {entry.destination && <><dt>Destination</dt><dd>{entry.destination}</dd></>}
    <dt>Undo will</dt><dd>{undoDescription(entry)}</dd>
  </dl>
}

export function UndoConfirmation({ entry, busy, error, onCancel, onConfirm }: {
  entry: WorkspaceHistoryEntry; busy: boolean; error: string; onCancel: () => void; onConfirm: () => void
}) {
  const dialog = useRef<HTMLDialogElement>(null)
  useEffect(() => {
    const element = dialog.current
    element?.showModal()
    return () => element?.close()
  }, [])
  return <dialog ref={dialog} className="review-undo-dialog" aria-labelledby="review-undo-title" onCancel={event => { event.preventDefault(); if (!busy) onCancel() }}>
    <h3 id="review-undo-title">Confirm Undo</h3>
    <HistoryDetails entry={entry} />
    <p>Undo is refused if the file has changed since this action or the destination is occupied.</p>
    {error && <p role="alert">{error}</p>}
    <div className="review-undo-actions">
      <button type="button" autoFocus disabled={busy} onClick={onCancel}>Cancel</button>
      <button type="button" disabled={busy} onClick={onConfirm}>{busy ? 'Undoing…' : 'Confirm Undo'}</button>
    </div>
  </dialog>
}
