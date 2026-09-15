import { useEffect, useRef, useState } from 'react'
import { Check, X, Rows3, Columns3, Minus } from 'lucide-react'
import type { Element, Theme } from './types'
import { textStyle } from './design'

export function InlineEditor({ element, theme, onCommit, onCancel, onDraftChange }: { element: Element; theme?: Theme; onCommit: (element: Element) => Promise<void>; onCancel: () => void; onDraftChange?: (dirty: boolean) => void }) {
  const [draft, setDraft] = useState<Element>(structuredClone(element))
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const form = useRef<HTMLFormElement>(null)
  const composing = useRef(false)
  const pending = useRef(false)
  const cancelled = useRef(false)
  const hasDraft = JSON.stringify(draft) !== JSON.stringify(element)
  useEffect(() => { onDraftChange?.(hasDraft); return () => onDraftChange?.(false) }, [hasDraft, onDraftChange])
  useEffect(() => {
    const input = form.current?.querySelector<HTMLTextAreaElement | HTMLInputElement>('textarea, input')
    input?.focus()
  }, [])
  async function commit() {
    if (pending.current || composing.current || cancelled.current) return
    pending.current = true; setBusy(true); setError('')
    try { await onCommit(draft); onCancel() }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { pending.current = false; setBusy(false) }
  }
  return <form ref={form} className="inline-editor" style={draft.type === 'group' ? { background: 'transparent' } : undefined} onSubmit={(event) => { event.preventDefault(); void commit() }}
    onPointerDown={(event) => event.stopPropagation()} onDoubleClick={(event) => event.stopPropagation()}
    onCompositionStart={() => { composing.current = true }} onCompositionEnd={() => { composing.current = false }}
    onKeyDown={(event) => {
      event.stopPropagation()
      if (event.nativeEvent.isComposing || composing.current || event.keyCode === 229) return
      if (event.key === 'Escape') { event.preventDefault(); if (!pending.current) { cancelled.current = true; onCancel() } }
      if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) { event.preventDefault(); void commit() }
    }}>
    {(draft.type === 'text' || draft.type === 'shape') && <textarea aria-label="Slide text editor" className="inline-text" value={draft.text} maxLength={4000} disabled={busy}
      style={textStyle(draft, theme)}
      onChange={(event) => setDraft({ ...draft, text: event.target.value })} />}
    {draft.type === 'group' && <div style={{ width: draft.view_width, height: draft.view_height, transformOrigin: 'top left', transform: `scale(${draft.width / draft.view_width}, ${draft.height / draft.view_height})` }}>{draft.children.map((child) => (child.type === 'text' || child.type === 'shape') && <textarea key={child.id} aria-label={`Group text ${child.id}`} className="inline-text" value={child.text} disabled={busy} maxLength={4000} style={{ ...textStyle(child, theme), position: 'absolute', left: child.x, top: child.y, width: child.width, height: child.height }} onChange={(event) => setDraft({ ...draft, children: draft.children.map((entry) => entry.id === child.id ? { ...child, text: event.target.value } : entry) })} />)}</div>}
    {draft.type === 'table' && <table className="slide-table inline-table" style={{ fontSize: draft.font_size }}><tbody>
      {draft.rows.map((row, rowIndex) => <tr key={rowIndex} style={{ height: `${100 / draft.rows.length}%` }}>{row.map((value, column) => <td key={column}>
        <textarea aria-label={`Table row ${rowIndex + 1} column ${column + 1}`} value={value} maxLength={200} rows={1} disabled={busy}
          onChange={(event) => setDraft({ ...draft, rows: draft.rows.map((entry, index) => index === rowIndex ? entry.map((cell, position) => position === column ? event.target.value : cell) : entry) })} />
      </td>)}</tr>)}
    </tbody></table>}
    <div className="inline-edit-actions" style={element.y + element.height > 660 ? { bottom: 'auto', top: -46 } : undefined}>
      {draft.type === 'table' && <>
        <button type="button" aria-label="Add table row" title="Add row" disabled={busy || draft.rows.length >= 12} onClick={() => setDraft({ ...draft, rows: [...draft.rows, draft.rows[0].map(() => '')] })}><Rows3 size={20} /></button>
        <button type="button" aria-label="Remove last table row" title="Remove last row" disabled={busy || draft.rows.length <= 1} onClick={() => setDraft({ ...draft, rows: draft.rows.slice(0, -1) })}><Minus size={20} /></button>
        <button type="button" aria-label="Add table column" title="Add column" disabled={busy || draft.rows[0].length >= 8} onClick={() => setDraft({ ...draft, rows: draft.rows.map((row) => [...row, '']) })}><Columns3 size={20} /></button>
        <button type="button" aria-label="Remove last table column" title="Remove last column" disabled={busy || draft.rows[0].length <= 1} onClick={() => setDraft({ ...draft, rows: draft.rows.map((row) => row.slice(0, -1)) })}><Minus size={20} /></button>
      </>}
      <button type="submit" aria-label="Apply on-slide edit" title="Apply edit" disabled={busy}><Check size={20} /></button>
      <button type="button" aria-label="Cancel on-slide edit" title="Cancel edit" disabled={busy} onClick={() => { cancelled.current = true; onCancel() }}><X size={20} /></button>
    </div>
    {error && <div className="inline-edit-error" role="alert">{error}</div>}
  </form>
}