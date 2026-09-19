import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Check, X, Rows3, Columns3, Minus } from 'lucide-react'
import type { Element, Theme } from './types'
import { textStyle } from './design'
import { RunControls, selectedRunStyle, unicodeRange } from './TextControls'
import type { TextDraftAdapters, TextElement } from './TextControls'
import type { RunStyle } from './types'
import { core } from './api'

export type InlineEditorProps = TextDraftAdapters & { element: Element; theme?: Theme; onCommit: (element: Element) => Promise<void>; onCancel: () => void; onDraftChange?: (dirty: boolean) => void; onBusy?: (busy: boolean) => void; disabled?: boolean; spellCheck?: boolean }

export function InlineEditor(props: InlineEditorProps) {
  return <InlineEditorDraft key={props.element.id} {...props} />
}

function InlineEditorDraft({ element, theme, onCommit, onCancel, onDraftChange, onBusy, disabled = false, spellCheck = false,
  onFormatRange = (element, start, end, style) => core<Element>({ op: 'format_text_element', element, start, end, style }),
  onReplaceText = (element, text) => core<Element>({ op: 'replace_element_text', element, text }),
}: InlineEditorProps) {
  const [draft, setDraft] = useState<Element>(structuredClone(element))
  const [texts, setTexts] = useState<Record<string, string>>({})
  const [cellTexts, setCellTexts] = useState<Record<string, string>>({})
  const [selection, setSelection] = useState<{ id: string; start: number; end: number } | null>(null)
  const [error, setError] = useState('')
  const [working, setBusy] = useState(false)
  const [toolbar, setToolbar] = useState<{ left: number; top: number; container: HTMLElement } | null>(null)
  const busy = working || disabled
  const form = useRef<HTMLFormElement>(null)
  const composing = useRef(false)
  const pending = useRef(false)
  const cancelled = useRef(false)
  const hasDraft = JSON.stringify(draft) !== JSON.stringify(element) || Object.keys(texts).length > 0 || Object.keys(cellTexts).length > 0
  useEffect(() => { onDraftChange?.(hasDraft); return () => onDraftChange?.(false) }, [hasDraft, onDraftChange])
  useEffect(() => {
    cancelled.current = false
    const input = form.current?.querySelector<HTMLTextAreaElement | HTMLInputElement>('textarea, input')
    input?.focus()
    return () => { cancelled.current = true }
  }, [])
  useEffect(() => {
    const anchor = form.current
    if (!anchor) return
    function position() {
      const bounds = anchor!.getBoundingClientRect()
      const actions = anchor!.querySelector('.inline-edit-actions')?.getBoundingClientRect()
      const width = Math.min(340, window.innerWidth - 16)
      const height = Math.min(280, window.innerHeight - 16)
      const left = Math.max(8, Math.min(bounds.left, window.innerWidth - width - 8))
      const bottom = Math.max(bounds.bottom, actions?.bottom ?? bounds.bottom)
      const above = Math.min(bounds.top, actions?.top ?? bounds.top)
      const top = bottom + height + 8 <= window.innerHeight ? bottom + 8 : Math.max(8, above - height - 8)
      setToolbar({ left, top, container: anchor!.closest('dialog') ?? document.body })
    }
    position()
    const observer = new ResizeObserver(position)
    observer.observe(anchor)
    window.addEventListener('resize', position)
    window.addEventListener('scroll', position, true)
    return () => { observer.disconnect(); window.removeEventListener('resize', position); window.removeEventListener('scroll', position, true) }
  }, [])
  function textInput(target: TextElement) {
    return {
      value: texts[target.id] ?? target.text, spellCheck,
      onChange: (event: React.ChangeEvent<HTMLTextAreaElement>) => {
        const text = event.target.value
        setTexts((previous) => { const next = { ...previous }; if (text === target.text) delete next[target.id]; else next[target.id] = text; return next })
      },
      onSelect: (event: React.SyntheticEvent<HTMLTextAreaElement>) => {
        if (document.activeElement === event.currentTarget) setSelection({ id: target.id, start: event.currentTarget.selectionStart, end: event.currentTarget.selectionEnd })
      },
      onFocus: (event: React.FocusEvent<HTMLTextAreaElement>) => setSelection({ id: target.id, start: event.currentTarget.selectionStart, end: event.currentTarget.selectionEnd }),
    }
  }
  async function prepare(target: Element): Promise<Element> {
    if (target.type === 'table') {
      let prepared: Element = target
      for (const [row, cells] of target.rows.entries()) {
        for (const [column, text] of cells.entries()) {
          const replacement = cellTexts[`${row}:${column}`]
          if (replacement !== undefined && replacement !== text) prepared = await core<Element>({ op: 'set_table_cell_text', element: prepared, row, column, text: replacement })
        }
      }
      return prepared
    }
    if ((target.type === 'text' || target.type === 'shape') && texts[target.id] !== undefined) return onReplaceText(target, texts[target.id])
    if (target.type === 'group') {
      const children: Element[] = []
      for (const child of target.children) children.push(await prepare(child))
      return { ...target, children }
    }
    return target
  }
  const active = selection ? (draft.type === 'group' ? draft.children.find((child) => child.id === selection.id) : draft) : undefined
  const activeText = active && (active.type === 'text' || active.type === 'shape') ? active : undefined
  const range = selection && activeText ? unicodeRange(texts[activeText.id] ?? activeText.text, selection.start, selection.end) : { start: 0, end: 0 }
  async function formatRange(style: RunStyle) {
    if (!activeText || !selection || range.start === range.end || pending.current || busy || composing.current || cancelled.current) return
    pending.current = true; setBusy(true); onBusy?.(true); setError('')
    try {
      const prepared = await prepare(draft)
      if (cancelled.current) return
      const target = prepared.type === 'group' ? prepared.children.find((child) => child.id === activeText.id)! : prepared
      const formatted = await onFormatRange(target, range.start, range.end, style)
      if (cancelled.current) return
      setDraft(prepared.type === 'group' ? { ...prepared, children: prepared.children.map((child) => child.id === formatted.id ? formatted : child) } : formatted)
      setTexts({})
    } catch (reason) { if (!cancelled.current) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { pending.current = false; if (!cancelled.current) setBusy(false); onBusy?.(false) }
  }
  async function commit() {
    if (pending.current || busy || composing.current || cancelled.current) return
    pending.current = true; setBusy(true); onBusy?.(true); setError('')
    try { const prepared = await prepare(draft); if (!cancelled.current) { await onCommit(prepared); if (!cancelled.current) onCancel() } }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { pending.current = false; if (!cancelled.current) setBusy(false); onBusy?.(false) }
  }
  return <form ref={form} className="inline-editor" style={draft.type === 'group' ? { background: 'transparent' } : undefined} onSubmit={(event) => { event.preventDefault(); void commit() }}
    onPointerDown={(event) => event.stopPropagation()} onDoubleClick={(event) => event.stopPropagation()}
    onCompositionStart={() => { composing.current = true }} onCompositionEnd={() => { composing.current = false }}
    onKeyDown={(event) => {
      event.stopPropagation()
      if (event.nativeEvent.isComposing || composing.current || event.keyCode === 229) return
      if (event.key === 'Escape') { event.preventDefault(); if (!pending.current && !disabled) { cancelled.current = true; onCancel() } }
      if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) { event.preventDefault(); void commit() }
    }}>
    {(draft.type === 'text' || draft.type === 'shape') && <textarea aria-label="Slide text editor" className="inline-text" maxLength={8000} disabled={busy} style={textStyle(draft, theme)} {...textInput(draft)} />}
    {draft.type === 'group' && <div style={{ width: draft.view_width, height: draft.view_height, transformOrigin: 'top left', transform: `scale(${draft.width / draft.view_width}, ${draft.height / draft.view_height})` }}>{draft.children.map((child) => (child.type === 'text' || child.type === 'shape') && <textarea key={child.id} aria-label={`Group text ${child.id}`} className="inline-text" disabled={busy} maxLength={8000} style={{ ...textStyle(child, theme), position: 'absolute', left: child.x, top: child.y, width: child.width, height: child.height }} {...textInput(child)} />)}</div>}
    {activeText && toolbar && createPortal(<details className="inline-rich-tools" open style={{ left: toolbar.left, top: toolbar.top }}><summary>Text formatting</summary><RunControls theme={theme} disabled={busy || range.start === range.end} style={selectedRunStyle(activeText, range.start, range.end)} onChange={(style) => { void formatRange(style) }} /></details>, toolbar.container)}
    {draft.type === 'table' && <table className="slide-table inline-table" style={{ fontSize: draft.font_size }}><tbody>
      {draft.rows.map((row, rowIndex) => <tr key={rowIndex} style={{ height: `${100 / draft.rows.length}%` }}>{row.map((value, column) => <td key={column}>
        <textarea aria-label={`Table row ${rowIndex + 1} column ${column + 1}`} value={cellTexts[`${rowIndex}:${column}`] ?? value} maxLength={200} rows={1} disabled={busy}
          onChange={(event) => setCellTexts((previous) => { const next = { ...previous }; const key = `${rowIndex}:${column}`; if (event.target.value === value) delete next[key]; else next[key] = event.target.value; return next })} />
      </td>)}</tr>)}
    </tbody></table>}
    <div className="inline-edit-actions" style={element.y + element.height > 660 ? { bottom: 'auto', top: -46 } : undefined}>
      {draft.type === 'table' && <>
        <button type="button" aria-label="Add table row" title="Add row" disabled={busy || draft.rows.length >= 12} onClick={() => setDraft({ ...draft, rows: [...draft.rows, draft.rows[0].map(() => '')] })}><Rows3 size={20} /></button>
        <button type="button" aria-label="Remove last table row" title="Remove last row" disabled={busy || draft.rows.length <= 1} onClick={() => { setCellTexts((previous) => Object.fromEntries(Object.entries(previous).filter(([key]) => Number(key.split(':')[0]) < draft.rows.length - 1))); setDraft({ ...draft, rows: draft.rows.slice(0, -1) }) }}><Minus size={20} /></button>
        <button type="button" aria-label="Add table column" title="Add column" disabled={busy || draft.rows[0].length >= 8} onClick={() => setDraft({ ...draft, rows: draft.rows.map((row) => [...row, '']) })}><Columns3 size={20} /></button>
        <button type="button" aria-label="Remove last table column" title="Remove last column" disabled={busy || draft.rows[0].length <= 1} onClick={() => { setCellTexts((previous) => Object.fromEntries(Object.entries(previous).filter(([key]) => Number(key.split(':')[1]) < draft.rows[0].length - 1))); setDraft({ ...draft, rows: draft.rows.map((row) => row.slice(0, -1)) }) }}><Minus size={20} /></button>
      </>}
      <button type="submit" aria-label="Apply on-slide edit" title="Apply edit" disabled={busy}><Check size={20} /></button>
      <button type="button" aria-label="Cancel on-slide edit" title="Cancel edit" disabled={busy} onClick={() => { cancelled.current = true; onCancel() }}><X size={20} /></button>
    </div>
    {error && <div className="inline-edit-error" role="alert">{error}</div>}
  </form>
}