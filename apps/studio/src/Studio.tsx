import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { Presentation, FileJson2, FolderOpen, Download, Undo2, Redo2, Copy, Trash2, Type, Square, ChevronLeft, ChevronRight, X, Check, Layers, FileText, Table2, AlertCircle, PanelRight, ArrowUp, ArrowDown, Save, Sparkles } from 'lucide-react'
import { core, decodeBase64, downloadBytes, fileBase64 } from './api'
import { SlideSurface } from './SlideSurface'
import { GenerationPanel } from './GenerationPanel'
import type { Deck, Element, Compiled, Exported, Report, Inspection, ProviderStatus } from './types'

let initial: Promise<{ report: Report; compiled: Compiled }> | undefined
function loadInitial() {
  initial ??= core<Report>({ op: 'sample' }).then(async (report) => ({ report, compiled: await core<Compiled>({ op: 'compile', report }) }))
  return initial
}

function Tool({ label, children, onClick, disabled = false }: { label: string; children: ReactNode; onClick: () => void; disabled?: boolean }) {
  return <button type="button" className="tool" title={label} aria-label={label} onClick={onClick} disabled={disabled}>{children}<span className="tooltip">{label}</span></button>
}

function Modal({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null)
  useEffect(() => {
    const previous = document.activeElement
    dialog.current?.showModal()
    return () => { if (previous instanceof HTMLElement) previous.focus() }
  }, [])
  return <dialog ref={dialog} className="modal" aria-label={title} onCancel={(event) => { event.preventDefault(); onClose() }}>
    <header><h2>{title}</h2><Tool label="Close dialog" onClick={onClose}><X size={18} /></Tool></header>
    {children}
  </dialog>
}

function Properties({ element, busy, onApply }: { element: Element; busy: boolean; onApply: (value: Element) => void }) {
  const [draft, setDraft] = useState<Element>(structuredClone(element))
  const [rows, setRows] = useState(element.type === 'table' ? JSON.stringify(element.rows, null, 2) : '')
  const [error, setError] = useState('')
  return <form className="properties" onSubmit={(event) => {
    event.preventDefault()
    try { onApply(draft.type === 'table' ? { ...draft, rows: JSON.parse(rows) } : draft); setError('') }
    catch { setError('Table data must be a JSON array of rows') }
  }}>
    <div className="section-label">POSITION & SIZE <span>px</span></div>
    <div className="geometry-inputs">{(['x', 'y', 'width', 'height'] as const).map((field) => <label key={field}><span>{field.toUpperCase()}</span><input aria-label={`Element ${field}`} type="number" required step="1" min={field === 'width' || field === 'height' ? 1 : 0} max={field === 'y' || field === 'height' ? 720 : 1280} value={draft[field]} onChange={(event) => setDraft({ ...draft, [field]: event.currentTarget.valueAsNumber })} /></label>)}</div>
    {draft.type === 'text' && <>
      <label className="field">Text content<textarea aria-label="Text content" rows={5} maxLength={4000} value={draft.text} onChange={(event) => setDraft({ ...draft, text: event.target.value })} /></label>
      <div className="inline-fields"><label className="field">Size<input type="number" min={8} max={120} required value={draft.font_size} onChange={(event) => setDraft({ ...draft, font_size: event.currentTarget.valueAsNumber })} /></label><label className="field">Color<input type="color" value={`#${draft.color}`} onChange={(event) => setDraft({ ...draft, color: event.target.value.slice(1) })} /></label></div>
      <label className="checkbox"><input type="checkbox" checked={draft.bold} onChange={(event) => setDraft({ ...draft, bold: event.target.checked })} />Bold</label>
    </>}
    {draft.type === 'rect' && <label className="field">Fill<input type="color" value={`#${draft.fill}`} onChange={(event) => setDraft({ ...draft, fill: event.target.value.slice(1) })} /></label>}
    {draft.type === 'table' && <label className="field">Table rows<textarea aria-label="Table rows" className="code-input" rows={9} value={rows} onChange={(event) => setRows(event.target.value)} /></label>}
    {error && <p role="alert" className="error">{error}</p>}
    <button type="submit" className="apply-button" disabled={busy}><Check size={16} />Apply changes</button>
  </form>
}

