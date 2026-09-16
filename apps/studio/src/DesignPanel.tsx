import { useRef, useState } from 'react'
import { ArrowLeft, Check, Copy, ImagePlus, LayoutTemplate, Palette, Plus, Square, Trash2, Type } from 'lucide-react'
import { core, fileBase64 } from './api'
import { SlideSurface } from './SlideSurface'
import { ColorField, TextControls } from './TextControls'
import { Tool } from './Tool'
import type { Deck, Design, DesignPreset, Element, Placeholder, Slide } from './types'

type Target = { kind: 'master' | 'layout'; id: string }
export function DesignPanel({ design, deck, preserveStructure = false, onSave, onPreset, onBusy }: { design: Design; deck: Deck; preserveStructure?: boolean; onSave: (design: Design) => Promise<void>; onPreset: (id: string, draft: Design) => Promise<void>; onBusy: (busy: boolean) => void }) {
  const [draft, setDraft] = useState(structuredClone(design))
  const [presets, setPresets] = useState<DesignPreset[]>([])
  const [presetMode, setPresetMode] = useState(false)
  const [presetId, setPresetId] = useState('public')
  const [previewLayout, setPreviewLayout] = useState('preset-cover')
  const [target, setTarget] = useState<Target>({ kind: 'master', id: design.masters[0].id })
  const [selected, setSelected] = useState<string | null>(null)
  const [placeholder, setPlaceholder] = useState<Placeholder['kind']>('body')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const operation = useRef(false)
  const imageInput = useRef<HTMLInputElement>(null)
  const active = (target.kind === 'master' ? draft.masters : draft.layouts).find((entry) => entry.id === target.id)!
  const owner = target.kind === 'master' ? active.id : draft.layouts.find((entry) => entry.id === active.id)!.master_id
  const selectedElement = active.elements.find((element) => element.id === selected)
  const usedLayouts = new Set(deck.slides.map((slide) => slide.layout_id ?? draft.layouts[0]?.id))
  const deletable = !preserveStructure && (target.kind === 'master'
    ? draft.masters.length > 1 && !draft.layouts.some((layout) => layout.master_id === target.id && usedLayouts.has(layout.id))
    : !usedLayouts.has(target.id) && draft.layouts.filter((layout) => layout.master_id === owner).length > 1)

  function choose(next: Target) { setTarget(next); setSelected(null); setError('') }
  function editTarget(patch: { name?: string; background?: string | null; elements?: Element[] }) {
    setDraft((previous) => target.kind === 'master'
      ? { ...previous, masters: previous.masters.map((entry) => entry.id === target.id ? { ...entry, ...patch, background: patch.background ?? entry.background } : entry) }
      : { ...previous, layouts: previous.layouts.map((entry) => entry.id === target.id ? { ...entry, ...patch } : entry) })
  }
  function replaceElement(element: Element) { editTarget({ elements: active.elements.map((entry) => entry.id === element.id ? element : entry) }) }
  async function perform(action: () => Promise<void>) {
    if (operation.current) return
    operation.current = true; setBusy(true); onBusy(true); setError('')
    try { await action() } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { operation.current = false; setBusy(false); onBusy(false) }
  }
  async function add(kind: 'text' | 'shape', asPlaceholder = false) {
    const created = await core<Element>({ op: 'create_object', kind, preset: kind === 'shape' ? 'rect' : undefined, id: `common-${crypto.randomUUID().slice(0, 8)}` })
    let next = created
    if (created.type === 'text') {
      const nextIndex = Math.max(-1, ...active.elements.map((entry) => entry.type === 'text' ? entry.format?.placeholder?.index ?? -1 : -1)) + 1
      next = asPlaceholder ? { ...created, x: 64, y: 180, width: 1100, height: 300, text: placeholder === 'title' ? 'Title' : 'Content', format: { font_family: placeholder === 'title' ? '@major' : '@minor', placeholder: { kind: placeholder, index: nextIndex } } }
        : { ...created, x: 64, y: 665, width: 700, height: 30, text: 'Company footer', font_size: 14 }
    }
    editTarget({ elements: [...active.elements, next] }); setSelected(next.id)
  }
  function newMaster(duplicate: boolean) {
    const id = `master-${crypto.randomUUID().slice(0, 8)}`
    const source = draft.masters.find((master) => master.id === owner)!
    const layouts = duplicate ? draft.layouts.filter((layout) => layout.master_id === owner).map((layout) => ({ ...structuredClone(layout), id: `layout-${crypto.randomUUID().slice(0, 8)}`, master_id: id }))
      : [{ id: `layout-${crypto.randomUUID().slice(0, 8)}`, name: 'Blank', master_id: id, background: null, elements: [] }]
    if (draft.masters.length >= 8 || draft.layouts.length + layouts.length > 32) { setError('Design limit: 8 masters and 32 layouts'); return }
    setDraft({ ...draft, masters: [...draft.masters, duplicate ? { ...structuredClone(source), id, name: `${source.name} copy`.slice(0, 100) } : { id, name: 'New master', background: '@lt1', elements: [] }], layouts: [...draft.layouts, ...layouts] }); choose({ kind: 'master', id })
  }
  function newLayout(duplicate: boolean) {
    if (draft.layouts.length >= 32) { setError('Design limit: 32 layouts'); return }
    const id = `layout-${crypto.randomUUID().slice(0, 8)}`
    const source = draft.layouts.find((layout) => layout.id === target.id)
    const next = duplicate && source ? { ...structuredClone(source), id, name: `${source.name} copy`.slice(0, 100) } : { id, name: 'New layout', master_id: owner, background: null, elements: [] }
    setDraft({ ...draft, layouts: [...draft.layouts, next] }); choose({ kind: 'layout', id })
  }
  function removeTarget() {
    if (!deletable) return
    const next = target.kind === 'master' ? { ...draft, masters: draft.masters.filter((entry) => entry.id !== target.id), layouts: draft.layouts.filter((entry) => entry.master_id !== target.id) } : { ...draft, layouts: draft.layouts.filter((entry) => entry.id !== target.id) }
    setDraft(next); choose({ kind: 'master', id: next.masters[0].id })
  }
  async function browsePresets() {
    await perform(async () => { if (!presets.length) setPresets(await core<DesignPreset[]>({ op: 'design_presets' })); setPresetMode(true) })
  }
  if (presetMode && presets.length) {
    const preset = presets.find((entry) => entry.id === presetId) ?? presets[0]
    const layout = preset.design.layouts.find((entry) => entry.id === previewLayout) ?? preset.design.layouts[1]
    const slide: Slide = { id: 'preset-preview', title: layout.name, background: '@lt1', inherit_background: true, layout_id: layout.id, notes: '', elements: layout.elements }
    const previewDesign: Design = { ...preset.design, layouts: preset.design.layouts.map((entry) => entry.id === layout.id ? { ...entry, elements: [] } : entry) }
    const allowed = !preserveStructure || draft.masters.some((master) => master.id === 'preset-master')
    return <div className="preset-browser">
      <div className="preset-browser-header"><button type="button" className="secondary" disabled={busy} onClick={() => setPresetMode(false)}><ArrowLeft size={18} />Edit masters</button><strong>Master presets</strong></div>
      <div className="master-preset-list">{presets.map((entry) => <button type="button" className="master-preset-choice" key={entry.id} aria-label={`Preset ${entry.name}`} aria-pressed={entry.id === preset.id} disabled={busy} onClick={() => setPresetId(entry.id)}><span className="preset-swatch-row" aria-hidden="true">{['dk1', 'accent1', 'accent2', 'accent3', 'lt2'].map((slot) => <i key={slot} style={{ background: `#${entry.design.theme.colors[slot]}` }} />)}</span><strong>{entry.name}</strong></button>)}</div>
      <div className="preset-workspace"><section className="preset-preview"><label className="field">Layout<select aria-label="Preset preview layout" value={layout.id} disabled={busy} onChange={(event) => setPreviewLayout(event.target.value)}>{preset.design.layouts.map((entry) => <option key={entry.id} value={entry.id}>{entry.name}</option>)}</select></label><SlideSurface slide={slide} design={previewDesign} /></section>
        <aside className="preset-rules"><h3>{preset.name}</h3><dl><dt>Margins</dt><dd aria-label="Preset margin">{preset.rules.margin} px</dd><dt>Column gap</dt><dd aria-label="Preset gutter">{preset.rules.gutter} px</dd><dt>Heading font</dt><dd aria-label="Preset heading font">{preset.design.theme.fonts.major}</dd><dt>Body font</dt><dd>{preset.design.theme.fonts.minor}</dd><dt>East Asian font</dt><dd>{preset.design.theme.fonts.east_asian}</dd><dt>Heading / body</dt><dd>{preset.rules.heading_size} / {preset.rules.body_size} px</dd></dl><h4>Content regions</h4>{preset.rules.regions.filter((entry) => entry.layout_id === layout.id).map((entry) => <p key={entry.name}>{entry.name}<br />{entry.x}, {entry.y} / {entry.width.toFixed(0)} x {entry.height.toFixed(0)} px</p>)}</aside>
      </div>
      {!allowed && <p role="status">This imported presentation has no preset master. Create a new presentation to add the preset layouts.</p>}
      {error && <p className="error modal-error" role="alert">{error}</p>}
      <div className="modal-actions"><span>{preset.design.layouts.length} layouts</span><button type="button" className="primary" disabled={busy || !allowed} onClick={() => void perform(() => onPreset(preset.id, draft))}><Check size={18} />Use preset</button></div>
    </div>
  }
  const preview: Slide = { id: target.id, title: active.name, background: active.background ?? draft.masters.find((entry) => entry.id === owner)!.background, elements: active.elements, notes: '', hide_master_graphics: target.kind === 'master', layout_id: target.kind === 'layout' ? target.id : undefined }
  const previewDesign = { ...draft, layouts: draft.layouts.map((layout) => layout.id === target.id ? { ...layout, elements: [] } : layout) }
  return <div className="design-editor">
    <div className="preset-browser-header"><button type="button" className="secondary" disabled={busy} onClick={() => void browsePresets()}><Palette size={18} />Browse presets</button></div>
    <fieldset className="design-panel" disabled={busy}>
      <aside className="design-tree" aria-label="Masters and layouts">
        <div className="design-tree-tools"><Tool label="New master" disabled={busy || preserveStructure} onClick={() => newMaster(false)}><Plus size={20} /></Tool><Tool label="New layout" disabled={busy || preserveStructure} onClick={() => newLayout(false)}><LayoutTemplate size={20} /></Tool><Tool label="Duplicate design" disabled={busy || preserveStructure} onClick={() => target.kind === 'master' ? newMaster(true) : newLayout(true)}><Copy size={20} /></Tool><Tool label="Delete design" disabled={busy || !deletable} onClick={removeTarget}><Trash2 size={20} /></Tool></div>
        {draft.masters.map((master) => <div key={master.id}><button className={target.id === master.id ? 'active' : ''} aria-label={`Master ${master.name}`} onClick={() => choose({ kind: 'master', id: master.id })}><Square size={16} />{master.name}</button>
          {draft.layouts.filter((layout) => layout.master_id === master.id).map((layout) => <button key={layout.id} className={`design-layout ${target.id === layout.id ? 'active' : ''}`} aria-label={`Layout ${layout.name}`} onClick={() => choose({ kind: 'layout', id: layout.id })}><LayoutTemplate size={15} />{layout.name}</button>)}
        </div>)}
      </aside>
      <section className="design-canvas">
        <div className="design-canvas-toolbar"><Tool label="Add common text" disabled={busy} onClick={() => void perform(() => add('text'))}><Type size={20} /></Tool><Tool label="Add common shape" disabled={busy} onClick={() => void perform(() => add('shape'))}><Square size={20} /></Tool><Tool label="Add common picture" disabled={busy} onClick={() => imageInput.current?.click()}><ImagePlus size={20} /></Tool>
          {target.kind === 'layout' && <><select aria-label="Placeholder kind" value={placeholder} onChange={(event) => setPlaceholder(event.target.value as Placeholder['kind'])}>{['title', 'body', 'subtitle', 'footer', 'date', 'slide_number'].map((kind) => <option key={kind} value={kind}>{kind.replace('_', ' ')}</option>)}</select><Tool label="Add placeholder" disabled={busy} onClick={() => void perform(() => add('text', true))}><Plus size={20} /></Tool></>}
        </div>
        <input ref={imageInput} hidden type="file" aria-label="Design picture file" accept="image/png,image/jpeg" onChange={(event) => {
          const file = event.target.files?.[0]; event.target.value = ''; if (!file) return
          void perform(async () => { if (file.size > 1024 * 1024) throw new Error('Picture must be at most 1 MiB'); const picture = await core<Element>({ op: 'create_picture', id: `logo-${crypto.randomUUID().slice(0, 8)}`, base64: await fileBase64(file), mime_type: file.type, alt: file.name }); const factor = Math.min(200 / picture.width, 100 / picture.height); const next = { ...picture, x: 1000, y: 40, width: picture.width * factor, height: picture.height * factor }; editTarget({ elements: [...active.elements, next] }); setSelected(next.id) })
        }} />
        <SlideSurface slide={preview} design={previewDesign} selected={selected} onSelect={setSelected} onMove={(id, x, y) => { const element = active.elements.find((entry) => entry.id === id); if (element) replaceElement({ ...element, x, y }) }} onResize={(id, width, height) => { const element = active.elements.find((entry) => entry.id === id); if (element) replaceElement({ ...element, width, height }) }} onEdit={async (element) => replaceElement(element)} />
        <div className="design-layers">{active.elements.map((element) => <button key={element.id} className={selected === element.id ? 'active' : ''} onClick={() => setSelected(element.id)}>{element.id}</button>)}</div>
      </section>
      <aside className="design-properties">
        <label className="field">{target.kind === 'master' ? 'Master name' : 'Layout name'}<input aria-label="Design name" value={active.name} maxLength={100} onChange={(event) => editTarget({ name: event.target.value })} /></label>
        {target.kind === 'layout' && <label className="checkbox"><input type="checkbox" checked={active.background === null} onChange={(event) => editTarget({ background: event.target.checked ? null : '@lt1' })} />Inherit master background</label>}
        {active.background !== null && <ColorField label="Design background" value={active.background} theme={draft.theme} onChange={(background) => editTarget({ background })} />}
        {selectedElement && <>
          <div className="selection-heading"><strong>{selectedElement.id}</strong><Tool label="Delete design element" disabled={busy} onClick={() => { editTarget({ elements: active.elements.filter((entry) => entry.id !== selectedElement.id) }); setSelected(null) }}><Trash2 size={20} /></Tool></div>
          <div className="geometry-inputs">{(['x', 'y', 'width', 'height'] as const).map((field) => <label key={field}><span>{field}</span><input aria-label={`Design element ${field}`} type="number" value={selectedElement[field]} onChange={(event) => replaceElement({ ...selectedElement, [field]: event.currentTarget.valueAsNumber })} /></label>)}</div>
          {(selectedElement.type === 'text' || selectedElement.type === 'shape') && <TextControls element={selectedElement} theme={draft.theme} textLabel="Design element text" onChange={replaceElement} />}
          {(selectedElement.type === 'shape' || selectedElement.type === 'rect') && <ColorField label="Design element fill" value={selectedElement.fill} theme={draft.theme} onChange={(fill) => replaceElement({ ...selectedElement, fill })} />}
          {selectedElement.type === 'text' && selectedElement.format?.placeholder && <>
            <label className="field">Placeholder type<select aria-label="Selected placeholder type" value={selectedElement.format.placeholder.kind} onChange={(event) => replaceElement({ ...selectedElement, format: { ...selectedElement.format, placeholder: { ...selectedElement.format!.placeholder!, kind: event.target.value as Placeholder['kind'] } } })}>{['title', 'body', 'subtitle', 'footer', 'date', 'slide_number'].map((kind) => <option key={kind} value={kind}>{kind.replace('_', ' ')}</option>)}</select></label>
            <label className="field">Placeholder index<input aria-label="Placeholder index" type="number" min={0} max={255} value={selectedElement.format.placeholder.index} onChange={(event) => replaceElement({ ...selectedElement, format: { ...selectedElement.format, placeholder: { ...selectedElement.format!.placeholder!, index: event.currentTarget.valueAsNumber } } })} /></label>
          </>}
        </>}
      </aside>
    </fieldset>
    {error && <p className="error modal-error" role="alert">{error}</p>}
    <div className="modal-actions"><span>{draft.masters.length} masters / {draft.layouts.length} layouts</span><button className="primary" disabled={busy} onClick={() => void perform(() => onSave(draft))}><Check size={17} />Save design</button></div>
  </div>
}