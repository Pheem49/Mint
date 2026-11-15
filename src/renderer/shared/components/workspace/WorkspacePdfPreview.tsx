import { useEffect, useRef, useState, useCallback } from 'react'
import type { PDFDocumentProxy } from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'
import { readPdfRange } from '../../utils/pdfRange'
import WorkspaceFileInfo from './WorkspaceFileInfo'
import '../../css/workspace-media-preview.css'

function PdfPageItem({
  pdf,
  pageNumber,
  width,
  zoom,
  onVisible,
}: {
  pdf: PDFDocumentProxy
  pageNumber: number
  width: number
  zoom: number
  onVisible?: (page: number) => void
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    let cancelled = false
    let renderTask: any = null

    void pdf.getPage(pageNumber).then((page) => {
      if (cancelled || !canvasRef.current) return
      const normal = page.getViewport({ scale: 1 })
      const viewport = page.getViewport({ scale: (width / normal.width) * zoom })
      const output = Math.min(
        window.devicePixelRatio || 1,
        2,
        Math.sqrt(8_000_000 / (viewport.width * viewport.height))
      )
      const canvas = canvasRef.current
      canvas.width = Math.ceil(viewport.width * output)
      canvas.height = Math.ceil(viewport.height * output)
      canvas.style.width = `${viewport.width}px`
      canvas.style.height = `${viewport.height}px`

      renderTask = page.render({
        canvas,
        viewport,
        transform: output === 1 ? undefined : [output, 0, 0, output, 0, 0],
      })
      return renderTask.promise
    }).catch(() => {})

    return () => {
      cancelled = true
      renderTask?.cancel()
    }
  }, [pdf, pageNumber, width, zoom])

  useEffect(() => {
    if (!containerRef.current || !onVisible) return
    const el = containerRef.current
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting && entry.intersectionRatio >= 0.4) {
            onVisible(pageNumber)
          }
        }
      },
      { threshold: 0.4 }
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [pageNumber, onVisible])

  return (
    <div
      ref={containerRef}
      id={`pdf-page-${pageNumber}`}
      className="workspace-pdf-page-container"
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        marginBottom: '24px',
        position: 'relative',
        width: '100%',
      }}
    >
      <canvas
        ref={canvasRef}
        style={{
          boxShadow: '0 6px 24px rgba(0, 0, 0, 0.4), 0 0 0 1px rgba(255, 255, 255, 0.08)',
          borderRadius: 'var(--radius-xs, 6px)',
          display: 'block',
          background: '#ffffff',
          maxWidth: '100%',
        }}
        aria-label={`PDF page ${pageNumber}`}
      />
      <span
        style={{
          marginTop: '8px',
          fontSize: '11px',
          color: 'var(--text-muted, #a1a1aa)',
          fontWeight: 500,
        }}
      >
        Slide {pageNumber} of {pdf.numPages}
      </span>
    </div>
  )
}

