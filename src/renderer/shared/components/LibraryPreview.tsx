import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Download, X } from 'lucide-react';
import type { LibraryItem } from '../types';
import { useLibraryFile } from '../library/useLibraryFile';
export default function LibraryPreview({ item, onClose, onOpenConversation }: {
  item: LibraryItem;
  onClose: () => void;
  onOpenConversation?: (id: string) => void;
}) {
  const [retry, setRetry] = useState(0);
  const file = useLibraryFile(item.id, false, retry);
  const [text, setText] = useState('');
  const dialog = useRef<HTMLDivElement>(null);
  const sourceMessage = item.message
    .replace(/^\/chat\s+/, '')
    .replace(/\[(?:Image|Video) #\d+\]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  const isText = item.kind === 'file' && /\.(txt|md|json|toml|ya?ml|rs|tsx?|jsx?|py|html?|css)$/i.test(item.name);
  useEffect(() => { let active = true; setText(''); if (isText && file.blob)
    file.blob.slice(0, 1024 * 1024).text().then(value => { if (active)
      setText(value); }); return () => { active = false; }; }, [file.blob, isText]);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    dialog.current?.querySelector<HTMLElement>('button')?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        onClose();
      }
      if (event.key === 'Tab') {
        const nodes = Array.from(dialog.current?.querySelectorAll<HTMLElement>('button:not([disabled]),a[href],video[controls],iframe,[tabindex="0"]') ?? []);
        const first = nodes[0], last = nodes.at(-1);
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last?.focus();
        }
        else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first?.focus();
        }
      }
    };
    document.addEventListener('keydown', onKey);
    return () => { document.body.style.overflow = overflow; document.removeEventListener('keydown', onKey); previous?.focus(); };
  }, [onClose]);
  return createPortal(<div className="library-preview-backdrop" onClick={e => { if (e.target === e.currentTarget)
    onClose(); }}><div className="library-preview" ref={dialog} role="dialog" aria-modal="true" aria-labelledby="library-preview-title">
  <header><h2 id="library-preview-title">{item.name}</h2><button type="button" onClick={onClose} aria-label="Close preview"><X size={18}/></button></header>
  {sourceMessage && <section className="library-preview-source" aria-label="Source message"><span>Source message</span><p>{sourceMessage}</p></section>}
  <div className="library-preview-body">
   {file.error ? <div role="alert"><p>{file.error}</p><button type="button" onClick={() => setRetry(n => n + 1)}>Retry</button></div> : !file.url ? <p role="status">Loading file…</p> : item.kind === 'image' ? <img src={file.url} alt={item.name}/> : item.kind === 'video' ? <video src={file.url} controls autoPlay={false}/> : isText ? <pre tabIndex={0}>{text}{file.blob!.size > 1024 * 1024 ? '\n[Preview limited to the first 1 MiB. Download to read the entire file.]' : ''}</pre> : /\.pdf$/i.test(item.name) ? <iframe title={item.name} src={file.url} sandbox=""/> : <p>This file is ready to download. A preview is not available for this format.</p>}
  </div>
  <footer><span>{item.origin === 'generated' ? 'AI generated' : item.origin === 'legacy' ? 'Imported from Pictures' : 'Uploaded'} · {item.mimeType}</span><div>{[...new Set(item.references.map(r => r.chatId))].map(id => <button type="button" key={id} data-open-chat onClick={() => onOpenConversation?.(id)}>Open source chat</button>)}{file.url && <a href={file.url} download={item.name}><Download size={15}/>Download</a>}</div></footer>
 </div></div>, document.body);
}
