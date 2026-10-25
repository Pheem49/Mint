import {
  Children,
  Fragment,
  isValidElement,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from 'react'
import { createPortal } from 'react-dom'
import { HugeiconsIcon } from '@hugeicons/react'
import { ArrowDown01Icon, Tick02Icon } from '@hugeicons/core-free-icons'
import './GlideSelect.css'

export interface GlideOption {
  value: string
  label: ReactNode
  tag?: string
}

export interface GlideSelectProps {
  options?: (string | GlideOption)[]
  value?: string
  defaultValue?: string
  onChange?: (value: string, option: GlideOption) => void
  placeholder?: string
  showTags?: boolean
  accentColor?: string
  surfaceColor?: string
  highlightColor?: string
  textColor?: string
  size?: 'sm' | 'md' | 'lg'
  radius?: number
  menuWidth?: number
  placement?: 'top' | 'bottom'
  align?: 'left' | 'right'
  popDuration?: number
  glideDuration?: number
  rememberPosition?: boolean
  disabled?: boolean
  ariaLabel?: string
  className?: string
  id?: string
  labelledBy?: string
  title?: string
  style?: CSSProperties
  fullWidth?: boolean
}

const SIZES = {
  sm: { chip: 28, row: 26, font: 12 },
  md: { chip: 32, row: 30, font: 13 },
  lg: { chip: 44, row: 40, font: 14 },
} as const
const GAP = 1
const MENU_GAP = 6
const DEFAULT_OPTIONS: (string | GlideOption)[] = ['One', 'Two', 'Three']
const norm = (o: string | GlideOption): GlideOption => (typeof o === 'string' ? { value: o, label: o } : o)
const textOf = (it: GlideOption): string => (typeof it.label === 'string' ? it.label : it.value)
const typeaheadIndex = (items: GlideOption[], from: number, ch: string): number => {
  const c = ch.toLowerCase()
  for (let k = 1; k <= items.length; k++) {
    const i = (from + k) % items.length
    if (textOf(items[i]).toLowerCase().startsWith(c)) return i
  }
  return from
}

export function GlideSelect({
  options = DEFAULT_OPTIONS,
  value,
  defaultValue,
  onChange,
  placeholder = 'Select…',
  showTags = true,
  accentColor = 'var(--interactive-fg)',
  surfaceColor = 'var(--bg-surface-raised)',
  highlightColor = 'var(--bg-surface-hover)',
  textColor = 'var(--text-primary)',
  size = 'md',
  radius = 10,
  menuWidth = 176,
  placement = 'bottom',
  align = 'left',
  popDuration = 180,
  glideDuration = 220,
  rememberPosition = true,
  disabled = false,
  ariaLabel = 'Select',
  className = '',
  id: triggerId,
  labelledBy,
  title,
  style,
  fullWidth = false,
}: GlideSelectProps) {
  const items = options.map(norm)
  const [inner, setInner] = useState(defaultValue ?? '')
  const current = value ?? inner
  const selected = items.findIndex((it) => it.value === current)
  const [phase, setPhase] = useState<'closed' | 'open' | 'closing'>('closed')
  const [active, setActive] = useState<number | null>(null)
  const [side, setSide] = useState<'top' | 'bottom'>(placement)
  const [position, setPosition] = useState<{
    top: number
    left: number
    width: number
    maxHeight: number
    fontFamily?: string
  }>({ top: 0, left: 0, width: menuWidth, maxHeight: 300 })
  const rootRef = useRef<HTMLDivElement | null>(null)
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const menuRef = useRef<HTMLDivElement | null>(null)
  const listRef = useRef<HTMLDivElement | null>(null)
  const pillRef = useRef<HTMLSpanElement | null>(null)
  const instant = useRef(false)
  const closeTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const scrub = useRef<{ id: number } | null>(null)
  const id = useId()
  const S = SIZES[size] ?? SIZES.md
  const step = S.row + GAP
  const popOut = Math.round((popDuration * 2) / 3)
  const unavailable = disabled || items.length === 0

  useLayoutEffect(() => {
    if (phase !== 'open') return
    const el = menuRef.current
    const trigger = triggerRef.current
    if (!el || !trigger) return
    const place = () => {
      const r = trigger.getBoundingClientRect()
      const below = Math.max(0, window.innerHeight - r.bottom - MENU_GAP - 8)
      const above = Math.max(0, r.top - MENU_GAP - 8)
      const desired = Math.min(300, items.length * step + 7)
      const nextSide: 'top' | 'bottom' =
        placement === 'bottom'
          ? below < desired && above > below
            ? 'top'
            : 'bottom'
          : above < desired && below > above
            ? 'bottom'
            : 'top'
      const maxHeight = Math.min(300, nextSide === 'bottom' ? below : above)
      const height = Math.min(desired, maxHeight)
      const width = Math.min(Math.max(menuWidth, r.width), window.innerWidth - 16)
      const left = Math.max(8, Math.min(align === 'right' ? r.right - width : r.left, window.innerWidth - width - 8))
      setSide(nextSide)
      setPosition({
        fontFamily: getComputedStyle(trigger).fontFamily,
        top: nextSide === 'bottom' ? r.bottom + MENU_GAP : r.top - MENU_GAP - height,
        left,
        width,
        maxHeight,
      })
    }
    place()
    el.dataset.state = 'closed'
    void el.offsetHeight
    el.dataset.state = 'open'
    const p = pillRef.current
    if (p) {
      p.style.transition = 'none'
      p.style.transform = `translateY(${Math.max(0, selected) * step}px)`
      p.style.opacity = '0'
      void p.offsetHeight
      p.style.transition = ''
    }
    const onScroll = (e: Event) => {
      if (!menuRef.current?.contains(e.target as Node)) place()
    }
    window.addEventListener('resize', place)
    window.addEventListener('scroll', onScroll, true)
    return () => {
      window.removeEventListener('resize', place)
      window.removeEventListener('scroll', onScroll, true)
    }
  }, [phase, placement, align, menuWidth, items.length, step, selected])

  useLayoutEffect(() => {
    const p = pillRef.current
    if (!p || phase !== 'open') return
    if (active === null) {
      p.style.opacity = '0'
      return
    }
    const jump = instant.current || p.style.opacity !== '1'
    p.style.transitionDuration = jump ? '0ms, 150ms' : ''
    p.style.transform = `translateY(${active * step}px)`
    p.style.opacity = '1'
    instant.current = false
    const list = listRef.current
    if (list) {
      const top = active * step
      if (top < list.scrollTop) list.scrollTop = top
      else if (top + S.row > list.scrollTop + list.clientHeight) list.scrollTop = top + S.row - list.clientHeight
    }
  }, [active, phase, step, S.row, position.maxHeight])

  const open = (viaKey: boolean) => {
    if (unavailable) return
    clearTimeout(closeTimer.current)
    instant.current = true
    setActive(selected >= 0 ? selected : viaKey ? 0 : null)
    setPhase('open')
  }
  const close = (mode: 'instant' | 'pop') => {
    setActive(null)
    scrub.current = null
    clearTimeout(closeTimer.current)
    const el = menuRef.current
    if (mode === 'instant' || !el) {
      setPhase('closed')
      return
    }
    el.style.transitionDuration = ''
    el.dataset.state = 'closed'
    setPhase('closing')
    closeTimer.current = setTimeout(() => setPhase('closed'), popOut + 20)
  }
  const pick = (i: number, viaKey: boolean) => {
    const it = items[i]
    if (!it) {
      close('instant')
      return
    }
    if (it.value !== current) {
      if (value === undefined) setInner(it.value)
      onChange?.(it.value, it)
      if (!viaKey && rootRef.current) rootRef.current.dataset.swap = ''
    }
    close('instant')
    triggerRef.current?.focus({ preventScroll: true })
  }
  const onTriggerKey = (e: ReactKeyboardEvent<HTMLButtonElement>) => {
    const k = e.key
    const n = items.length
    const cur = active ?? Math.max(0, selected)
    if (phase !== 'open') {
      if (['Enter', ' ', 'ArrowDown', 'ArrowUp'].includes(k)) {
        e.preventDefault()
        open(true)
      }
      return
    }
    const go = (i: number) => {
      e.preventDefault()
      instant.current = true
      setActive(Math.min(n - 1, Math.max(0, i)))
    }
    if (k === 'ArrowDown' || k === 'ArrowUp') go(active === null ? cur : cur + (k === 'ArrowDown' ? 1 : -1))
    else if (k === 'Home' || k === 'End') go(k === 'Home' ? 0 : n - 1)
    else if (k === 'Enter' || k === ' ') {
      e.preventDefault()
      pick(cur, true)
    } else if (k === 'Escape' || k === 'Tab') {
      if (k === 'Escape') {
        e.preventDefault()
        e.stopPropagation()
      }
      close('instant')
    } else if (k.length === 1 && !e.metaKey && !e.ctrlKey && !e.altKey) go(typeaheadIndex(items, cur, k))
  }
  useEffect(() => {
    if (phase === 'closed') return
    const onDown = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node) && !menuRef.current?.contains(e.target as Node)) close('pop')
    }
    const onFocus = (e: FocusEvent) => {
      if (!rootRef.current?.contains(e.target as Node) && !menuRef.current?.contains(e.target as Node)) close('instant')
    }
    document.addEventListener('pointerdown', onDown, true)
    document.addEventListener('focusin', onFocus)
    return () => {
      document.removeEventListener('pointerdown', onDown, true)
      document.removeEventListener('focusin', onFocus)
    }
  }, [phase])
  useEffect(() => {
    if (unavailable && phase !== 'closed') close('instant')
  }, [unavailable, phase])
  useEffect(() => () => clearTimeout(closeTimer.current), [])

  const rowAt = (y: number, x: number): number | null => {
    const list = listRef.current
    if (!scrub.current || !list) return null
    const r = list.getBoundingClientRect()
    if (y < r.top || y >= r.bottom || x < r.left || x >= r.right) return null
    const offset = y - r.top + list.scrollTop
    const i = Math.floor(offset / step)
    return i >= 0 && i < items.length && offset % step < S.row ? i : null
  }
  const onListDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (scrub.current || e.button !== 0) return
    if (e.pointerType === 'touch') return
    e.preventDefault()
    try {
      e.currentTarget.setPointerCapture(e.pointerId)
    } catch {}
    scrub.current = { id: e.pointerId }
    instant.current = true
    setActive(rowAt(e.clientY, e.clientX))
  }
  const onListMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!scrub.current || scrub.current.id !== e.pointerId) return
    const i = rowAt(e.clientY, e.clientX)
    if (i !== active) setActive(i)
  }
  const onListUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!scrub.current || scrub.current.id !== e.pointerId) return
    const i = e.type === 'pointerup' ? rowAt(e.clientY, e.clientX) : null
    scrub.current = null
    if (i !== null) pick(i, false)
    else if (!rememberPosition) setActive(null)
  }
  const onListOver = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.pointerType === 'touch' || scrub.current) return
    const row = (e.target as HTMLElement).closest('[data-index]') as HTMLElement | null
    if (!row) return
    const i = Number(row.dataset.index)
    if (i !== active) setActive(i)
  }
  const variables: Record<string, string> = {
    '--gs-accent': accentColor,
    '--gs-surface': surfaceColor,
    '--gs-highlight': highlightColor,
    '--gs-text': textColor,
    '--gs-radius': `${radius}px`,
    '--gs-inner-radius': `${Math.max(3, radius - 4)}px`,
    '--gs-chip': `${S.chip}px`,
    '--gs-row': `${S.row}px`,
    '--gs-font': `${S.font}px`,
    '--gs-menu-w': `${menuWidth}px`,
    '--gs-pop': `${popDuration}ms`,
    '--gs-pop-out': `${popOut}ms`,
    '--gs-glide': `${glideDuration}ms`,
    '--gs-origin': `${side === 'bottom' ? 'top' : 'bottom'} ${align}`,
  }
  return (
    <div
      ref={rootRef}
      className={`glide-select${className ? ` ${className}` : ''}`}
      data-size={size}
      data-disabled={unavailable ? '' : undefined}
      data-full-width={fullWidth ? '' : undefined}
      style={{ ...(variables as CSSProperties), ...style }}
      onAnimationEnd={(e) => {
        if (e.animationName === 'gs-swap' && rootRef.current) delete rootRef.current.dataset.swap
      }}
    >
      <button
        ref={triggerRef}
        id={triggerId}
        title={title}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={phase === 'open'}
        aria-controls={phase !== 'closed' ? `${id}-list` : undefined}
        aria-activedescendant={phase === 'open' && active !== null ? `${id}-${active}` : undefined}
        aria-label={labelledBy ? undefined : ariaLabel}
        aria-labelledby={labelledBy}
        disabled={unavailable}
        className="glide-select__trigger"
        onPointerDown={(e) => {
          if (e.button !== 0 || unavailable) return
          e.currentTarget.focus({ preventScroll: true })
          if (phase === 'open') close('pop')
          else open(false)
        }}
        onClick={(e) => {
          if (e.detail === 0 && phase === 'closed') open(true)
        }}
        onKeyDown={onTriggerKey}
      >
        <span className="glide-select__label" key={current} data-empty={selected < 0 ? '' : undefined}>
          {selected >= 0 ? items[selected].label : placeholder}
        </span>
        <span className="glide-select__chevron" aria-hidden="true">
          <HugeiconsIcon icon={ArrowDown01Icon} size={12} strokeWidth={2.5} />
        </span>
      </button>
      {phase !== 'closed' &&
        createPortal(
          <div
            ref={menuRef}
            className="glide-select__menu"
            data-state="open"
            data-side={side}
            data-align={align}
            style={{
              ...(variables as CSSProperties),
              top: position.top,
              left: position.left,
              width: position.width,
              maxHeight: position.maxHeight,
              fontFamily: position.fontFamily,
            }}
          >
            <div
              ref={listRef}
              id={`${id}-list`}
              role="listbox"
              aria-label={ariaLabel}
              className="glide-select__list"
              style={{ maxHeight: Math.max(0, position.maxHeight - 8) }}
              data-live={active !== null ? '' : undefined}
              onPointerOver={onListOver}
              onPointerLeave={() => {
                if (!scrub.current && !rememberPosition) setActive(null)
              }}
              onPointerDown={onListDown}
              onPointerMove={onListMove}
              onPointerUp={onListUp}
              onPointerCancel={onListUp}
              onLostPointerCapture={onListUp}
            >
              <span ref={pillRef} className="glide-select__pill" aria-hidden="true" />
              {items.map((it, i) => (
                <div
                  key={it.value}
                  id={`${id}-${i}`}
                  role="option"
                  aria-selected={i === selected}
                  data-index={i}
                  className="glide-select__option"
                  onClick={(e) => {
                    if (e.detail === 0 || (e.nativeEvent as PointerEvent).pointerType === 'touch') pick(i, false)
                  }}
                >
                  <span className="glide-select__name">{it.label}</span>
                  {showTags && it.tag ? <span className="glide-select__tag">{it.tag}</span> : null}
                  <span className="glide-select__check" data-on={i === selected ? '' : undefined} aria-hidden="true">
                    <HugeiconsIcon icon={Tick02Icon} size={13} strokeWidth={2.5} />
                  </span>
                </div>
              ))}
            </div>
          </div>,
          document.body,
        )}
    </div>
  )
}

