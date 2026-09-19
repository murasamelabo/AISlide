import { useEffect, useId, useImperativeHandle, useRef, useState } from 'react'
import type { Ref } from 'react'
import { AlignCenter, AlignJustify, AlignLeft, AlignRight, Bold, Italic, List, ListOrdered, Underline, Subscript, Superscript, Highlighter, Plus, X } from 'lucide-react'
import { cssColor } from './design'
import { Tool } from './Tool'
import type { Element, TextFormat, Theme, RichParagraph, RichSpacing, RunStyle } from './types'
import { core } from './api'
import './text-tools.css'

export type TextElement = Extract<Element, { type: 'text' | 'shape' }>
export type TextDraftAdapters = {
  onFormatRange?: (element: Element, start: number, end: number, style: RunStyle) => Promise<Element>
  onReplaceText?: (element: Element, text: string) => Promise<Element>
}

export function unicodeRange(text: string, start: number, end: number) {
  return { start: Array.from(text.slice(0, start)).length, end: Array.from(text.slice(0, end)).length }
}

export function selectedRunStyle(element: TextElement, start: number, end: number): RunStyle {
  const base: RunStyle = { bold: element.bold, italic: element.format?.italic ?? false, underline: element.format?.underline ?? false, color: element.color, font_size: element.font_size, font_family: element.format?.font_family ?? '@minor', baseline: 0, highlight: 'none' }
  let offset = 0
  const styles: RunStyle[] = []
  for (const paragraph of element.format?.paragraphs ?? []) {
    for (const run of paragraph.runs) {
      const length = Array.from(run.text).length
      if (offset < end && offset + length > start) styles.push({ ...base, ...run.style })
      offset += length
    }
    offset += 1
  }
  if (!styles.length) return base
  return Object.fromEntries(Object.entries(styles[0]).filter(([key, value]) => styles.every((style) => style[key as keyof RunStyle] === value)))
}

export function RunControls({ style, theme, disabled = false, onChange, scopeLabel = 'Selected text' }: { style: RunStyle; theme?: Theme; disabled?: boolean; onChange: (style: RunStyle) => void; scopeLabel?: string }) {
  return <fieldset className="run-controls" disabled={disabled}><legend>{scopeLabel}</legend>
    <div className="text-format-bar" role="group" aria-label="Selected text style">
      <Tool label="Bold" disabled={disabled} pressed={style.bold === true} onClick={() => onChange({ bold: !style.bold })}><Bold size={20} /></Tool>
      <Tool label="Italic" disabled={disabled} pressed={style.italic === true} onClick={() => onChange({ italic: !style.italic })}><Italic size={20} /></Tool>
      <Tool label="Underline" disabled={disabled} pressed={style.underline === true} onClick={() => onChange({ underline: !style.underline })}><Underline size={20} /></Tool>
      <Tool label="Superscript" disabled={disabled} pressed={(style.baseline ?? 0) > 0} onClick={() => onChange({ baseline: (style.baseline ?? 0) > 0 ? 0 : 30000 })}><Superscript size={20} /></Tool>
      <Tool label="Subscript" disabled={disabled} pressed={(style.baseline ?? 0) < 0} onClick={() => onChange({ baseline: (style.baseline ?? 0) < 0 ? 0 : -25000 })}><Subscript size={20} /></Tool>
    </div>
    <div className="text-tools-grid">
      <label className="field">Size (px)<input aria-label="Selection font size" type="number" min={1} max={400} step={1} value={style.font_size ?? ''} placeholder="Mixed" onChange={(event) => { if (event.currentTarget.value && event.currentTarget.validity.valid) onChange({ font_size: event.currentTarget.valueAsNumber }) }} /></label>
      <ColorField label="Selection color" value={style.color ?? '000000'} theme={theme} onChange={(color) => onChange({ color })} />
      <ColorField label="Selection highlight" value={!style.highlight || style.highlight === 'none' ? 'FFFF00' : style.highlight} theme={theme} onChange={(highlight) => onChange({ highlight })} />
      <Tool label="Clear highlight" disabled={disabled} onClick={() => onChange({ highlight: 'none' })}><Highlighter size={20} /></Tool>
    </div>
  </fieldset>
}

