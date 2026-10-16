import { useEffect, useRef, useState, type RefObject, type TextareaHTMLAttributes } from 'react'
import { insertPastedText, pastedTextLabel } from '../utils/pastedText'
import '../css/collapsed-paste.css'

type Props = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, 'value'> & {
  value: string
  inputRef: RefObject<HTMLTextAreaElement | null>
  onValueChange: (value: string) => void
  onResizeInput?: (input: HTMLTextAreaElement) => void
}

export function CollapsiblePasteInput({ value, inputRef, onValueChange, onResizeInput, ...props }: Props) {
  // The owner always keeps the complete prompt. Folding changes presentation,
  // so every submit path (including queued messages) receives the real text.
  const [folded, setFolded] = useState<{ value: string; text: string } | null>(null)
  const [expanded, setExpanded] = useState(false)
  const badgeRef = useRef<HTMLButtonElement>(null)
  const collapsed = folded !== null && folded.value === value && !expanded
  useEffect(() => {
    if (folded && folded.value !== value) { setFolded(null); setExpanded(false) }
    if (collapsed) badgeRef.current?.focus()
  }, [value, folded, collapsed])
  const open = () => {
    setExpanded(true)
    setTimeout(() => {
      const input = inputRef.current
      if (!input) return
      input.focus()
      input.setSelectionRange(0, 0)
      onResizeInput?.(input)
    }, 0)
  }
  return <>
    {collapsed && <button
      type="button"
      ref={badgeRef}
      className="chat-pasted-content"
      aria-label={`${pastedTextLabel(folded.text)} — view or edit full prompt`}
      onClick={open}
      onKeyDown={event => {
        if (event.key === 'F2') { event.preventDefault(); open() }
        if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
          event.preventDefault()
          event.currentTarget.form?.requestSubmit()
        }
      }}
    >
      <span>{pastedTextLabel(folded.text)}</span>
      <span className="chat-pasted-content-action">View / edit</span>
    </button>}
    <textarea
      {...props}
      ref={inputRef}
      value={value}
      hidden={collapsed}
      style={{ ...props.style, display: collapsed ? 'none' : props.style?.display }}
      aria-keyshortcuts={folded ? 'F2' : props['aria-keyshortcuts']}
      title={folded ? 'F2: open/collapse pasted content' : props.title}
      onKeyDown={event => {
        if (event.key === 'F2' && folded && folded.value === value && !event.nativeEvent.isComposing) {
          event.preventDefault()
          event.stopPropagation()
          setFolded({ value, text: value })
          setExpanded(false)
        } else props.onKeyDown?.(event)
      }}
      onChange={event => {
        if (folded) setFolded({ value: event.target.value, text: event.target.value })
        props.onChange?.(event)
      }}
      onPaste={event => {
        props.onPaste?.(event)
        if (event.defaultPrevented) return
        const text = event.clipboardData.getData('text/plain')
        if (!text) return
        event.preventDefault()
        event.stopPropagation()
        const input = event.currentTarget
        const draft = insertPastedText(value, input.selectionStart, input.selectionEnd, text)
        setFolded(draft.collapsed ? { value: draft.value, text: draft.text } : folded ? { value: draft.value, text: draft.value } : null)
        setExpanded(!draft.collapsed && folded !== null)
        onValueChange(draft.value)
        if (!draft.collapsed) setTimeout(() => {
          input.setSelectionRange(draft.caret, draft.caret)
          onResizeInput?.(input)
        }, 0)
      }}
    />
  </>
}
