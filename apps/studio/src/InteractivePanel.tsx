import { useEffect, useRef, useState } from 'react'
import { core } from './api'
import { Content } from './SlideSurface'
import type { InteractivePreview, InteractiveSpec, InteractiveState, PartInstance, PartPreset, PartSpec, Theme } from './types'
import './interactive.css'

let previewQueue: Promise<void> = Promise.resolve()

export function InteractivePanel({ presets, instance, theme, onApply, onBusy }: {
  presets: PartPreset[]; instance?: PartInstance | null; theme?: Theme
  onApply: (spec: PartSpec) => Promise<void>; onBusy: (busy: boolean) => void
}) {
  const first = instance?.spec ?? presets[0]?.example
  if (!first || first.data.kind !== 'interactive') throw new Error('This core build does not provide the requested interactive part')
  const initialData = first.data
  const [spec, setSpec] = useState<PartSpec & { data: { kind: 'interactive'; spec: InteractiveSpec } }>(() => ({ ...first, data: structuredClone(initialData) }))
  const [state, setState] = useState<InteractiveState>(() => structuredClone(initialData.spec.initial_state))
  const [raw, setRaw] = useState(JSON.stringify(first, null, 2))
  const [editing, setEditing] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [render, setRender] = useState<{ key: string; result: InteractivePreview } | null>(null)
  const applying = useRef(false)
  const host = useRef<HTMLDivElement>(null)
  const [previewWidth, setPreviewWidth] = useState(800)
  const key = JSON.stringify({ spec, state })
  useEffect(() => {
    const element = host.current
    if (!element) return
    const observer = new ResizeObserver(([entry]) => setPreviewWidth(entry.contentRect.width))
    observer.observe(element)
    return () => observer.disconnect()
  }, [editing])
  useEffect(() => {
    let obsolete = false
    onBusy(true)
    const current = JSON.stringify({ spec, state })
    previewQueue = previewQueue.then(async () => {
      if (obsolete) return
      try {
        const result = await core<InteractivePreview>({ op: 'preview_interactive', id: 'interactive-preview', spec, state, theme })
        if (!obsolete) { setRender({ key: current, result }); setError('') }
      } catch (reason) { if (!obsolete) setError(reason instanceof Error ? reason.message : String(reason)) }
      finally { if (!obsolete && !applying.current) onBusy(false) }
    })
    return () => { obsolete = true }
  }, [spec, state, theme, onBusy])
  const input = spec.data.spec
  const visible = input.items.filter(item => !state.group_id || item.group_id === state.group_id)
  const selected = input.items.find(item => item.id === state.selected_id)
  function choose(preset: PartPreset) {
    if (preset.example.data.kind !== 'interactive') return
    const next = { ...structuredClone(preset.example), data: { kind: 'interactive' as const, spec: structuredClone(preset.example.data.spec) } }
    setSpec(next); setRaw(JSON.stringify(next, null, 2)); setState(next.data.spec.initial_state); setError('')
  }
  async function validateData() {
    if (applying.current) return
    applying.current = true; setBusy(true); onBusy(true); setError('')
    try {
      await previewQueue
      const candidate: PartSpec = JSON.parse(raw)
      const result = await core<InteractivePreview>({ op: 'preview_interactive', id: 'interactive-preview', spec: candidate, theme })
      if (candidate.data.kind !== 'interactive') throw new Error('Data must be an interactive part')
      setSpec({ ...candidate, data: candidate.data }); setState(result.state); setEditing(false)
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { applying.current = false; setBusy(false); onBusy(false) }
  }
  async function save() {
    if (applying.current || render?.key !== key) return
    applying.current = true; setBusy(true); onBusy(true); setError('')
    try {
      await previewQueue
      await onApply({ ...spec, data: { kind: 'interactive', spec: { ...input, initial_state: state } } })
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { applying.current = false; setBusy(false); onBusy(false) }
  }
  const active = render?.key === key
  return <section className="interactive-panel" aria-label="Interactive part editor">
    <p>Prototype: explore locally. Save explicitly to set the default view. PowerPoint, PDF and print receive editable static content, not interactive HTML.</p>
    {error && <p role="alert" className="error-panel">{error}</p>}
    {!instance && <label className="field">Template<select aria-label="Interactive template" value={spec.preset} disabled={busy || editing} onChange={event => { const preset = presets.find(p => p.id === event.target.value); if (preset) choose(preset) }}>{presets.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>}
    <div className="interactive-actions"><button className="secondary" disabled={busy} onClick={() => { setRaw(JSON.stringify(spec, null, 2)); setEditing(true) }}>Edit interactive data</button>
      <button className="secondary" disabled={busy || editing} onClick={() => setState(structuredClone(input.initial_state))}>Reset preview</button></div>
    {editing ? <div><label className="field">Typed data JSON<textarea aria-label="Interactive data JSON" rows={16} value={raw} disabled={busy} onChange={event => setRaw(event.target.value)} /></label>
      <button className="primary" disabled={busy} onClick={() => void validateData()}>Validate data</button>
      <button className="secondary" disabled={busy} onClick={() => { setEditing(false); setError('') }}>Cancel data changes</button></div> : <>
      <div className="interactive-controls">
        {(input.groups?.length ?? 0) > 0 && <label className="field">Group<select aria-label="Interactive group" disabled={busy} value={state.group_id ?? ''} onChange={event => {
          const group_id = event.target.value || undefined
          const first = input.items.find(item => !group_id || item.group_id === group_id)
          if (first) setState({ ...state, group_id, selected_id: first.id })
        }}><option value="">All groups</option>{input.groups?.map(group => <option key={group.id} value={group.id} disabled={!input.items.some(item => item.group_id === group.id)}>{group.label}</option>)}</select></label>}
        {input.template === 'dashboard' && <label className="field">Scale<select aria-label="Interactive scale" disabled={busy} value={state.scale ?? 'linear'} onChange={event => setState({ ...state, scale: event.target.value === 'logarithmic' ? 'logarithmic' : 'linear' })}><option value="linear">Linear (zero baseline)</option><option value="logarithmic">Log (1 + value)</option></select></label>}
        {input.template === 'comparison' && <label className="field">Dimension<select aria-label="Interactive dimension" disabled={busy} value={state.dimension ?? 0} onChange={event => setState({ ...state, dimension: Number(event.target.value) })}>{input.dimensions?.map((dimension, index) => <option key={dimension} value={index}>{dimension}</option>)}</select></label>}
      </div>
      <div className="interactive-items" role="group" aria-label={input.template === 'timeline' ? 'Stages' : 'Items'}>
        {input.template === 'timeline' && <button className="secondary" disabled={busy || visible[0]?.id === state.selected_id} onClick={() => { const i = visible.findIndex(item => item.id === state.selected_id); if (i > 0) setState({ ...state, selected_id: visible[i - 1].id }) }}>Previous stage</button>}
        {visible.map((item, index) => <button key={item.id} className="secondary" disabled={busy} aria-pressed={item.id === state.selected_id} onClick={() => setState({ ...state, selected_id: item.id })}>{input.template === 'timeline' ? `${index + 1}. ` : ''}{item.label}</button>)}
        {input.template === 'timeline' && <button className="secondary" disabled={busy || visible.at(-1)?.id === state.selected_id} onClick={() => { const i = visible.findIndex(item => item.id === state.selected_id); if (i < visible.length - 1) setState({ ...state, selected_id: visible[i + 1].id }) }}>Next stage</button>}
      </div>
      <p role="status" aria-live="polite">{active ? `Selected: ${selected?.label ?? ''}` : 'Rendering interactive view'}</p>
      <div ref={host} className="interactive-preview" aria-label="Interactive view preview" aria-busy={!active} style={{ aspectRatio: render ? `${render.result.element.width} / ${render.result.element.height}` : '1152 / 512' }}>
        {render && <div style={{ width: render.result.element.width, height: render.result.element.height, transform: `scale(${previewWidth / render.result.element.width})`, transformOrigin: 'top left' }}><Content element={render.result.element} theme={theme} /></div>}
      </div>
      <p className="interactive-source">Source: {input.source}</p>
      <button className="primary" disabled={busy || !active} onClick={() => void save()}>{instance ? 'Save default and update part' : 'Save default and insert part'}</button>
    </>}
  </section>
}
