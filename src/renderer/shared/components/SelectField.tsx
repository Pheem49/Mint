import { Children, Fragment, isValidElement, type CSSProperties, type ReactNode } from 'react'
import GlideSelect, { type GlideOption } from './GlideSelect'

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
  return Children.toArray(children).flatMap(child => {
    if (!isValidElement<{ value?: string | number; children?: ReactNode }>(child)) return []
    if (child.type === Fragment) return optionsFromChildren(child.props.children)
    if (child.type !== 'option') return []
    const label = Children.toArray(child.props.children).join('')
    return [{ value: String(child.props.value ?? label), label }]
  })
}

/** Shared field adapter keeps Mint's dynamic option lists and value callbacks intact. */
export default function SelectField({ children, value, defaultValue, onValueChange, disabled,
  id, 'aria-label': ariaLabel, 'aria-labelledby': labelledBy, className, title, style, fullWidth = true }: SelectFieldProps) {
  return <GlideSelect options={optionsFromChildren(children)}
    value={value === undefined ? undefined : String(value)}
    defaultValue={defaultValue === undefined ? undefined : String(defaultValue)}
    onChange={onValueChange} disabled={disabled} id={id} ariaLabel={ariaLabel ?? title ?? 'Select'} labelledBy={labelledBy}
    className={className ? `mint-select-field ${className}` : 'mint-select-field'} title={title}
    style={style} fullWidth={fullWidth} />
}
