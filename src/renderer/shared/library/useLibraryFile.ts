import { useEffect, useState } from 'react';
import { libraryPlatform } from '../platform';
/** Own each object URL for exactly one mounted consumer, including late responses. */
export function useLibraryFile(id: string | null, thumbnail = false, revision = 0) {
  const [state, setState] = useState<{
    url: string;
    blob: Blob | null;
    error: string;
  }>({ url: '', blob: null, error: '' });
  useEffect(() => {
    let active = true;
    let url = '';
    setState({ url: '', blob: null, error: '' });
    if (id)
      libraryPlatform.readLibraryFile(id, thumbnail).then(blob => {
        if (!active)
          return;
        url = URL.createObjectURL(blob);
        setState({ url, blob, error: '' });
      }).catch(error => { if (active)
        setState({ url: '', blob: null, error: String(error) }); });
    return () => { active = false; if (url)
      URL.revokeObjectURL(url); };
  }, [id, thumbnail, revision]);
  return state;
}
export async function downloadLibraryFile(id: string, name: string) {
  const blob = await libraryPlatform.readLibraryFile(id);
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = name;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  // Allow the browser to start consuming the download before releasing its URL.
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
