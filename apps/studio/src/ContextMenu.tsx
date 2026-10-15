import { useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import type { LucideIcon } from 'lucide-react'

export type MenuCommand = { label: string; icon?: LucideIcon; action: () => void; disabled?: boolean; separator?: boolean; danger?: boolean }
export type MenuPosition = { x: number; y: number; anchor?: HTMLElement }

export function ContextMenu({ position, label, commands, onClose }: { position: MenuPosition; label: string; commands: MenuCommand[]; onClose: () => void }) {
  const menu = useRef<HTMLDivElement>(null)
  const [point, setPoint] = useState({ x: position.x, y: position.y })
  useLayoutEffect(() => {
    const element = menu.current
    if (!element) return
    const bounds = element.getBoundingClientRect()
    setPoint({ x: Math.max(8, Math.min(position.x, window.innerWidth - bounds.width - 8)), y: Math.max(8, Math.min(position.y, window.innerHeight - bounds.height - 8)) })
    element.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
    const dismiss = (event: PointerEvent) => { if (!element.contains(event.target as Node)) onClose() }
    const dismissResize = () => onClose()
    document.addEventListener('pointerdown', dismiss, true)
    window.addEventListener('resize', dismissResize)
    return () => { document.removeEventListener('pointerdown', dismiss, true); window.removeEventListener('resize', dismissResize); if (position.anchor?.isConnected) position.anchor.focus() }
  }, [position, onClose])
  return createPortal(<div ref={menu} className="context-menu" role="menu" aria-label={label} style={{ left: point.x, top: point.y }} onContextMenu={(event) => event.preventDefault()} onKeyDown={(event) => {
    if (event.key === 'Escape' || event.key === 'Tab') { event.preventDefault(); event.stopPropagation(); onClose(); return }
    const buttons = [...(menu.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])]
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement)
    const next = event.key === 'ArrowDown' ? (index + 1) % buttons.length : event.key === 'ArrowUp' ? (index - 1 + buttons.length) % buttons.length : event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : -1
    if (next >= 0) { event.preventDefault(); event.stopPropagation(); buttons[next]?.focus() }
  }}>{commands.map(({ label, icon: Icon, action, disabled, separator, danger }) => <div key={label} role="none">{separator && <div role="separator" className="menu-separator" />}<button type="button" role="menuitem" tabIndex={-1} disabled={disabled} className={danger ? 'danger' : ''} onClick={() => { onClose(); action() }}>{Icon && <Icon size={18} aria-hidden="true" />}<span>{label}</span></button></div>)}</div>, position.anchor?.closest('dialog') ?? document.body)
}