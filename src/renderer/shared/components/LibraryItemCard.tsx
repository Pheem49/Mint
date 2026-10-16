import { Download, FileText, Image, Trash2, Undo2, Video } from 'lucide-react'
import type { LibraryItem } from '../types'
import { useLibraryFile } from '../library/useLibraryFile'

function formatSize(size: number) {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / 1024 / 1024).toFixed(1)} MB`
}

function Thumbnail({ item }: { item: LibraryItem }) {
  const enabled = item.thumbnailStatus === 'ready' && !item.deletedAt && item.availability === 'ready'
  const file = useLibraryFile(enabled ? item.id : null, true)
  const Icon = item.kind === 'image' ? Image : item.kind === 'video' ? Video : FileText
  return (
    <span className={`library-thumbnail is-${item.kind}`}>
      {file.url ? <img src={file.url} alt="" loading="lazy" /> : <Icon size={23} />}
    </span>
  )
}

export default function LibraryItemCard({ item, linked, trashed, actionPending, downloadPending, onPreview, onDownload, onMove, onRemove }: {
  item: LibraryItem
  linked: boolean
  trashed: boolean
  actionPending: boolean
  downloadPending: boolean
  onPreview: (item: LibraryItem) => void
  onDownload: (item: LibraryItem) => void
  onMove: (item: LibraryItem) => void
  onRemove: (item: LibraryItem) => void
}) {
  const missing = item.availability === 'missing'
  const disabled = missing || !!item.deletedAt
  const status = missing ? 'Original file missing' : item.deletedAt ? 'In Trash'
    : item.origin === 'generated' ? 'AI generated' : item.origin === 'legacy' ? 'Imported from Pictures' : 'Uploaded'
  return (
    <article className={`library-item ${linked ? 'is-linked' : ''}`}>
      <button className="library-item-preview" data-preview={item.id} type="button" disabled={disabled}
        onClick={() => onPreview(item)} aria-label={`Preview ${item.name}`}>
        <Thumbnail item={item} />
        <span className="library-item-copy">
          <strong title={item.name}>{item.name}</strong>
          <small>{status} · {formatSize(item.size)}</small>
        </span>
      </button>
      <span className="library-item-date">
        {new Date(item.createdAt).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' })}
      </span>
      <div className="library-item-actions">
        {!trashed && (
          <button type="button" aria-label={`Download ${item.name}`} disabled={disabled || downloadPending} onClick={() => onDownload(item)}>
            <Download size={16} />
          </button>
        )}
        <button type="button" aria-label={trashed ? `Restore ${item.name}` : `Move ${item.name} to Trash`}
          data-trash={trashed ? undefined : item.id} data-restore={trashed ? item.id : undefined}
          disabled={actionPending} onClick={() => onMove(item)}>
          {trashed ? <Undo2 size={16} /> : <Trash2 size={16} />}
        </button>
        {trashed && (
          <button type="button" data-remove={item.id} disabled={actionPending}
            aria-label={`Remove ${item.name} from Mint`}
            title="Remove from Mint and move files to system Trash"
            onClick={() => onRemove(item)}>
            <Trash2 size={16} />
          </button>
        )}
      </div>
    </article>
  )
}