export default function Studio() {
  const [history, setHistory] = useState<{ past: Deck[]; present: Deck; future: Deck[] } | null>(null)
  const [report, setReport] = useState<Report | null>(null)
  const [slideIndex, setSlideIndex] = useState(0)
  const [selected, setSelected] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [status, setStatus] = useState('Loading local core')
  const [modal, setModal] = useState<'report' | 'inspect' | 'generate' | null>(null)
  const [provider, setProvider] = useState<ProviderStatus | null>(null)
  const [reportJson, setReportJson] = useState('')
  const [inspection, setInspection] = useState<Inspection | null>(null)
  const [imported, setImported] = useState('')
  const [importedSlide, setImportedSlide] = useState(0)
  const [importedRun, setImportedRun] = useState(0)
  const [patch, setPatch] = useState('')
  const [panel, setPanel] = useState<'elements' | 'notes'>('elements')
  const [zoom, setZoom] = useState('fit')
  const [showInspector, setShowInspector] = useState(true)
  const pptxInput = useRef<HTMLInputElement>(null)
  const sceneInput = useRef<HTMLInputElement>(null)
  const activeOperation = useRef(false)
  const deck = history?.present
  const slide = deck?.slides[slideIndex]
  const element = slide?.elements.find((item) => item.id === selected)
  const inspectedSlide = inspection?.slides[importedSlide]
  const inspectedText = inspectedSlide?.texts[importedRun]

  useEffect(() => {
    let disposed = false
    loadInitial().then(({ report: input, compiled }) => {
      if (disposed) return
      setReport(input)
      setHistory({ past: [], present: compiled.deck, future: [] })
      setStatus('Synthetic sample / Local core')
    }).catch((reason) => { if (!disposed) setError(String(reason)) })
    return () => { disposed = true }
  }, [])

  function commit(next: Deck) {
    setHistory((previous) => previous ? { past: [...previous.past.slice(-29), previous.present], present: next, future: [] } : { past: [], present: next, future: [] })
    setStatus('Unsaved changes')
  }
  function undo() {
    setHistory((previous) => {
      if (!previous?.past.length) return previous
      return { past: previous.past.slice(0, -1), present: previous.past.at(-1)!, future: [previous.present, ...previous.future] }
    })
    setSelected(null)
    setSlideIndex(0)
  }
  function redo() {
    setHistory((previous) => {
      if (!previous?.future.length) return previous
      return { past: [...previous.past, previous.present], present: previous.future[0], future: previous.future.slice(1) }
    })
    setSelected(null)
    setSlideIndex(0)
  }
  async function run(action: () => Promise<void>) {
    if (activeOperation.current || busy) return
    activeOperation.current = true
    setBusy(true)
    setError('')
    try { await action() } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { activeOperation.current = false; setBusy(false) }
  }
  async function apply(next: Deck) {
    await core({ op: 'validate', deck: next })
    commit(next)
  }
  function replaceElement(next: Element) {
    if (!deck || !slide) return
    void run(() => apply({ ...deck, slides: deck.slides.map((item) => item.id === slide.id ? { ...item, elements: item.elements.map((old) => old.id === next.id ? next : old) } : item) }))
  }
  function changeSlide(index: number) { setSlideIndex(index); setSelected(null) }
  function addElement(type: 'text' | 'rect') {
    if (!deck || !slide) return
    const id = `${type}-${crypto.randomUUID().slice(0, 8)}`
    const common = { id, x: 100, y: 220, width: 360, height: 110 }
    const next: Element = type === 'text' ? { ...common, type, text: 'New text', font_size: 32, color: '202525', bold: false } : { ...common, type, fill: 'E3EFEB' }
    void run(async () => { await apply({ ...deck, slides: deck.slides.map((item) => item.id === slide.id ? { ...item, elements: [...item.elements, next] } : item) }); setSelected(id) })
  }
  const disable = busy || !deck
  const closeModal = () => { if (!busy) { setModal(null); setError('') } }

  return <div className="studio">
    <header className="app-header">
      <div className="brand"><span className="brand-mark"><Presentation size={23} /></span><h1>AISlide</h1><span className="version">STUDIO / 01</span></div>
      <div className="document-name"><strong>{deck?.title ?? 'Untitled report'}</strong><span>Data report</span></div>
      <div className="header-actions">
        <button className="secondary" aria-label="Generate with AI" disabled={disable} onClick={() => void run(async () => { setProvider(await core<ProviderStatus>({ op: 'provider_status' })); setModal('generate') })}><Sparkles size={17} /><span>Generate</span></button>
        <button className="secondary" aria-label="Report data" disabled={disable} onClick={() => { setReportJson(JSON.stringify(report, null, 2)); setError(''); setModal('report') }}><FileJson2 size={17} /><span>Report data</span></button>
        <Tool label="Inspect PPTX" disabled={disable} onClick={() => pptxInput.current?.click()}><FolderOpen size={19} /></Tool>
        <button className="primary" aria-label="Export PPTX" disabled={disable} onClick={() => void run(async () => {
          const result = await core<Exported>({ op: 'export', deck })
          const saved = await downloadBytes(decodeBase64(result.base64), result.filename, 'application/vnd.openxmlformats-officedocument.presentationml.presentation')
          setStatus(saved ? 'PPTX exported' : 'Export cancelled')
        })}><Download size={17} /><span>Export .pptx</span></button>
      </div>
      <input ref={pptxInput} type="file" accept=".pptx" hidden aria-label="Open PPTX file" onChange={(event) => {
        const file = event.target.files?.[0]
        event.target.value = ''
        if (!file) return
        void run(async () => {
          const base64 = await fileBase64(file)
          const result = await core<Inspection>({ op: 'inspect', base64 })
          setImported(base64); setInspection(result); setImportedSlide(0); setImportedRun(0); setPatch(result.slides[0]?.texts[0]?.text ?? ''); setModal('inspect')
        })
      }} />
    </header>

    <div className="ribbon">
      <div className="ribbon-group"><Tool label="Undo" disabled={disable || !history?.past.length} onClick={undo}><Undo2 size={18} /></Tool><Tool label="Redo" disabled={disable || !history?.future.length} onClick={redo}><Redo2 size={18} /></Tool></div>
      <div className="ribbon-group"><Tool label="Add text" disabled={disable} onClick={() => addElement('text')}><Type size={19} /></Tool><Tool label="Add rectangle" disabled={disable} onClick={() => addElement('rect')}><Square size={18} /></Tool></div>
      <div className="ribbon-group"><Tool label="Duplicate slide" disabled={disable || !slide} onClick={() => {
        if (!deck || !slide) return
        const copy = { ...structuredClone(slide), id: `slide-${crypto.randomUUID().slice(0, 8)}` }
        void run(async () => { await apply({ ...deck, slides: [...deck.slides.slice(0, slideIndex + 1), copy, ...deck.slides.slice(slideIndex + 1)] }); changeSlide(slideIndex + 1) })
      }}><Copy size={18} /></Tool><Tool label="Delete element" disabled={disable || !selected} onClick={() => {
        if (!deck || !slide) return
        void run(async () => { await apply({ ...deck, slides: deck.slides.map((item) => item.id === slide.id ? { ...item, elements: item.elements.filter((old) => old.id !== selected) } : item) }); setSelected(null) })
      }}><Trash2 size={18} /></Tool></div>
      <span className="ribbon-space" />
      <Tool label="Open scene JSON" disabled={disable} onClick={() => sceneInput.current?.click()}><FolderOpen size={18} /></Tool>
      <input ref={sceneInput} hidden type="file" accept=".json" aria-label="Open scene file" onChange={(event) => {
        const file = event.target.files?.[0]
        event.target.value = ''
        if (!file) return
        void run(async () => {
          if (file.size > 3 * 1024 * 1024) throw new Error('Scene JSON must be smaller than 3 MiB')
          const next = JSON.parse((await file.text()).replace(/^\uFEFF/, '')) as Deck
          await apply(next)
          changeSlide(0)
          setStatus('Scene checkpoint restored')
        })
      }} />
      <Tool label="Save scene JSON" disabled={disable} onClick={() => void run(async () => { await downloadBytes(new TextEncoder().encode(JSON.stringify(deck, null, 2)), 'report.scene.json', 'application/json'); setStatus('Scene snapshot exported') })}><Save size={18} /></Tool>
      <select aria-label="Zoom" className="zoom-select" value={zoom} onChange={(event) => setZoom(event.target.value)}><option value="fit">Fit</option><option value="0.75">75%</option><option value="1">100%</option></select>
      <Tool label="Toggle inspector" onClick={() => setShowInspector(!showInspector)}><PanelRight size={18} /></Tool>
    </div>

    {error && !modal && <div className="error-strip" role="alert"><AlertCircle size={17} />{error}<button aria-label="Dismiss error" onClick={() => setError('')}><X size={16} /></button></div>}

    <div className={`workbench ${showInspector ? '' : 'inspector-hidden'}`}>
      <aside className="slide-list" aria-label="Slides">
        <div className="panel-heading"><h2>Slides</h2><span>{deck?.slides.length ?? 0}</span></div>
        <div className="thumbnails">{deck?.slides.map((item, index) => <button key={item.id} className={`thumbnail ${index === slideIndex ? 'active' : ''}`} aria-label={`Slide ${index + 1}: ${item.title.replaceAll('\n', ' ')}`} aria-current={index === slideIndex ? 'true' : undefined} onClick={() => changeSlide(index)}>
          <div className="thumbnail-image"><SlideSurface slide={item} /></div><div className="thumbnail-caption"><span>{String(index + 1).padStart(2, '0')}</span><strong>{item.title.split('\n')[0]}</strong></div>
        </button>)}</div>
      </aside>

      <main className="canvas-workspace">
        <div className="canvas-heading"><span>{String(slideIndex + 1).padStart(2, '0')} <span className="slash">/</span> {String(deck?.slides.length ?? 0).padStart(2, '0')}</span><h2>{slide?.title.split('\n')[0] ?? 'Opening report'}</h2><span className="native-badge">NATIVE OBJECTS</span></div>
        <div className="canvas-scroll">
          <div className="slide-stage" style={{ width: zoom === 'fit' ? '100%' : `${1280 * Number(zoom)}px` }}>
            {slide ? <SlideSurface slide={slide} selected={selected} onSelect={setSelected} onMove={(id, x, y) => {
              const original = slide.elements.find((item) => item.id === id)
              if (original && !busy) replaceElement({ ...original, x, y })
            }} /> : <div className="loading-state">{error ? 'Core unavailable' : 'Opening local report...'}</div>}
          </div>
        </div>
        <div className="canvas-footer"><span>{selected ? `${element?.type ?? ''} / ${selected}` : 'No selection'}</span><div><Tool label="Previous slide" disabled={slideIndex === 0} onClick={() => changeSlide(slideIndex - 1)}><ChevronLeft size={17} /></Tool><Tool label="Next slide" disabled={!deck || slideIndex === deck.slides.length - 1} onClick={() => changeSlide(slideIndex + 1)}><ChevronRight size={17} /></Tool></div></div>
        <div className="notes-preview"><FileText size={16} /><span>{slide?.notes.split('\n')[0]}</span></div>
      </main>

      {showInspector && <aside className="inspector" aria-label="Inspector">
        <div className="panel-tabs" role="tablist" aria-label="Inspector views"><button role="tab" aria-selected={panel === 'elements'} onClick={() => setPanel('elements')}><Layers size={16} />Elements</button><button role="tab" aria-selected={panel === 'notes'} onClick={() => setPanel('notes')}><FileText size={16} />Notes</button></div>
        {panel === 'elements' ? <>
          <div className="layer-list">{slide?.elements.map((item) => <button key={item.id} className={selected === item.id ? 'active' : ''} aria-label={`Select ${item.id}`} onClick={() => setSelected(item.id)}>{item.type === 'text' ? <Type size={15} /> : item.type === 'table' ? <Table2 size={15} /> : <Square size={15} />}<span>{item.id}</span></button>)}</div>
          {element ? <><div className="selection-heading"><strong>{element.id}</strong><span>{element.type}</span></div><Properties key={JSON.stringify(element)} element={element} busy={busy} onApply={replaceElement} /></> : <div className="empty-properties"><Layers size={24} /><p>No element selected</p></div>}
          <div className="slide-order"><span>Slide order</span><Tool label="Move slide up" disabled={disable || slideIndex === 0} onClick={() => {
            if (!deck) return
            const slides = [...deck.slides]; [slides[slideIndex - 1], slides[slideIndex]] = [slides[slideIndex], slides[slideIndex - 1]]
            commit({ ...deck, slides }); changeSlide(slideIndex - 1)
          }}><ArrowUp size={16} /></Tool><Tool label="Move slide down" disabled={disable || !deck || slideIndex >= deck.slides.length - 1} onClick={() => {
            if (!deck) return
            const slides = [...deck.slides]; [slides[slideIndex + 1], slides[slideIndex]] = [slides[slideIndex], slides[slideIndex + 1]]
            commit({ ...deck, slides }); changeSlide(slideIndex + 1)
          }}><ArrowDown size={16} /></Tool></div>
        </> : <div className="notes-panel"><h3>Speaker notes & sources</h3><p>{slide?.notes}</p></div>}
        <div className="validation-note"><AlertCircle size={16} /><span>Office text layout unverified</span></div>
      </aside>}
    </div>
    <footer className="status-bar"><span className={busy ? 'status busy' : 'status'}>{busy ? 'Processing' : status}</span><span>{deck?.slides.length ?? 0} slides<span className="status-divider">|</span>16:9<span className="status-divider">|</span>Core 0.1</span></footer>

    {modal === 'generate' && provider && <Modal title="Generate report" onClose={closeModal}>
      <GenerationPanel provider={provider} onBusy={setBusy} onApply={(draft) => {
        setReport(draft.report)
        commit(draft.compiled.deck)
        changeSlide(0)
        setStatus('AI draft / Content unverified')
        setModal(null)
      }} />
    </Modal>}

    {modal === 'report' && <Modal title="Report data" onClose={closeModal}>
      <label className="field report-field">Report JSON<textarea aria-label="Report JSON" className="code-input" value={reportJson} onChange={(event) => setReportJson(event.target.value)} rows={19} spellCheck={false} /></label>
      {error && <p className="error modal-error" role="alert">{error}</p>}
      <div className="modal-actions"><label className="secondary upload-label"><FolderOpen size={16} />Open JSON<input type="file" accept=".json" onChange={(event) => { const file = event.target.files?.[0]; if (file) void run(async () => { if (file.size > 2 * 1024 * 1024) throw new Error('JSON must be smaller than 2 MiB'); setReportJson((await file.text()).replace(/^\uFEFF/, '')) }) }} /></label><button className="primary" disabled={busy} onClick={() => void run(async () => { const input = JSON.parse(reportJson); const result = await core<Compiled>({ op: 'compile', report: input }); setReport(input); commit(result.deck); changeSlide(0); setModal(null) })}><Check size={16} />Compile report</button></div>
    </Modal>}

    {modal === 'inspect' && <Modal title="PPTX text inspector" onClose={closeModal}>
      <div className="import-fields"><label className="field">Slide<select value={importedSlide} onChange={(event) => { const index = Number(event.target.value); setImportedSlide(index); setImportedRun(0); setPatch(inspection?.slides[index]?.texts[0]?.text ?? '') }}>{inspection?.slides.map((item, index) => <option value={index} key={item.part}>{index + 1}: {item.part}</option>)}</select></label>
        <label className="field">Text run<select value={importedRun} onChange={(event) => { const index = Number(event.target.value); setImportedRun(index); setPatch(inspectedSlide?.texts[index]?.text ?? '') }}>{inspectedSlide?.texts.map((item, index) => <option value={index} key={`${item.shape_id}-${item.run_index}`}>{item.shape_id}:{item.run_index} / {item.text.slice(0, 70)}</option>)}</select></label>
        <label className="field">Replacement text<textarea aria-label="Replacement text" rows={5} maxLength={4000} value={patch} onChange={(event) => setPatch(event.target.value)} /></label>
        <p className="warning">Limited to simple top-level text runs. Font coverage and visual fit are unverified.</p>
      </div>
      {error && <p className="error modal-error" role="alert">{error}</p>}
      <div className="modal-actions"><span>Original file remains unchanged</span><button className="primary" disabled={busy || !inspectedText} onClick={() => void run(async () => { if (!inspectedSlide || !inspectedText) return; const result = await core<Exported>({ op: 'patch_text', base64: imported, part: inspectedSlide.part, shape_id: inspectedText.shape_id, run_index: inspectedText.run_index, expected: inspectedText.text, text: patch }); await downloadBytes(decodeBase64(result.base64), 'patched.pptx', 'application/vnd.openxmlformats-officedocument.presentationml.presentation') })}><Download size={16} />Export patch</button></div>
    </Modal>}
  </div>
}