export default function WorkspacePdfPreview({
  url,
  path,
  root,
  byteLength,
  range,
}: {
  url: string
  path: string
  root?: string
  byteLength?: number
  range?: boolean
}) {
  const [pdf, setPdf] = useState<PDFDocumentProxy | null>(null)
  const [pageNumber, setPageNumber] = useState(1)
  const [zoom, setZoom] = useState(1)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [retry, setRetry] = useState(0)
  const [width, setWidth] = useState(500)
  const area = useRef<HTMLDivElement>(null)

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
    setPdf(null)
    setError('')
    setLoading(true)
    setPageNumber(1)

    void (async () => {
      const library = await import('pdfjs-dist/legacy/build/pdf.mjs')
      if (cancelled) return
      library.GlobalWorkerOptions.workerSrc = workerUrl
      const assets = import.meta.env.DEV
        ? new URL('/pdfjs/', location.href).href
        : new URL(['..', 'pdfjs', ''].join('/'), import.meta.url).href
      const options = {
        cMapUrl: `${assets}cmaps/`,
        cMapPacked: true,
        standardFontDataUrl: `${assets}standard_fonts/`,
        wasmUrl: `${assets}wasm/`,
        useSystemFonts: true,
        isEvalSupported: false,
        disableAutoFetch: true,
        disableStream: true,
      }
      if (range && byteLength) {
        class FileRanges extends library.PDFDataRangeTransport {
          requestDataRange(begin: number, end: number) {
            const controller = new AbortController()
            controllers.add(controller)
            void readPdfRange(url, begin, end, controller.signal)
              .then((bytes) => {
                if (!cancelled) this.onDataRange(begin, bytes)
              })
              .catch((reason) => {
                if (!cancelled) {
                  setError(String(reason instanceof Error ? reason.message : reason))
                  setLoading(false)
                  void task?.destroy()
                }
              })
              .finally(() => controllers.delete(controller))
          }
          abort() {
            for (const controller of controllers) controller.abort()
            controllers.clear()
          }
        }
        task = library.getDocument({
          ...options,
          range: new FileRanges(byteLength, new Uint8Array()),
          rangeChunkSize: 64 * 1024,
        })
      } else {
        task = library.getDocument({ ...options, url })
      }
      const document = await task.promise
      if (!cancelled) {
        setPdf(document)
        setLoading(false)
      }
    })().catch((reason) => {
      if (!cancelled) {
        setError(reason instanceof Error ? reason.message : String(reason))
        setLoading(false)
      }
    })

    return () => {
      cancelled = true
      for (const controller of controllers) controller.abort()
      void task?.destroy()
    }
  }, [url, byteLength, range, retry])

  const scrollToPage = useCallback((p: number) => {
    if (!pdf) return
    const target = Math.max(1, Math.min(pdf.numPages, p))
    setPageNumber(target)
    const el = document.getElementById(`pdf-page-${target}`)
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'start' })
    }
  }, [pdf])

  useEffect(() => {
    if (!pdf) return
    const onKeyDown = (e: KeyboardEvent) => {
      const targetTag = (e.target as HTMLElement)?.tagName
      if (['INPUT', 'TEXTAREA'].includes(targetTag)) return
      if (e.key === 'ArrowRight' || e.key === 'ArrowDown' || e.key === 'PageDown' || e.key === ' ') {
        e.preventDefault()
        scrollToPage(pageNumber + 1)
      } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp' || e.key === 'PageUp') {
        e.preventDefault()
        scrollToPage(pageNumber - 1)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [pdf, pageNumber, scrollToPage])

  const pageCount = pdf?.numPages || 0
  const pages = Array.from({ length: pageCount }, (_, i) => i + 1)

  return (
    <section className="workspace-pdf-preview" aria-label="PDF preview">
      <div className="workspace-pdf-controls" role="toolbar" aria-label="PDF controls">
        <span style={{ fontWeight: 600, fontSize: '12px', color: 'var(--text-main)' }} aria-live="polite">
          Slide {pageNumber} / {pdf?.numPages ?? '–'}
        </span>
        <div style={{ height: '14px', width: '1px', background: 'var(--border)', margin: '0 4px' }} />
        <div style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
          <button
            type="button"
            aria-label="Zoom PDF out"
            disabled={zoom <= 0.5}
            onClick={() => setZoom((value) => Math.max(0.5, value - 0.25))}
          >
            −
          </button>
          <span style={{ fontSize: '11px', minWidth: '36px', textAlign: 'center' }}>{Math.round(zoom * 100)}%</span>
          <button
            type="button"
            aria-label="Zoom PDF in"
            disabled={zoom >= 2}
            onClick={() => setZoom((value) => Math.min(2, value + 0.25))}
          >
            +
          </button>
        </div>
      </div>

      <div className="workspace-pdf-pages" ref={area}>
        {loading && !error && <p role="status">Loading PDF…</p>}
        {error ? (
          <>
            <p className="workspace-media-error" role="alert">
              Could not display PDF: {error}
            </p>
            <button type="button" onClick={() => setRetry((value) => value + 1)}>
              Retry PDF
            </button>
            <WorkspaceFileInfo path={path} root={root} />
          </>
        ) : (
          pdf && (
            <div style={{ display: 'flex', flexDirection: 'column', width: '100%', alignItems: 'center' }}>
              {pages.map((num) => (
                <PdfPageItem
                  key={num}
                  pdf={pdf}
                  pageNumber={num}
                  width={width}
                  zoom={zoom}
                  onVisible={setPageNumber}
                />
              ))}
            </div>
          )
        )}
      </div>
    </section>
  )
}