interface SelectFieldProps {
  children: ReactNode
  value?: string | number
  defaultValue?: string | number
  onValueChange?: (value: string) => void
  disabled?: boolean
  id?: string
  'aria-label'?: string
  'aria-labelledby'?: string
  className?: string
  title?: string
  style?: CSSProperties
  fullWidth?: boolean
}

function optionsFromChildren(children: ReactNode): GlideOption[] {
  return Children.toArray(children).flatMap((child) => {
    if (!isValidElement<{ value?: string | number; children?: ReactNode }>(child)) return []
    if (child.type === Fragment) return optionsFromChildren(child.props.children)
    if (child.type !== 'option') return []
    const label = Children.toArray(child.props.children).join('')
    return [{ value: String(child.props.value ?? label), label }]
  })
}

/** Shared field module keeps Mint's dynamic option lists and value callbacks behind a native-like <SelectField> interface. */
export default function SelectField({
  children,
  value,
  defaultValue,
  onValueChange,
  disabled,
  id,
  'aria-label': ariaLabel,
  'aria-labelledby': labelledBy,
  className,
  title,
  style,
  fullWidth = true,
}: SelectFieldProps) {
  return (
    <GlideSelect
      options={optionsFromChildren(children)}
      value={value === undefined ? undefined : String(value)}
      defaultValue={defaultValue === undefined ? undefined : String(defaultValue)}
      onChange={onValueChange}
      disabled={disabled}
      id={id}
      ariaLabel={ariaLabel ?? title ?? 'Select'}
      labelledBy={labelledBy}
      className={className ? `mint-select-field ${className}` : 'mint-select-field'}
      title={title}
      style={style}
      fullWidth={fullWidth}
    />
  )
}
