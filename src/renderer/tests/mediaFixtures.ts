/** Small valid documents/media for the real browser decoders; no runtime mocks for rendering. */
export function samplePdf(): Uint8Array<ArrayBuffer> {
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << /Font << /F1 7 0 R >> >> /Contents 4 0 R >>',
    '',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>',
    '',
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
  ]
  for (const [index, label] of [[3, 'Media PDF page one'], [5, 'Media PDF page two']] as const) {
    const content = `BT /F1 18 Tf 35 300 Td (${label}) Tj ET`
    objects[index] = `<< /Length ${content.length} >>\nstream\n${content}\nendstream`
  }
  let data = '%PDF-1.4\n'
  const offsets = [0]
  objects.forEach((object, index) => { offsets.push(data.length); data += `${index + 1} 0 obj\n${object}\nendobj\n` })
  const crossref = data.length
  data += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`
  data += offsets.slice(1).map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')
  data += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${crossref}\n%%EOF`
  return new TextEncoder().encode(data)
}
export function sampleWav(): ArrayBuffer {
  const samples = 8000, buffer = new ArrayBuffer(44 + samples * 2), view = new DataView(buffer)
  const text = (offset: number, value: string) => [...value].forEach((letter, index) => view.setUint8(offset + index, letter.charCodeAt(0)))
  text(0, 'RIFF'); view.setUint32(4, buffer.byteLength - 8, true); text(8, 'WAVE'); text(12, 'fmt ')
  view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true); view.setUint32(24, 8000, true); view.setUint32(28, 16000, true); view.setUint16(32, 2, true); view.setUint16(34, 16, true); text(36, 'data'); view.setUint32(40, samples * 2, true)
  for (let i = 0; i < samples; i++) view.setInt16(44 + i * 2, Math.sin(i * Math.PI * 2 * 220 / 8000) * 3000, true)
  return buffer
}
