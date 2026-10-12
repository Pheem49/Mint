import { useEffect, useRef, useState, type RefObject } from 'react'
import type { SlashChoice } from '../platform'
import '../css/slash-choice.css'

interface Props {
  choice: { command: string; title: string; options: SlashChoice[] }
  inputRef: RefObject<HTMLTextAreaElement | null>
  onSelect?: (value: string) => void
  onDismiss?: () => void
}

export default function SlashChoicePanel({ choice, inputRef, onSelect, onDismiss }: Props) {
  const [selectedIndex, setSelectedIndex] = useState(0)
  const panelRef = useRef<HTMLElement>(null)
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([])
  const selected = Math.min(selectedIndex, Math.max(0, choice.options.length - 1))

  useEffect(() => {
    setSelectedIndex(0)
    inputRef.current?.focus()
  }, [choice, inputRef])

  useEffect(() => {
    optionRefs.current[selected]?.scrollIntoView({ block: 'nearest' })
  }, [selected, choice])

  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.isComposing || event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return
      const target = event.target as Node | null
      const onInput = target === inputRef.current
      const inPanel = target !== null && panelRef.current?.contains(target)
      if (event.key !== 'Escape' && !onInput && !inPanel) return
      // Keep native Enter activation for the dismiss button.
      if (event.key === 'Enter' && !onInput && !optionRefs.current.includes(target as HTMLButtonElement)) return
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        onDismiss?.()
        inputRef.current?.focus()
        return
      }
      if (!choice.options.length) return
      if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
        event.preventDefault()
        event.stopPropagation()
        const next = (selected + (event.key === 'ArrowDown' ? 1 : -1) + choice.options.length) % choice.options.length
        setSelectedIndex(next)
        if (inPanel) optionRefs.current[next]?.focus()
      } else if (event.key === 'Enter') {
        event.preventDefault()
        event.stopPropagation()
        onSelect?.(choice.options[selected].value)
        inputRef.current?.focus()
      }
    }
    // Capture before the composer's history/submit handlers claim these keys.
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [choice, inputRef, onDismiss, onSelect, selected])

  return (
    <section ref={panelRef} className="slash-choice-panel" aria-label={`${choice.command} choices`}>
      <header className="slash-choice-heading">
        <div><span className="slash-choice-command">{choice.command}</span><p>{choice.title}</p></div>
        <button type="button" onClick={onDismiss} aria-label="Dismiss choices">×</button>
      </header>
      <div className="slash-choice-options" role="group" aria-label="Options">
        {choice.options.map((option, index) => (
          <button
            key={option.value}
            ref={(element) => { optionRefs.current[index] = element }}
            type="button"
            aria-pressed={index === selected}
            className={index === selected ? 'active' : undefined}
            onPointerMove={() => setSelectedIndex(index)}
            onFocus={() => setSelectedIndex(index)}
            onClick={() => { onSelect?.(option.value); inputRef.current?.focus() }}
          >
            <span>{index + 1}.</span>{option.label}
          </button>
        ))}
      </div>
      <span className="slash-choice-status" role="status">{choice.options[selected]?.label}</span>
      <div className="slash-choice-hint">↑↓ select · Enter choose · Esc dismiss</div>
    </section>
  )
}
