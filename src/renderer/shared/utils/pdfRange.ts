/** Read only the PDF bytes requested by its worker, including asset-protocol chunk limits. */
export async function readPdfRange(url: string, begin: number, end: number, signal: AbortSignal, request: typeof fetch = fetch): Promise<Uint8Array<ArrayBuffer>> {
  if (!Number.isSafeInteger(begin) || !Number.isSafeInteger(end) || begin < 0 || end <= begin) throw Error('Invalid PDF byte range')
  const result = new Uint8Array(end - begin)
  let offset = begin
  while (offset < end) {
    const response = await request(url, { headers: { Range: `bytes=${offset}-${Math.min(end, offset + 256 * 1024) - 1}` }, signal })
    const range = /^bytes (\d+)-(\d+)\/(\d+)$/.exec(response.headers.get('Content-Range') || '')
    if (response.status !== 206 || !range || Number(range[1]) !== offset || Number(range[2]) >= end || Number(range[2]) < offset) { await response.body?.cancel(); throw Error('The PDF source could not provide the requested pages') }
    const chunk = new Uint8Array(await response.arrayBuffer())
    if (chunk.byteLength !== Number(range[2]) - offset + 1) throw Error('Incomplete PDF data')
    result.set(chunk, offset - begin)
    offset += chunk.byteLength
  }
  return result
}
