import { useEffect, useRef, useState } from 'react'
import type { PDFDocumentProxy } from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'
import { readPdfRange } from '../utils/pdfRange'
import WorkspaceFileInfo from './WorkspaceFileInfo'
import '../css/workspace-media-preview.css'

export default function WorkspacePdfPreview({ url, path, root, byteLength, range }: { url: string; path: string; root?: string; byteLength?: number; range?: boolean }) {
  const [pdf, setPdf] = useState<PDFDocumentProxy | null>(null)
  const [pageNumber, setPageNumber] = useState(1)
  const [zoom, setZoom] = useState(1)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [retry, setRetry] = useState(0)
  const [width, setWidth] = useState(500)
  const area = useRef<HTMLDivElement>(null)
  const canvas = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const element = area.current
    if (!element) return
    const observer = new ResizeObserver(() => setWidth(Math.max(100, element.clientWidth - 32)))
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    let cancelled = false
    let task: ReturnType<typeof import('pdfjs-dist').getDocument> | undefined
    const controllers = new Set<AbortController>()
    setPdf(null); setError(''); setLoading(true); setPageNumber(1)
    void (async () => {
      const library = await import('pdfjs-dist/legacy/build/pdf.mjs')
      if (cancelled) return
      library.GlobalWorkerOptions.workerSrc = workerUrl
      const assets = import.meta.env.DEV ? new URL('/pdfjs/', location.href).href : new URL(['..', 'pdfjs', ''].join('/'), import.meta.url).href
      const options = { cMapUrl: `${assets}cmaps/`, cMapPacked: true, standardFontDataUrl: `${assets}standard_fonts/`, wasmUrl: `${assets}wasm/`, useSystemFonts: true, isEvalSupported: false, disableAutoFetch: true, disableStream: true }
      if (range && byteLength) {
        class FileRanges extends library.PDFDataRangeTransport {
          requestDataRange(begin: number, end: number) {
            const controller = new AbortController(); controllers.add(controller)
            void readPdfRange(url, begin, end, controller.signal).then(bytes => { if (!cancelled) this.onDataRange(begin, bytes) }).catch(reason => {
              if (!cancelled) { setError(String(reason instanceof Error ? reason.message : reason)); setLoading(false); void task?.destroy() }
            }).finally(() => controllers.delete(controller))
          }
          abort() { for (const controller of controllers) controller.abort(); controllers.clear() }
        }
        task = library.getDocument({ ...options, range: new FileRanges(byteLength, new Uint8Array()), rangeChunkSize: 64 * 1024 })
      } else task = library.getDocument({ ...options, url })
      const document = await task.promise
      if (!cancelled) setPdf(document)
    })().catch(reason => { if (!cancelled) { setError(reason instanceof Error ? reason.message : String(reason)); setLoading(false) } })
    return () => { cancelled = true; for (const controller of controllers) controller.abort(); void task?.destroy() }
  }, [url, byteLength, range, retry])

  useEffect(() => {
    if (!pdf || !canvas.current) return
    let cancelled = false
    let render: ReturnType<Awaited<ReturnType<PDFDocumentProxy['getPage']>>['render']> | undefined
    setLoading(true)
    void pdf.getPage(pageNumber).then(page => {
      if (cancelled || !canvas.current) return
      const normal = page.getViewport({ scale: 1 })
      const viewport = page.getViewport({ scale: width / normal.width * zoom })
      const output = Math.min(window.devicePixelRatio || 1, 2, Math.sqrt(8_000_000 / (viewport.width * viewport.height)))
      canvas.current.width = Math.ceil(viewport.width * output)
      canvas.current.height = Math.ceil(viewport.height * output)
      canvas.current.style.width = `${viewport.width}px`; canvas.current.style.height = `${viewport.height}px`
      render = page.render({ canvas: canvas.current, viewport, transform: output === 1 ? undefined : [output, 0, 0, output, 0, 0] })
      return render.promise
    }).then(() => { if (!cancelled) setLoading(false) }).catch(reason => { if (!cancelled) { setError(reason instanceof Error ? reason.message : String(reason)); setLoading(false) } })
    return () => { cancelled = true; render?.cancel() }
  }, [pdf, pageNumber, width, zoom])

  return <section className="workspace-pdf-preview" aria-label="PDF preview">
    <div className="workspace-pdf-controls" role="toolbar" aria-label="PDF controls">
      <button type="button" aria-label="Previous PDF page" disabled={!pdf || pageNumber <= 1} onClick={() => setPageNumber(value => value - 1)}>Previous</button>
      <span aria-live="polite">{pageNumber} / {pdf?.numPages ?? '–'}</span>
      <button type="button" aria-label="Next PDF page" disabled={!pdf || pageNumber >= pdf.numPages} onClick={() => setPageNumber(value => value + 1)}>Next</button>
      <button type="button" aria-label="Zoom PDF out" disabled={zoom <= .5} onClick={() => setZoom(value => Math.max(.5, value - .25))}>−</button>
      <span>{Math.round(zoom * 100)}%</span>
      <button type="button" aria-label="Zoom PDF in" disabled={zoom >= 2} onClick={() => setZoom(value => Math.min(2, value + .25))}>+</button>
    </div>
    <div className="workspace-pdf-pages" ref={area}>
      {loading && !error && <p role="status">Loading PDF…</p>}
      {error ? <><p className="workspace-media-error" role="alert">Could not display PDF: {error}</p><button type="button" onClick={() => setRetry(value => value + 1)}>Retry PDF</button><WorkspaceFileInfo path={path} root={root} /></> : <canvas ref={canvas} aria-label={`PDF page ${pageNumber}`} />}
    </div>
  </section>
}