type ParagraphPatch = Omit<Partial<RichParagraph>, 'runs'>

export function ParagraphControls({ paragraph, paragraphIndex, paragraphCount, onSelectParagraph, onChange, disabled = false }: { paragraph: RichParagraph; paragraphIndex: number; paragraphCount: number; onSelectParagraph: (index: number) => void; onChange: (patch: ParagraphPatch) => void; disabled?: boolean }) {
  function spacing(label: string, key: 'line_spacing' | 'space_before' | 'space_after') {
    const value = paragraph[key]
    const kind = value?.kind ?? (key === 'line_spacing' ? 'percent' : 'points')
    const scale = kind === 'percent' ? 1000 : 100
    return <div className="text-tools-grid" key={key}>
      <label className="field">{label}<input aria-label={label} type="number" step={0.01} min={key === 'line_spacing' ? 0.01 : 0} max={kind === 'percent' ? 1000 : 1584} value={value ? value.value / scale : ''} placeholder="Inherited" onChange={(event) => { if (!event.currentTarget.value) onChange({ [key]: null }); else if (event.currentTarget.validity.valid) onChange({ [key]: { kind, value: Math.round(event.currentTarget.valueAsNumber * scale) } as RichSpacing }) }} /></label>
      <label className="field">Unit<select aria-label={`${label} unit`} value={kind} onChange={(event) => { const next = event.currentTarget.value as RichSpacing['kind']; onChange({ [key]: { kind: next, value: next === 'percent' ? 100000 : key === 'line_spacing' ? 2400 : 0 } }) }}><option value="percent">Percent</option><option value="points">Points</option></select></label>
    </div>
  }
  return <fieldset className="paragraph-controls" disabled={disabled}><legend>Paragraph</legend>
    <label className="field">Paragraph<select aria-label="Paragraph" value={paragraphIndex} onChange={(event) => onSelectParagraph(Number(event.target.value))}>{Array.from({ length: paragraphCount }, (_, index) => <option key={index} value={index}>{index + 1}</option>)}</select></label>
    <div className="text-format-bar" role="group" aria-label="Paragraph alignment">{([['left', AlignLeft], ['center', AlignCenter], ['right', AlignRight], ['justify', AlignJustify]] as const).map(([alignment, Icon]) => <Tool key={alignment} label={`Paragraph align ${alignment}`} disabled={disabled} pressed={paragraph.alignment === alignment} onClick={() => onChange({ alignment })}><Icon size={20} /></Tool>)}</div>
    <label className="field">List<select aria-label="Paragraph list" value={paragraph.bullet ?? 'none'} onChange={(event) => { const bullet = event.target.value as RichParagraph['bullet']; onChange({ bullet, numbering: bullet === 'numbered' ? 'arabicPeriod' : null, number_start: bullet === 'numbered' ? 1 : null, bullet_character: null }) }}><option value="none">None</option><option value="bullet">Bullets</option><option value="numbered">Numbered</option></select></label>
    {paragraph.bullet === 'numbered' && <div className="text-tools-grid"><label className="field">Number style<select aria-label="Number style" value={paragraph.numbering ?? 'arabicPeriod'} onChange={(event) => onChange({ numbering: event.target.value as RichParagraph['numbering'] })}>{(['arabicPeriod', 'arabicParenR', 'arabicParenBoth', 'arabicPlain', 'alphaLcPeriod', 'alphaUcPeriod', 'alphaLcParenR', 'alphaUcParenR', 'romanLcPeriod', 'romanUcPeriod'] as const).map((value, index) => <option key={value} value={value}>{['1.', '1)', '(1)', '1', 'a.', 'A.', 'a)', 'A)', 'i.', 'I.'][index]}</option>)}</select></label><label className="field">Start at<input aria-label="Number start" type="number" min={1} max={32767} step={1} value={paragraph.number_start ?? 1} onChange={(event) => { if (event.currentTarget.validity.valid && event.currentTarget.value) onChange({ number_start: event.currentTarget.valueAsNumber }) }} /></label></div>}
    <div className="text-tools-grid">{([['Indent level', 'level', 0, 8, 1], ['Left margin (px)', 'margin_left', 0, 5376, 9525], ['Hanging indent (px)', 'indent', -5376, 5376, 9525]] as const).map(([label, key, min, max, scale]) => <label className="field" key={key}>{label}<input aria-label={label} type="number" min={min} max={max} step={1} value={(paragraph[key] ?? 0) / scale} onChange={(event) => { if (event.currentTarget.value && event.currentTarget.validity.valid) onChange({ [key]: Math.round(event.currentTarget.valueAsNumber * scale) }) }} /></label>)}</div>
    {spacing('Line spacing', 'line_spacing')}{spacing('Space before', 'space_before')}{spacing('Space after', 'space_after')}
    <div className="paragraph-tabs"><span>Tab stops</span>{(paragraph.tabs ?? []).map((tab, index, tabs) => <div className="text-tools-grid" key={index}>
      <label className="field">Position (px)<input aria-label={`Tab ${index + 1} position`} type="number" min={index ? Math.floor(tabs[index - 1].position / 9525) + 1 : 0} max={index + 1 < tabs.length ? Math.ceil(tabs[index + 1].position / 9525) - 1 : 5376} step={1} value={tab.position / 9525} onChange={(event) => { if (event.currentTarget.value && event.currentTarget.validity.valid) onChange({ tabs: tabs.map((entry, position) => position === index ? { ...entry, position: Math.round(event.currentTarget.valueAsNumber * 9525) } : entry) }) }} /></label>
      <label className="field">Alignment<select aria-label={`Tab ${index + 1} alignment`} value={tab.alignment ?? 'left'} onChange={(event) => onChange({ tabs: tabs.map((entry, position) => position === index ? { ...entry, alignment: event.target.value as typeof tab.alignment } : entry) })}>{['left', 'center', 'right', 'decimal'].map((alignment) => <option key={alignment}>{alignment}</option>)}</select></label>
      <Tool label={`Remove tab ${index + 1}`} disabled={disabled} onClick={() => onChange({ tabs: tabs.filter((_, position) => position !== index) })}><X size={20} /></Tool>
    </div>)}<Tool label="Add tab stop" disabled={disabled || (paragraph.tabs?.length ?? 0) >= 32 || (paragraph.tabs?.at(-1)?.position ?? 0) + 457200 > 51206400} onClick={() => onChange({ tabs: [...paragraph.tabs ?? [], { position: (paragraph.tabs?.at(-1)?.position ?? 0) + 457200, alignment: 'left' }] })}><Plus size={20} /></Tool></div>
  </fieldset>
}

