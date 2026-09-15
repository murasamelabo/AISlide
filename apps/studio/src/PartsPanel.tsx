import { useDeferredValue, useEffect, useRef, useState } from 'react'
import { Check, Code2, List, Plus, RotateCcw, Search, Trash2 } from 'lucide-react'
import { core } from './api'
import { Content } from './SlideSurface'
import { cssColor } from './design'
import type { Element, PartCatalog, PartInstance, PartPreset, PartSpec, Theme } from './types'
import './parts.css'

type FieldValue = string | number | boolean | null | FieldValue[] | { [key: string]: FieldValue }
let previewQueue: Promise<void> = Promise.resolve()

function Fields({ value, label, onChange }: { value: FieldValue; label: string; onChange: (value: FieldValue) => void }) {
  if (Array.isArray(value)) return <div className="part-array"><div className="part-array-heading"><h4>{label}</h4><button type="button" className="tool" aria-label={`Add ${label}`} title={`Add ${label}`} disabled={!value.length || value.length >= 12} onClick={() => onChange([...value, structuredClone(value[0])])}><Plus size={16} /></button></div><div className={value.every((item) => typeof item !== 'object') ? 'part-values' : 'part-records'}>{value.map((item, index) => <div className="part-record" key={index}><Fields value={item} label={`${label} ${index + 1}`} onChange={(next) => onChange(value.map((old, position) => position === index ? next : old))} /><button type="button" className="tool" aria-label={`Remove ${label} ${index + 1}`} title={`Remove ${label} ${index + 1}`} disabled={value.length <= 1} onClick={() => onChange(value.filter((_, position) => position !== index))}><Trash2 size={14} /></button></div>)}</div></div>
  if (value !== null && typeof value === 'object') return <div className="part-fields">{Object.entries(value).filter(([key]) => key !== 'kind').map(([key, item]) => <Fields key={key} value={item} label={label === 'Data' ? key.replaceAll('_', ' ') : `${label} ${key.replaceAll('_', ' ')}`} onChange={(next) => onChange({ ...value, [key]: next })} />)}</div>
  if (typeof value === 'boolean') return <label className="checkbox"><input type="checkbox" checked={value} onChange={(event) => onChange(event.target.checked)} />{label}</label>
  const numeric = typeof value === 'number' || / value$/.test(label)
  return <label className="field">{label}<input aria-label={label} type={numeric ? 'number' : 'text'} step={numeric ? 'any' : undefined} value={value ?? ''} maxLength={numeric ? undefined : 120} onChange={(event) => onChange(numeric ? event.target.value === '' ? null : event.currentTarget.valueAsNumber : event.target.value || (value === null ? null : ''))} /></label>
}

function Preview({ element, theme }: { element: Element | null; theme?: Theme }) {
  const host = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(500)
  useEffect(() => {
    if (!host.current) return
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width))
    observer.observe(host.current)
    return () => observer.disconnect()
  }, [])
  return <div ref={host} className="part-preview" role="img" aria-label="Part preview" style={{ background: cssColor('@lt1', theme) }}>{element ? <div className="part-preview-content" style={{ transform: `scale(${width / 1152})` }}><Content element={{ ...element, width: 1152, height: 512 }} theme={theme} /></div> : <span role="status">Rendering preview</span>}</div>
}

