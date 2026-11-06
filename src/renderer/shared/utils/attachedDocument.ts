/**
 * shared/utils/attachedDocument.ts
 *
 * Parses document attachment text and boilerplate injected into `interaction.userText`
 * by the backend (`with_document_text`), allowing the chat interface to separate
 * authentic user prompt text from the attached file contents.
 */

export interface AttachedDocumentPayload {
  filename: string
  content: string
  lineCount: number
  charCount: number
}

export interface ParsedUserMessage {
  userText: string
  attachedDocument?: AttachedDocumentPayload
}

const ATTACHED_DOC_REGEX =
  /(?:^|\n\n)Attached document: ([^\n]+)\nUse the extracted document text below when answering[^\n]*\n\n--- Extracted document text ---\n([\s\S]*?)\n--- End extracted document text ---$/

export function parseAttachedDocument(rawText: string | null | undefined): ParsedUserMessage {
  if (!rawText || typeof rawText !== 'string') {
    return { userText: '' }
  }

  const match = rawText.match(ATTACHED_DOC_REGEX)
  if (!match || match.index === undefined) {
    return { userText: rawText }
  }

  const userText = rawText.slice(0, match.index).trim()
  const filename = match[1].trim()
  const content = match[2]
  const lineCount = content ? content.split('\n').length : 0
  const charCount = content ? content.length : 0

  return {
    userText,
    attachedDocument: {
      filename,
      content,
      lineCount,
      charCount,
    },
  }
}
