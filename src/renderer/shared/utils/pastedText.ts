export function insertPastedText(value: string, start: number, end: number, text: string) {
  const normalized = text.replace(/\r\n?/g, '\n')
  return {
    value: value.slice(0, start) + normalized + value.slice(end),
    text: normalized,
    caret: start + normalized.length,
    collapsed: normalized.replace(/\n$/, '').split('\n').length > 6,
  }
}

export function pastedTextLabel(text: string) {
  return `[Pasted Content ${Array.from(text).length} chars]`
}