export function PartsPanel({ catalog, theme, instance, onApply, onBusy }: { catalog: PartCatalog; theme?: Theme; instance?: PartInstance | null; onApply: (spec: PartSpec) => Promise<void>; onBusy: (busy: boolean) => void }) {
  const initial = instance?.spec ?? catalog.presets[0].example
  const [spec, setSpec] = useState<PartSpec>(structuredClone(initial))
  const [category, setCategory] = useState(initial.preset.split('/')[0])
  const [query, setQuery] = useState('')
  const [mode, setInputMode] = useState<'fields' | 'json'>('fields')
  const drafts = useRef(new Map<string, { spec: PartSpec; raw: string; mode: 'fields' | 'json' }>())
  const [raw, setRaw] = useState(JSON.stringify(initial.data, null, 2))
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const applying = useRef(false)
  const [render, setRender] = useState<{ key: string; element: Element | null; error: string } | null>(null)
  const deferred = useDeferredValue(spec)
  const renderKey = JSON.stringify(spec)
  const categories = [...new Map(catalog.presets.map((preset) => [preset.category, preset.category_name])).entries()]
  const presets = catalog.presets.filter((preset) => query ? `${preset.name} ${preset.category_name} ${preset.category}`.toLowerCase().includes(query.toLowerCase()) : preset.category === category)
  useEffect(() => {
    let obsolete = false
    const key = JSON.stringify(deferred)
    previewQueue = previewQueue.then(async () => {
      if (obsolete) return
      try {
        const element = await core<Element>({ op: 'create_part', id: 'preview', spec: deferred, theme })
        if (!obsolete) setRender({ key, element, error: '' })
      } catch (reason) { if (!obsolete) setRender({ key, element: null, error: String(reason) }) }
    })
    return () => { obsolete = true }
  }, [deferred, theme])
  function choose(preset: PartPreset, keepData = true) {
    if (preset.category !== category) {
      drafts.current.set(category, { spec: structuredClone(spec), raw, mode })
      const saved = drafts.current.get(preset.category)
      if (saved) { setSpec(structuredClone(saved.spec)); setRaw(saved.raw); setInputMode(saved.mode); setCategory(preset.category); setQuery(''); setError(''); return }
    }
    const currentExample = catalog.presets.find((preset) => preset.id === spec.preset)?.example
    const unchangedExample = JSON.stringify(currentExample?.data) === JSON.stringify(spec.data)
    const keepRaw = keepData && preset.category === category && mode === 'json' && raw !== JSON.stringify(spec.data, null, 2)
    const next = keepData && preset.category === category ? { ...spec, preset: preset.id, data: unchangedExample && !keepRaw ? structuredClone(preset.example.data) : spec.data } : structuredClone(preset.example)
    setSpec(next); if (!keepRaw) setRaw(JSON.stringify(next.data, null, 2)); setCategory(preset.category); setQuery(''); setError('')
  }
  async function submit() {
    if (applying.current) return
    applying.current = true; setBusy(true); onBusy(true); setError('')
    try {
      await previewQueue
      const next = mode === 'json' ? { ...spec, data: JSON.parse(raw) } : spec
      await onApply(next)
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { applying.current = false; setBusy(false); onBusy(false) }
  }
  function setMode(next: 'fields' | 'json') {
    if (next === mode) return
    if (next === 'fields' && mode === 'json') { void previewJson(true); return }
    if (next === 'json') setRaw(JSON.stringify(spec.data, null, 2))
    setInputMode(next)
  }
  async function previewJson(showFields = false) {
    if (applying.current) return
    applying.current = true; setBusy(true); onBusy(true); setError('')
    try {
      await previewQueue
      const next = { ...spec, data: JSON.parse(raw) }
      const element = await core<Element>({ op: 'create_part', id: 'preview', spec: next, theme })
      setSpec(next); setRender({ key: JSON.stringify(next), element, error: '' })
      if (showFields) setInputMode('fields')
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { applying.current = false; setBusy(false); onBusy(false) }
  }
  return <div className="parts-panel">
    <aside className="parts-library"><label className="part-search"><Search size={16} /><input aria-label="Search parts" value={query} disabled={busy} onChange={(event) => setQuery(event.target.value)} placeholder="Search parts" /></label><label className="field">Category<select aria-label="Part category" value={category} disabled={busy} onChange={(event) => { const preset = catalog.presets.find((preset) => preset.category === event.target.value); if (preset) choose(preset, false) }}>{categories.map(([id, name]) => <option key={id} value={id}>{name}</option>)}</select></label><div className="parts-count">{catalog.presets.length} presets / {categories.length} categories</div><div className="parts-presets">{presets.map((preset) => <button key={preset.id} type="button" className={preset.id === spec.preset ? 'part-preset active' : 'part-preset'} aria-label={preset.name} aria-pressed={preset.id === spec.preset} disabled={busy} onClick={() => choose(preset)}><span>{preset.family} / {preset.category_name}</span><strong>{preset.name}</strong><small>{preset.id.split('/')[1]}</small></button>)}{!presets.length && <p role="status">No matching parts</p>}</div></aside>
    <form className="part-editor" onSubmit={(event) => { event.preventDefault(); void submit() }}>
      <Preview element={render?.key === renderKey ? render.element : null} theme={theme} />
      <fieldset className="part-editor-fields" disabled={busy}><div className="part-title-fields"><label className="field">Title<input aria-label="Part title" maxLength={80} value={spec.title} onChange={(event) => setSpec({ ...spec, title: event.target.value })} /></label><label className="field">Subtitle<input aria-label="Part subtitle" maxLength={120} value={spec.subtitle ?? ''} onChange={(event) => setSpec({ ...spec, subtitle: event.target.value })} /></label></div><div className="panel-tabs" role="tablist" aria-label="Part input mode"><button type="button" role="tab" aria-selected={mode === 'fields'} onClick={() => setMode('fields')}><List size={16} />Data</button><button type="button" role="tab" aria-selected={mode === 'json'} onClick={() => setMode('json')}><Code2 size={16} />JSON</button></div>{mode === 'fields' ? <Fields label="Data" value={spec.data as unknown as FieldValue} onChange={(value) => { setSpec({ ...spec, data: value as PartSpec['data'] }); setError('') }} /> : <label className="field">Metadata JSON<textarea aria-label="Part metadata JSON" className="code-input" maxLength={100000} rows={12} value={raw} onChange={(event) => setRaw(event.target.value)} /></label>}</fieldset>
      {(error || (render?.key === renderKey && render.error)) && <p className="error" role="alert">{error || render?.error}</p>}
      <footer className="part-actions"><span>{catalog.presets.find((preset) => preset.id === spec.preset)?.name}</span><button type="button" className="tool" aria-label="Reset part example" title="Reset part example" disabled={busy} onClick={() => { const preset = catalog.presets.find((preset) => preset.id === spec.preset); if (preset) choose(preset, false) }}><RotateCcw size={16} /></button>{mode === 'json' && <button type="button" className="secondary" disabled={busy} onClick={() => void previewJson()}><Code2 size={16} />Preview JSON</button>}<button type="submit" className="primary" disabled={busy || (mode === 'fields' && (render?.key !== renderKey || !render.element))}>{instance ? <Check size={16} /> : <Plus size={16} />}{instance ? 'Update part' : 'Insert part'}</button></footer>
    </form>
  </div>
}