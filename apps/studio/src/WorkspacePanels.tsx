import { useEffect, useEffectEvent, useRef, useState } from 'react'
import type { PointerEvent as ReactPointerEvent } from 'react'

const clamp = (value: number, minimum: number, maximum: number) => Math.round(Math.min(maximum, Math.max(minimum, value)))

type ResizeProps = {
  label: string; controls: string; orientation: 'vertical' | 'horizontal'; direction?: 1 | -1
  value: number; preferredValue: number; minimum: number; maximum: number; defaultValue: number; disabled?: boolean
  onChange: (value: number, commit: boolean) => void
}

export function PaneResizeHandle({ label, controls, orientation, direction = 1, value, preferredValue, minimum, maximum, defaultValue, disabled = false, onChange }: ResizeProps) {
  const handle = useRef<HTMLDivElement>(null)
  const gesture = useRef<{ pointerId: number; start: number; value: number; restore: number } | null>(null)
  const [resizing, setResizing] = useState(false)
  function release() {
    const active = gesture.current
    gesture.current = null
    setResizing(false)
    if (active && handle.current?.hasPointerCapture(active.pointerId)) handle.current.releasePointerCapture(active.pointerId)
  }
  function cancel() {
    const active = gesture.current
    if (!active) return
    release()
    onChange(active.restore, false)
  }
  const cancelOnBlur = useEffectEvent(cancel)
  useEffect(() => {
    const blur = () => cancelOnBlur()
    window.addEventListener('blur', blur)
    return () => window.removeEventListener('blur', blur)
  }, [])
  useEffect(() => { if (disabled) cancelOnBlur() }, [disabled])
  function position(event: ReactPointerEvent) { return orientation === 'vertical' ? event.clientX : event.clientY }
  function move(event: ReactPointerEvent, commit: boolean) {
    const active = gesture.current
    if (!active || active.pointerId !== event.pointerId) return
    event.preventDefault()
    event.stopPropagation()
    const next = clamp(active.value + (position(event) - active.start) * direction, minimum, maximum)
    if (commit) release()
    onChange(next, commit)
  }
  return <div ref={handle} className="pane-resize-handle" role="separator" tabIndex={disabled ? -1 : 0}
    aria-label={label} aria-controls={controls} aria-orientation={orientation} aria-valuemin={minimum} aria-valuemax={maximum} aria-valuenow={value} aria-valuetext={`${value} pixels`} aria-disabled={disabled || undefined}
    data-resizing={resizing || undefined}
    onPointerDown={event => {
      if (disabled || event.button !== 0 || !event.isPrimary || gesture.current) return
      event.preventDefault(); event.stopPropagation()
      event.currentTarget.focus({ preventScroll: true })
      event.currentTarget.setPointerCapture(event.pointerId)
      gesture.current = { pointerId: event.pointerId, start: position(event), value, restore: preferredValue }
      setResizing(true)
    }}
    onPointerMove={event => move(event, false)} onPointerUp={event => move(event, true)}
    onPointerCancel={cancel} onLostPointerCapture={cancel} onBlur={cancel}
    onDoubleClick={() => { if (!disabled) { cancel(); onChange(clamp(defaultValue, minimum, maximum), true) } }}
    onContextMenu={event => { event.preventDefault(); event.stopPropagation() }}
    onKeyDown={event => {
      if (event.key === 'Tab') { cancel(); return }
      event.stopPropagation()
      if (disabled) return
      const decrease = orientation === 'vertical' ? 'ArrowLeft' : 'ArrowUp'
      const increase = orientation === 'vertical' ? 'ArrowRight' : 'ArrowDown'
      if (!['Escape', 'Home', 'End', 'Enter', decrease, increase].includes(event.key)) return
      event.preventDefault()
      if (event.key === 'Escape') { cancel(); return }
      if (gesture.current) return
      const next = event.key === 'Home' ? minimum : event.key === 'End' ? maximum : event.key === 'Enter' ? defaultValue : value + (event.key === increase ? 1 : -1) * direction * (event.shiftKey ? 32 : 8)
      onChange(clamp(next, minimum, maximum), true)
    }} />
}