/** Parse JSON-backed chat cards, repairing one narrowly-recognized model error:
 * a simple property (commonly `badge`) emitted just outside its object. */
export function parseCardJsonSafely(input: string): any {
  if (!input || !input.trim()) return null

  const cleaned = input.trim()
  const parse = (text: string): any => {
    try {
      return JSON.parse(text)
    } catch {
      try {
        return JSON.parse(text.replace(/,\s*([\]}])/g, '$1'))
      } catch {
        return null
      }
    }
  }

  const parsed = parse(cleaned)
  if (parsed !== null) return parsed

  // Example: [{"title":"Plan"}, "badge":"Recommended"}]
  // Move only scalar fields back into the preceding object; avoid guessing
  // about malformed nested objects, arrays, or unrelated syntax errors.
  const repaired = cleaned.replace(
    /}\s*,\s*("(?:\\.|[^"\\])*"\s*:\s*(?:"(?:\\.|[^"\\])*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null))(?=\s*[,}])/g,
    ', $1',
  )
  return repaired === cleaned ? null : parse(repaired)
}
