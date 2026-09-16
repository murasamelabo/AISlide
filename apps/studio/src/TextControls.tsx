import { useId } from 'react'
import { AlignCenter, AlignJustify, AlignLeft, AlignRight, Bold, Italic, List, ListOrdered, Underline } from 'lucide-react'
import { cssColor } from './design'
import { Tool } from './Tool'
import type { Element, TextFormat, Theme } from './types'

export function ColorField({ label, value, theme, onChange }: { label: string; value: string; theme?: Theme; onChange: (value: string) => void }) {
  return <div className="color-field"><label className="field">{label}<input aria-label={label} type="color" value={cssColor(value, theme)} onChange={(event) => onChange(event.target.value.slice(1).toUpperCase())} /></label>
    {theme && <select aria-label={`${label} binding`} value={value.startsWith('@') ? value : 'custom'} onChange={(event) => onChange(event.target.value === 'custom' ? cssColor(value, theme).slice(1) : event.target.value)}><option value="custom">Custom</option>{Object.keys(theme.colors).map((key) => <option key={key} value={`@${key}`}>{key}</option>)}</select>}
  </div>
}

export function TextControls({ element, theme, onChange, textLabel = 'Text content' }: { element: Extract<Element, { type: 'text' | 'shape' }>; theme?: Theme; onChange: (element: Element) => void; textLabel?: string }) {
  const fontOptionsId = useId()
  const format = element.format ?? {}
  const changeFormat = (patch: TextFormat) => onChange({ ...element, format: { ...format, ...patch } })
  return <>
    <label className="field">{textLabel}<textarea aria-label={textLabel} rows={4} maxLength={4000} value={element.text} onChange={(event) => onChange({ ...element, text: event.target.value })} /></label>
    <div className="inline-fields"><label className="field">Size<input aria-label="Text size" type="number" min={8} max={120} required value={element.font_size} onChange={(event) => onChange({ ...element, font_size: event.currentTarget.valueAsNumber })} /></label><ColorField label="Text color" value={element.color} theme={theme} onChange={(color) => onChange({ ...element, color })} /></div>
    <label className="field">Font<input aria-label="Text font" list={fontOptionsId} value={format.font_family ?? '@minor'} maxLength={100} onChange={(event) => changeFormat({ font_family: event.target.value || null })} /></label>
    <datalist id={fontOptionsId}>{['@major', '@minor', 'Aptos', 'Arial', 'Yu Gothic', 'Meiryo', 'Segoe UI', 'Georgia'].map((font) => <option key={font} value={font} />)}</datalist>
    <div className="text-format-bar" role="group" aria-label="Text style">
      <Tool label="Bold" pressed={element.bold} onClick={() => onChange({ ...element, bold: !element.bold })}><Bold size={20} /></Tool>
      <Tool label="Italic" pressed={Boolean(format.italic)} onClick={() => changeFormat({ italic: !format.italic })}><Italic size={20} /></Tool>
      <Tool label="Underline" pressed={Boolean(format.underline)} onClick={() => changeFormat({ underline: !format.underline })}><Underline size={20} /></Tool>
      <Tool label="Bullets" pressed={format.bullet === 'bullet'} onClick={() => changeFormat({ bullet: format.bullet === 'bullet' ? 'none' : 'bullet' })}><List size={20} /></Tool>
      <Tool label="Numbering" pressed={format.bullet === 'numbered'} onClick={() => changeFormat({ bullet: format.bullet === 'numbered' ? 'none' : 'numbered' })}><ListOrdered size={20} /></Tool>
    </div>
    <div className="text-format-bar" role="group" aria-label="Text alignment">{([['left', AlignLeft], ['center', AlignCenter], ['right', AlignRight], ['justify', AlignJustify]] as const).map(([alignment, Icon]) => <Tool key={alignment} label={`Align ${alignment}`} pressed={(format.alignment ?? 'left') === alignment} onClick={() => changeFormat({ alignment })}><Icon size={20} /></Tool>)}</div>
    <label className="field">Vertical alignment<select aria-label="Vertical alignment" value={format.vertical ?? 'top'} onChange={(event) => changeFormat({ vertical: event.target.value as TextFormat['vertical'] })}><option value="top">Top</option><option value="middle">Middle</option><option value="bottom">Bottom</option></select></label>
    <label className="field">Hyperlink<input aria-label="Hyperlink" type="text" placeholder="https://" maxLength={2048} value={format.hyperlink ?? ''} onChange={(event) => changeFormat({ hyperlink: event.target.value || null })} /></label>
  </>
}