export function ColorField({ label, value, theme, onChange }: { label: string; value: string; theme?: Theme; onChange: (value: string) => void }) {
  return <div className="color-field"><label className="field">{label}<input aria-label={label} type="color" value={cssColor(value, theme)} onChange={(event) => onChange(event.target.value.slice(1).toUpperCase())} /></label>
    {theme && <select aria-label={`${label} binding`} value={value.startsWith('@') ? value : 'custom'} onChange={(event) => onChange(event.target.value === 'custom' ? cssColor(value, theme).slice(1) : event.target.value)}><option value="custom">Custom</option>{Object.keys(theme.colors).map((key) => <option key={key} value={`@${key}`}>{key}</option>)}</select>}
  </div>
}

export type TextControlsProps = TextDraftAdapters & { element: TextElement; theme?: Theme; onChange: (element: Element) => void; textLabel?: string; disabled?: boolean; onBusy?: (busy: boolean) => void; onPendingTextChange?: (pending: boolean) => void; spellCheck?: boolean; prepareRef?: Ref<{ prepare: () => Promise<Element | undefined> }> }

export function TextControls(props: TextControlsProps) {
  return <TextControlsDraft key={props.element.id} {...props} />
}

function TextControlsDraft({ element, theme, onChange, textLabel = 'Text content', disabled = false, onBusy, onPendingTextChange, spellCheck = false, prepareRef,
  onFormatRange = (element, start, end, style) => core<Element>({ op: 'format_text_element', element, start, end, style }),
  onReplaceText = (element, text) => core<Element>({ op: 'replace_element_text', element, text }),
}: TextControlsProps) {
  const fontOptionsId = useId()
  const [textDraft, setTextDraft] = useState<string | null>(null)
  const [selection, setSelection] = useState({ start: 0, end: 0 })
  const [paragraphIndex, setParagraphIndex] = useState(0)
  const [working, setWorking] = useState(false)
  const [error, setError] = useState('')
  const pending = useRef(false)
  const alive = useRef(true)
  const composing = useRef(false)
  const latest = useRef(element)
  useEffect(() => { latest.current = element }, [element])
  useEffect(() => { alive.current = true; return () => { alive.current = false } }, [])
  useEffect(() => { onPendingTextChange?.(textDraft !== null); return () => onPendingTextChange?.(false) }, [textDraft, onPendingTextChange])
  const busy = working || disabled
  const range = unicodeRange(textDraft ?? element.text, selection.start, selection.end)
  const format = element.format ?? {}
  const hasRich = Boolean(format.paragraphs?.length && element.text)
  const selected = range.end > range.start
  const paragraphs = format.paragraphs?.length ? format.paragraphs : element.text.split('\n').map((text) => ({ runs: [{ text, style: {} }] }))
  const selectedParagraph = Math.min(paragraphIndex, paragraphs.length - 1)
  const changeFormat = (patch: TextFormat) => onChange({ ...element, format: { ...format, ...patch } })
  async function prepare(style?: RunStyle, wholeText = false) {
    if (pending.current || busy || composing.current) return
    pending.current = true; setWorking(true); onBusy?.(true); setError('')
    const original = element
    try {
      let next: Element = textDraft === null ? element : await onReplaceText(element, textDraft)
      if (!alive.current || latest.current !== original) return
      if (style) next = await onFormatRange(next, wholeText ? 0 : range.start, wholeText ? Array.from(textDraft ?? element.text).length : range.end, style)
      if (!alive.current || latest.current !== original) return
      onChange(next); setTextDraft(null)
      return next
    } catch (reason) { if (alive.current) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { pending.current = false; if (alive.current) setWorking(false); onBusy?.(false) }
  }
  useImperativeHandle(prepareRef, () => ({ prepare: () => prepare() }))
  return <fieldset className="text-controls" disabled={busy} onCompositionStart={() => { composing.current = true }} onCompositionEnd={() => { composing.current = false }}>
    <label className="field">{textLabel}<textarea aria-label={textLabel} rows={4} maxLength={8000} spellCheck={spellCheck} value={textDraft ?? element.text} onSelect={(event) => { if (document.activeElement === event.currentTarget) setSelection({ start: event.currentTarget.selectionStart, end: event.currentTarget.selectionEnd }) }} onChange={(event) => {
      if (format.paragraphs?.length) setTextDraft(event.target.value === element.text ? null : event.target.value)
      else onChange({ ...element, text: event.target.value })
    }} onKeyDown={(event) => {
      event.stopPropagation()
      if (event.nativeEvent.isComposing || composing.current || event.keyCode === 229) return
      if (event.key === 'Enter' && (event.ctrlKey || event.metaKey) && textDraft !== null) { event.preventDefault(); void prepare() }
      if (event.key === 'Escape' && !pending.current) { event.preventDefault(); setTextDraft(null) }
    }} /></label>
    {textDraft !== null && <button type="button" disabled={busy} onClick={() => { void prepare() }}>Apply text content</button>}
    {(selected || hasRich) && <RunControls theme={theme} disabled={busy} scopeLabel={selected ? 'Selected text' : 'All text'} style={selectedRunStyle(element, selected ? range.start : 0, selected ? range.end : Array.from(element.text).length)} onChange={(style) => { void prepare(style, !selected) }} />}
    <fieldset disabled={busy || textDraft !== null || range.end > range.start} className="text-frame-controls">
    {!hasRich && <>
    <div className="inline-fields"><label className="field">Size<input aria-label="Text size" type="number" min={8} max={120} required value={element.font_size} onChange={(event) => { if (event.currentTarget.value && event.currentTarget.validity.valid) onChange({ ...element, font_size: event.currentTarget.valueAsNumber }) }} /></label><ColorField label="Text color" value={element.color} theme={theme} onChange={(color) => onChange({ ...element, color })} /></div>
    <label className="field">Font<input aria-label="Text font" list={fontOptionsId} value={format.font_family ?? '@minor'} maxLength={100} onChange={(event) => changeFormat({ font_family: event.target.value || null })} /></label>
    <datalist id={fontOptionsId}>{['@major', '@minor', 'Aptos', 'Arial', 'Yu Gothic', 'Meiryo', 'Segoe UI', 'Georgia'].map((font) => <option key={font} value={font} />)}</datalist>
    {range.end === range.start && <div className="text-format-bar" role="group" aria-label="Text style">
      <Tool label="Bold" pressed={element.bold} onClick={() => onChange({ ...element, bold: !element.bold })}><Bold size={20} /></Tool>
      <Tool label="Italic" pressed={Boolean(format.italic)} onClick={() => changeFormat({ italic: !format.italic })}><Italic size={20} /></Tool>
      <Tool label="Underline" pressed={Boolean(format.underline)} onClick={() => changeFormat({ underline: !format.underline })}><Underline size={20} /></Tool>
      <Tool label="Bullets" pressed={format.bullet === 'bullet'} onClick={() => changeFormat({ bullet: format.bullet === 'bullet' ? 'none' : 'bullet' })}><List size={20} /></Tool>
      <Tool label="Numbering" pressed={format.bullet === 'numbered'} onClick={() => changeFormat({ bullet: format.bullet === 'numbered' ? 'none' : 'numbered' })}><ListOrdered size={20} /></Tool>
    </div>}
    <div className="text-format-bar" role="group" aria-label="Text alignment">{([['left', AlignLeft], ['center', AlignCenter], ['right', AlignRight], ['justify', AlignJustify]] as const).map(([alignment, Icon]) => <Tool key={alignment} label={`Align ${alignment}`} pressed={(format.alignment ?? 'left') === alignment} onClick={() => changeFormat({ alignment })}><Icon size={20} /></Tool>)}</div>
    </>}
    <label className="field">Vertical alignment<select aria-label="Vertical alignment" value={format.vertical ?? 'top'} onChange={(event) => changeFormat({ vertical: event.target.value as TextFormat['vertical'] })}><option value="top">Top</option><option value="middle">Middle</option><option value="bottom">Bottom</option></select></label>
    <label className="field">Hyperlink<input aria-label="Hyperlink" type="text" placeholder="https://" maxLength={2048} value={format.hyperlink ?? ''} onChange={(event) => changeFormat({ hyperlink: event.target.value || null })} /></label>
    </fieldset>
    <ParagraphControls paragraph={paragraphs[selectedParagraph]} paragraphIndex={selectedParagraph} paragraphCount={paragraphs.length} onSelectParagraph={setParagraphIndex} disabled={busy || textDraft !== null} onChange={(patch) => changeFormat({ inherit_layout: false, paragraphs: paragraphs.map((paragraph, index) => index === selectedParagraph ? { ...paragraph, ...patch } : paragraph) })} />
    {error && <p role="alert">{error}</p>}
  </fieldset>
}