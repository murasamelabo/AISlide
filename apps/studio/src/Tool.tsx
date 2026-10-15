import { useState } from 'react'
import type { ReactNode, SyntheticEvent } from 'react'
import * as Tooltip from '@radix-ui/react-tooltip'

export function Tool({ label, children, onClick, disabled = false, pressed, className = 'tool' }: { label: string; children: ReactNode; onClick: () => void; disabled?: boolean; pressed?: boolean; className?: string }) {
  const [open, setOpen] = useState(false)
  const [container, setContainer] = useState<HTMLElement | null>(null)
  if (disabled && open) setOpen(false)
  function locate(event: SyntheticEvent<HTMLButtonElement>) {
    setContainer(event.currentTarget.closest('dialog') ?? document.body)
  }
  return <Tooltip.Root open={open && !disabled} onOpenChange={(next) => setOpen(next && !disabled)}>
    <Tooltip.Trigger asChild>
      <button type="button" className={className} aria-label={label} aria-pressed={pressed} onClick={onClick} disabled={disabled} onPointerEnter={locate} onFocus={locate}>{children}</button>
    </Tooltip.Trigger>
    <Tooltip.Portal container={container}>
      <Tooltip.Content className="tool-tooltip" side="bottom" sideOffset={8} collisionPadding={8} hideWhenDetached onEscapeKeyDown={(event) => { event.preventDefault(); event.stopPropagation(); setOpen(false) }}>
        {label}
      </Tooltip.Content>
    </Tooltip.Portal>
  </Tooltip.Root>
}