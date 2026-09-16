import { lazy, Suspense, useCallback, useEffect, useEffectEvent, useRef, useState } from 'react'
import type { ReactNode, MouseEvent as ReactMouseEvent, KeyboardEvent as ReactKeyboardEvent } from 'react'
import { Presentation, FileJson2, FolderOpen, Download, Undo2, Redo2, Copy, Trash2, Type, Square, ChevronLeft, ChevronRight, X, Check, Layers, FileText, Table2, AlertCircle, PanelRight, ArrowUp, ArrowDown, Save, Sparkles, ChartNoAxesCombined, ImagePlus, Workflow, Replace, Database, FileInput, ScanLine, Shapes, Palette, LayoutTemplate, RotateCcw } from 'lucide-react'
import { core, decodeBase64, downloadBytes, downloadPresentation, fileBase64, fileAssets, svgAsset } from './api'
import { SlideSurface } from './SlideSurface'
import { GenerationPanel } from './GenerationPanel'
import { SourcePanel } from './SourcePanel'
import { InsertPanel } from './InsertPanel'
import { PartsPanel } from './PartsPanel'
import type { AssetInput } from './types'
import { ContextMenu } from './ContextMenu'
import type { MenuPosition, MenuCommand } from './ContextMenu'
import { FilePlus2, FileDown, Plus, Pencil, ArrowUpToLine, ArrowDownToLine, Menu, Sticker, Network } from 'lucide-react'
import type { SlideOperation, ElementOperation } from './types'
import './workspace.css'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { CloseRequestedEvent } from '@tauri-apps/api/window'
import { Blocks } from 'lucide-react'
import type { PartCatalog, PartInstance, GraphCatalog } from './types'
import { ThemePanel } from './ThemePanel'
import { DesignPanel } from './DesignPanel'
import { ColorField, TextControls } from './TextControls'
import { Tool } from './Tool'
import { chartNames } from './design'
import { AislideClient, DocumentSession } from '../../../packages/client/index.mjs'
import type { ReplaceOptions } from '../../../packages/client/index.mjs'
import type { AislideDocument, Deck, Element, Compiled, Exported, Report, Inspection, ProviderStatus, LayoutReport, Design, ObjectCatalog, Theme } from './types'

const client = new AislideClient(core)
const AssetPanel = lazy(() => import('./AssetPanel').then((module) => ({ default: module.AssetPanel })))
const GraphEditor = lazy(() => import('./GraphEditor').then((module) => ({ default: module.GraphEditor })))
let initial: Promise<AislideDocument> | undefined
function loadInitial() {
  initial ??= client.createPresentation(crypto.randomUUID()).then((opened) => opened.document)
  return initial
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

function Properties({ element, theme, busy, onApply, onReplaceImage, onDraftChange }: { element: Element; theme?: Theme; busy: boolean; onApply: (value: Element) => Promise<void>; onReplaceImage: () => void; onDraftChange: (dirty: boolean) => void }) {
  const [draft, setDraft] = useState<Element>(structuredClone(element))
  const [rows, setRows] = useState(element.type === 'table' ? JSON.stringify(element.rows, null, 2) : '')
  const [chartData, setChartData] = useState(element.type === 'chart' ? JSON.stringify({ categories: element.categories, series: element.series }, null, 2) : '')
  const [error, setError] = useState('')
  const hasDraft = JSON.stringify(draft) !== JSON.stringify(element)
    || element.type === 'table' && rows !== JSON.stringify(element.rows, null, 2)
    || element.type === 'chart' && chartData !== JSON.stringify({ categories: element.categories, series: element.series }, null, 2)
  useEffect(() => { onDraftChange(hasDraft); return () => onDraftChange(false) }, [hasDraft, onDraftChange])
  async function submit() {
    setError('')
    try {
      if (draft.type === 'chart') {
        const data = JSON.parse(chartData)
        await onApply({ ...draft, categories: data.categories, series: data.series })
      } else await onApply(draft.type === 'table' ? { ...draft, rows: JSON.parse(rows) } : draft)
    } catch (reason) { setError(reason instanceof SyntaxError ? 'Data must be valid JSON with the required fields' : reason instanceof Error ? reason.message : String(reason)) }
  }
  return <form className="properties" onSubmit={(event) => { event.preventDefault(); void submit() }}>
    <fieldset className="properties-body" disabled={busy}>
    <div className="section-label">POSITION & SIZE <span>px</span></div>
    <div className="geometry-inputs">{(['x', 'y', 'width', 'height'] as const).map((field) => <label key={field}><span>{field.toUpperCase()}</span><input aria-label={`Element ${field}`} type="number" required step="1" min={field === 'width' || field === 'height' ? 1 : 0} max={field === 'y' || field === 'height' ? 720 : 1280} value={draft[field]} onChange={(event) => setDraft({ ...draft, [field]: event.currentTarget.valueAsNumber })} /></label>)}</div>
    {(draft.type === 'text' || draft.type === 'shape') && <TextControls element={draft} theme={theme} onChange={setDraft} />}
    {(draft.type === 'rect' || draft.type === 'shape') && <ColorField label="Fill" value={draft.fill} theme={theme} onChange={(fill) => setDraft({ ...draft, fill })} />}
    {draft.type === 'shape' && <><ColorField label="Outline" value={draft.stroke} theme={theme} onChange={(stroke) => setDraft({ ...draft, stroke })} /><label className="field">Outline width<input aria-label="Outline width" type="number" min={0} max={20} step={0.5} value={draft.stroke_width} onChange={(event) => setDraft({ ...draft, stroke_width: event.currentTarget.valueAsNumber })} /></label><label className="field">Rotation<input aria-label="Rotation" type="number" min={-360} max={360} value={draft.rotation} onChange={(event) => setDraft({ ...draft, rotation: event.currentTarget.valueAsNumber })} /></label></>}
    {draft.type === 'picture' && <>
      <label className="field">Alt text<textarea aria-label="Alt text" maxLength={500} value={draft.alt} onChange={(event) => setDraft({ ...draft, alt: event.target.value })} /></label>
      <div className="geometry-inputs">{(['left', 'top', 'right', 'bottom'] as const).map((edge) => <label key={edge}>Crop {edge} (%)<input aria-label={`Crop ${edge} (%)`} type="number" min={0} max={99} step={1} value={Math.round((draft.crop?.[edge] ?? 0) * 100)} onChange={(event) => setDraft({ ...draft, crop: { ...(draft.crop ?? { left: 0, right: 0, top: 0, bottom: 0 }), [edge]: event.currentTarget.valueAsNumber / 100 } })} /></label>)}</div>
      <button type="button" className="secondary" disabled={busy} onClick={onReplaceImage}><Replace size={16} />Replace picture</button>
    </>}
    {draft.type === 'group' && draft.children.filter((child) => child.type === 'text').map((child, index) => child.type === 'text' && <label className="field" key={child.id}>Step {index + 1}<textarea aria-label={`Step ${index + 1}`} maxLength={80} rows={3} value={child.text} onChange={(event) => setDraft({ ...draft, children: draft.children.map((entry) => entry.id === child.id ? { ...child, text: event.target.value } : entry) })} /></label>)}
    {draft.type === 'connector' && <>
      <ColorField label="Stroke" value={draft.color} theme={theme} onChange={(color) => setDraft({ ...draft, color })} />
      <label className="field">Line width<input type="number" min={0.5} max={20} step={0.5} value={draft.stroke_width} onChange={(event) => setDraft({ ...draft, stroke_width: event.currentTarget.valueAsNumber })} /></label>
      <label className="checkbox"><input type="checkbox" checked={draft.arrow} onChange={(event) => setDraft({ ...draft, arrow: event.target.checked })} />Arrow</label>
    </>}
    {draft.type === 'table' && <label className="field">Table rows<textarea aria-label="Table rows" className="code-input" rows={9} value={rows} onChange={(event) => setRows(event.target.value)} /></label>}
    {draft.type === 'chart' && <>
      <label className="field">Chart type<select aria-label="Chart type" value={draft.kind} onChange={(event) => setDraft({ ...draft, kind: event.target.value as typeof draft.kind })}>{Object.entries(chartNames).map(([kind, name]) => <option key={kind} value={kind}>{name}</option>)}</select></label>
      <label className="field">Chart data<textarea aria-label="Chart data" className="code-input" rows={12} value={chartData} onChange={(event) => setChartData(event.target.value)} /></label>
    </>}
    </fieldset>
    {error && <div role="alert" className="error property-error"><span>{error}</span><button type="button" className="tool" aria-label="Dismiss error" title="Dismiss error" onClick={() => setError('')}><X size={16} /></button></div>}
    <button type="submit" className="apply-button" disabled={busy}><Check size={16} />Apply changes</button>
  </form>
}

export default function Studio() {
  const [documentState, setDocumentSnapshot] = useState<AislideDocument | null>(null)
  const [historyState, setHistoryState] = useState({ undo: false, redo: false })
  const session = useRef<DocumentSession | null>(null)
  const [slideIndex, setSlideIndex] = useState(0)
  const [selected, setSelected] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [status, setStatus] = useState('Loading local core')
  const [modal, setModal] = useState<'report' | 'inspect' | 'generate' | 'sources' | 'layout' | 'import' | 'insert' | 'parts' | 'graph' | 'theme' | 'design' | 'assets' | null>(null)
  const [graphCatalog, setGraphCatalog] = useState<GraphCatalog | null>(null)
  const [partCatalog, setPartCatalog] = useState<PartCatalog | null>(null)
  const [partEditing, setPartEditing] = useState<PartInstance | null>(null)
  const [designDefaults, setDesignDefaults] = useState<Design | null>(null)
  const [catalog, setCatalog] = useState<ObjectCatalog | null>(null)
  const [layoutReport, setLayoutReport] = useState<LayoutReport | null>(null)
  const [importWarnings, setImportWarnings] = useState<string[]>([])
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
  const [filename, setFilename] = useState('Untitled presentation.pptx')
  const [savedHash, setSavedHash] = useState<string | null>(null)
  const [inlineDraft, setInlineDraft] = useState(false)
  const [propertiesDraft, setPropertiesDraft] = useState(false)
  const [pendingReplacement, setPendingReplacement] = useState<(() => Promise<void>) | null>(null)
  const [nameDialog, setNameDialog] = useState<{ kind: 'save' | 'presentation' | 'slide'; value: string; slideId?: string } | null>(null)
  const [context, setContext] = useState<{ position: MenuPosition; kind: 'file' | 'canvas' | 'slide' | 'element'; commands: MenuCommand[] } | null>(null)
  const [editRequest, setEditRequest] = useState<{ id: string; slideId: string; sequence: number } | undefined>()
  const [assetDrag, setAssetDrag] = useState(false)
  const closeContext = useCallback(() => setContext(null), [])
  const pptxInput = useRef<HTMLInputElement>(null)
  const nativeInput = useRef<HTMLInputElement>(null)
  const pictureInput = useRef<HTMLInputElement>(null)
  const pictureTarget = useRef<string | null>(null)
  const activeOperation = useRef(false)
  const deck = documentState?.deck
  const report = documentState?.report
  const slide = deck?.slides[slideIndex]
  const element = slide?.elements.find((item) => item.id === selected)
  const selectedPart = documentState?.parts?.find((part) => part.slide_id === slide?.id && part.element_id === selected)
  const inspectedSlide = inspection?.slides[importedSlide]
  const inspectedText = inspectedSlide?.texts[importedRun]
  const hasDrafts = inlineDraft || propertiesDraft
  const dirty = hasDrafts || Boolean(documentState && documentState.hash !== savedHash)
  function setDocumentState(next: AislideDocument) {
    setDocumentSnapshot(next)
    setHistoryState({ undo: Boolean(session.current?.canUndo), redo: Boolean(session.current?.canRedo) })
  }

  useEffect(() => {
    let disposed = false
    loadInitial().then((initialDocument) => {
      if (disposed) return
      const opened = new DocumentSession(core, initialDocument)
      session.current = opened
      setDocumentState(opened.document)
      setSavedHash(opened.document.hash)
      setStatus('Blank presentation / Local core')
    }).catch((reason) => { if (!disposed) setError(String(reason)) })
    return () => { disposed = true }
  }, [])

  function undo() {
    void run(async () => {
      if (!session.current) return
      const result = await session.current.undo(); setDocumentState(result)
      changeSlide(Math.max(0, result.deck.slides.findIndex((entry) => entry.id === slide?.id)))
      setStatus('Undo / Shared transaction')
    })
  }
  function redo() {
    void run(async () => {
      if (!session.current) return
      const result = await session.current.redo(); setDocumentState(result)
      changeSlide(Math.max(0, result.deck.slides.findIndex((entry) => entry.id === slide?.id)))
      setStatus('Redo / Shared transaction')
    })
  }
  async function run(action: () => Promise<void>) {
    if (activeOperation.current || busy) return
    activeOperation.current = true
    setContext(null)
    setBusy(true)
    setError('')
    try { await action() } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { activeOperation.current = false; setBusy(false) }
  }
  async function apply(next: Deck, options?: ReplaceOptions) {
    if (!session.current) throw new Error('No document is open')
    setDocumentState(await session.current.replaceDeck(next, options))
    setStatus('Unsaved changes')
  }
  function replaceElement(next: Element) {
    if (!deck || !slide) return
    if (next.type === 'text' && next.format?.inherit_layout) next = { ...next, format: { ...next.format, inherit_layout: false } }
    void run(() => apply({ ...deck, slides: deck.slides.map((item) => item.id === slide.id ? { ...item, elements: item.elements.map((old) => old.id === next.id ? next : old) } : item) }))
  }
  async function editOnSlide(next: Element) {
    if (!deck || !slide || activeOperation.current || busy) throw new Error('Another edit is in progress')
    activeOperation.current = true; setBusy(true); setError('')
    try { await apply({ ...deck, slides: deck.slides.map((item) => item.id === slide.id ? { ...item, elements: item.elements.map((old) => old.id === next.id ? next : old) } : item) }) }
    finally { activeOperation.current = false; setBusy(false) }
  }
  function changeSlide(index: number) { setSlideIndex(index); setSelected(null) }
  function replacePresentation(action: () => Promise<void>) {
    if (busy || activeOperation.current) return
    setContext(null); setError('')
    if (dirty) setPendingReplacement(() => action)
    else void run(action)
  }
  async function newPresentation() {
    const opened = await client.createPresentation(crypto.randomUUID())
    session.current = opened; setDocumentState(opened.document); setSavedHash(null); setFilename('Untitled presentation.pptx')
    setImportWarnings([]); setModal(null); changeSlide(0); setStatus('New presentation / Unsaved')
  }
  async function savePresentation(name = filename) {
    if (!session.current) return false
    if (hasDrafts) throw new Error('Apply or cancel the current object edits before saving')
    const clean = name.trim()
    if (!clean.toLowerCase().endsWith('.pptx') || /[\\/:*?"<>|]/.test(clean) || [...clean].some((character) => character.charCodeAt(0) < 32) || /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i.test(clean.split('.')[0].trimEnd()) || clean.length > 180 || clean.startsWith('.')) throw new Error('Choose a plain PPTX filename without path characters')
    const document = session.current.document
    const result = await session.current.exportPresentation()
    const saved = await downloadPresentation(document, { ...result, filename: clean })
    if (session.current.document.id !== document.id || session.current.document.hash !== document.hash) throw new Error('The document changed during saving; newer edits remain unsaved')
    if (saved) { setSavedHash(document.hash); setFilename(clean); setStatus('PPTX export completed') } else setStatus('Save cancelled')
    return saved
  }
  function slidesCommand(operations: SlideOperation[], focus?: string) {
    void run(async () => {
      if (!session.current) return
      const result = await session.current.editSlides(operations)
      setDocumentState(result)
      const index = result.deck.slides.findIndex((entry) => entry.id === (focus ?? slide?.id))
      changeSlide(index < 0 ? Math.min(slideIndex, result.deck.slides.length - 1) : index)
      setStatus('Unsaved slide changes')
    })
  }
  function newSlide(after = slide?.id) {
    const id = `slide-${crypto.randomUUID().slice(0, 8)}`
    slidesCommand([{ op: 'insert', id, after, title: `Slide ${(deck?.slides.length ?? 0) + 1}` }], id)
  }
  function duplicateSlide(id = slide?.id) {
    if (!id) return
    const copy = `slide-${crypto.randomUUID().slice(0, 8)}`
    slidesCommand([{ op: 'duplicate', slide_id: id, id: copy }], copy)
  }
  function elementsCommand(slideId: string, operations: ElementOperation[], focus: string | null = null) {
    void run(async () => { if (!session.current) return; setDocumentState(await session.current.editElements(slideId, operations)); setSelected(focus); setStatus('Unsaved object changes') })
  }
  function duplicateElement(slideId: string, id: string) {
    const copy = `copy-${crypto.randomUUID().slice(0, 8)}`
    elementsCommand(slideId, [{ op: 'duplicate', id, new_id: copy }], copy)
  }
  function openContext(kind: 'file' | 'canvas' | 'slide' | 'element', position: MenuPosition, slideId = slide?.id, id?: string) {
    if (busy || modal || nameDialog || pendingReplacement) return
    if (slideId && kind === 'slide') changeSlide(deck?.slides.findIndex((entry) => entry.id === slideId) ?? 0)
    if (kind === 'element') setSelected(id ?? null)
    if (kind === 'canvas') setSelected(null)
    setContext({ kind, position, commands: menuCommands({ kind, slideId, id }) })
  }
  function handleContext(event: ReactMouseEvent<HTMLElement> | ReactKeyboardEvent<HTMLElement>, kind: 'file' | 'canvas' | 'slide' | 'element', slideId?: string, id?: string) {
    if ((event.target as HTMLElement).closest('input,textarea,[contenteditable=true]')) return
    if ('key' in event) {
      if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return
      event.preventDefault(); event.stopPropagation(); const bounds = event.currentTarget.getBoundingClientRect()
      openContext(kind, { x: bounds.left + 12, y: bounds.top + 12, anchor: event.currentTarget }, slideId, id)
    } else { event.preventDefault(); event.stopPropagation(); openContext(kind, { x: event.clientX, y: event.clientY, anchor: event.currentTarget }, slideId, id) }
  }
  function editMetadata(part: PartInstance) {
    void run(async () => { setPartEditing(part); if (part.spec.data.kind === 'diagram') { setGraphCatalog(await client.graphCatalog()); setModal('graph') } else { setPartCatalog(await client.partCatalog()); setModal('parts') } })
  }
  async function insertAssets(assets: AssetInput[], location?: { x: number; y: number }) {
    if (!session.current || !slide) throw new Error('No slide is open')
    if (assets.length < 1 || assets.length > 8) throw new Error('Insert between 1 and 8 assets')
    const document = session.current.document
    const index = document.deck.slides.findIndex((entry) => entry.id === slide.id)
    if (index < 0) throw new Error('The destination slide is no longer available')
    const elements: Element[] = []
    for (const [offset, asset] of assets.entries()) {
      const element = await client.createAsset(asset)
      if (location) { element.x = Math.max(0, Math.min(1280 - element.width, location.x + offset * 24)); element.y = Math.max(0, Math.min(720 - element.height, location.y + offset * 24)) }
      else { element.x += offset * 24; element.y += offset * 24 }
      elements.push(element)
    }
    const next = await session.current.transact(elements.map((element) => ({ op: 'add', path: `/deck/slides/${index}/elements/-`, value: element })), { expectedRevision: document.revision })
    setDocumentState(next); setSelected(elements.at(-1)?.id ?? null); setStatus('Unsaved asset changes')
  }
  async function importDropped(files: File[], location?: { x: number; y: number }) {
    if (files.length === 1 && files[0].name.toLowerCase().endsWith('.pptx')) {
      const file = files[0]
      replacePresentation(async () => { const opened = await client.openPresentation(crypto.randomUUID(), await fileBase64(file)); session.current = opened.session; setDocumentState(opened.session.document); setSavedHash(opened.session.document.hash); setFilename(file.name); setImportWarnings(opened.warnings); changeSlide(0); setStatus(`Opened PPTX / ${file.name}`) })
    } else await run(async () => insertAssets(await fileAssets(files), location))
  }
  const shortcuts = useEffectEvent((event: KeyboardEvent) => {
    if (event.isComposing) return
    const input = (event.target as HTMLElement)?.closest('input,textarea,select,[contenteditable=true]')
    const key = event.key.toLowerCase()
    if (event.ctrlKey || event.metaKey) {
      if (!['s', 'o', 'n', 'm', 'd', 'z', 'y'].includes(key) || input && !['s', 'o'].includes(key)) return
      event.preventDefault()
      if (busy || modal || nameDialog || pendingReplacement) return
      if (key === 's') { if (event.shiftKey) setNameDialog({ kind: 'save', value: filename }); else void run(async () => { await savePresentation() }) }
      else if (key === 'o') nativeInput.current?.click()
      else if (key === 'n') replacePresentation(newPresentation)
      else if (key === 'm') newSlide()
      else if (key === 'd') { if (slide && selected) duplicateElement(slide.id, selected); else duplicateSlide() }
      else if (key === 'y' || key === 'z' && event.shiftKey) { if (session.current?.canRedo) redo() }
      else if (session.current?.canUndo) undo()
    } else if (key === 'delete' && !input && !modal && !nameDialog && !pendingReplacement && !context && slide && selected) {
      event.preventDefault(); elementsCommand(slide.id, [{ op: 'remove', id: selected }])
    }
  })
  useEffect(() => { const handler = (event: KeyboardEvent) => shortcuts(event); window.addEventListener('keydown', handler); return () => window.removeEventListener('keydown', handler) }, [])
  useEffect(() => {
    if (!dirty) return
    const handler = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = '' }
    window.addEventListener('beforeunload', handler); return () => window.removeEventListener('beforeunload', handler)
  }, [dirty])
  const closeRequested = useEffectEvent((event: CloseRequestedEvent) => {
    if (busy || activeOperation.current || modal || nameDialog || pendingReplacement) { event.preventDefault(); return }
    if (dirty) { event.preventDefault(); setPendingReplacement(() => async () => getCurrentWindow().destroy()) }
  })
  useEffect(() => {
    if (!isTauri()) return
    let disposed = false
    let unlisten: (() => void) | undefined
    void getCurrentWindow().onCloseRequested((event) => closeRequested(event)).then((stop) => { if (disposed) stop(); else unlisten = stop }).catch((reason) => setError(`Close protection unavailable: ${String(reason)}`))
    return () => { disposed = true; unlisten?.() }
  }, [])
  function addElement(type: 'text' | 'rect' | 'chart') {
    if (!deck || !slide) return
    const id = `${type}-${crypto.randomUUID().slice(0, 8)}`
    const common = { id, x: 100, y: 220, width: 360, height: 110 }
    const next: Element = type === 'text' ? { ...common, type, text: 'New text', font_size: 32, color: '202525', bold: false }
      : type === 'chart' ? { ...common, type, width: 800, height: 350, kind: 'column', categories: ['A', 'B', 'C'], series: [{ name: 'Synthetic values', values: [4, 7, 5], color: '087F73' }] }
      : { ...common, type, fill: 'E3EFEB' }
    void run(async () => { await apply({ ...deck, slides: deck.slides.map((item) => item.id === slide.id ? { ...item, elements: [...item.elements, next] } : item) }); setSelected(id) })
  }
  const disable = busy || !deck
  const hasOrigin = Boolean(documentState?.origin)
  const importedMode = hasOrigin && !documentState?.origin?.native
  const closeModal = () => { if (!busy) { setModal(null); setError('') } }
  function menuCommands(context: { kind: 'file' | 'canvas' | 'slide' | 'element'; slideId?: string; id?: string }): MenuCommand[] {
  const fileCommands: MenuCommand[] = [
    { label: 'New presentation', icon: FilePlus2, action: () => replacePresentation(newPresentation) },
    { label: 'Open PPTX', icon: FolderOpen, action: () => nativeInput.current?.click() },
    { label: 'Save PPTX', icon: Save, action: () => void run(async () => { await savePresentation() }) },
    { label: 'Save as', icon: FileDown, action: () => setNameDialog({ kind: 'save', value: filename }) },
    { label: 'Rename presentation', icon: Pencil, action: () => setNameDialog({ kind: 'presentation', value: deck?.title ?? '' }) },
  ]
  let contextCommands = fileCommands
  const contextSlide = deck?.slides.find((entry) => entry.id === context?.slideId)
  const contextElement = contextSlide?.elements.find((entry) => entry.id === context?.id)
  if (context?.kind === 'slide' && contextSlide && deck) {
    const index = deck.slides.indexOf(contextSlide)
    contextCommands = [
      { label: 'New slide', icon: Plus, action: () => newSlide(contextSlide.id), disabled: importedMode || deck.slides.length >= 32 },
      { label: 'Duplicate slide', icon: Copy, action: () => duplicateSlide(contextSlide.id), disabled: importedMode || deck.slides.length >= 32 },
      { label: 'Rename slide', icon: Pencil, action: () => setNameDialog({ kind: 'slide', value: contextSlide.title, slideId: contextSlide.id }) },
      { label: 'Move slide up', icon: ArrowUp, action: () => slidesCommand([{ op: 'move', slide_id: contextSlide.id, index: index - 1 }]), disabled: importedMode || index === 0, separator: true },
      { label: 'Move slide down', icon: ArrowDown, action: () => slidesCommand([{ op: 'move', slide_id: contextSlide.id, index: index + 1 }]), disabled: importedMode || index === deck.slides.length - 1 },
      { label: 'Delete slide', icon: Trash2, action: () => slidesCommand([{ op: 'remove', slide_id: contextSlide.id }]), disabled: importedMode || deck.slides.length === 1, danger: true, separator: true },
    ]
  } else if (context?.kind === 'element' && contextSlide && contextElement) {
    const index = contextSlide.elements.indexOf(contextElement)
    const part = documentState?.parts?.find((part) => part.slide_id === contextSlide.id && part.element_id === contextElement.id)
    contextCommands = [
      { label: 'Edit content', icon: Pencil, disabled: !['text', 'shape', 'table', 'group'].includes(contextElement.type), action: () => setEditRequest({ id: contextElement.id, slideId: contextSlide.id, sequence: Date.now() }) },
      { label: 'Properties', icon: PanelRight, action: () => { setSelected(contextElement.id); setShowInspector(true) } },
      ...(part ? [{ label: part.spec.data.kind === 'diagram' ? 'Edit graph' : 'Edit part data', icon: Blocks, action: () => editMetadata(part), disabled: part.stale }] : []),
      { label: 'Duplicate element', icon: Copy, action: () => duplicateElement(contextSlide.id, contextElement.id), disabled: importedMode, separator: true },
      { label: 'Bring to front', icon: ArrowUpToLine, action: () => elementsCommand(contextSlide.id, [{ op: 'order', id: contextElement.id, index: contextSlide.elements.length - 1 }], contextElement.id), disabled: index === contextSlide.elements.length - 1 },
      { label: 'Send to back', icon: ArrowDownToLine, action: () => elementsCommand(contextSlide.id, [{ op: 'order', id: contextElement.id, index: 0 }], contextElement.id), disabled: index === 0 },
      { label: 'Delete element', icon: Trash2, action: () => elementsCommand(contextSlide.id, [{ op: 'remove', id: contextElement.id }]), danger: true, separator: true },
    ]
  } else if (context?.kind === 'canvas') contextCommands = [
    { label: 'Add text', icon: Type, action: () => addElement('text') },
    { label: 'Add rectangle', icon: Square, action: () => addElement('rect') },
    { label: 'Insert icons', icon: Sticker, action: () => setModal('assets'), disabled: importedMode },
    { label: 'Add picture', icon: ImagePlus, action: () => { pictureTarget.current = null; pictureInput.current?.click() } },
    { label: 'Insert objects', icon: Shapes, action: () => void run(async () => { setCatalog(await client.objectCatalog()); setModal('insert') }), disabled: importedMode },
    { label: 'Architecture diagram', icon: Network, action: () => void run(async () => { setGraphCatalog(await client.graphCatalog()); setPartEditing(null); setModal('graph') }), disabled: importedMode },
    { label: 'Parts library', icon: Blocks, action: () => void run(async () => { setPartCatalog(await client.partCatalog()); setPartEditing(null); setModal('parts') }), disabled: importedMode },
    { label: 'New slide', icon: Plus, action: () => newSlide(), separator: true, disabled: importedMode || (deck?.slides.length ?? 0) >= 32 },
    ...fileCommands.map((command, index) => ({ ...command, separator: index === 0 })),
  ]
  return contextCommands
  }

  return <div className="studio">
    <header className="app-header">
      <div className="brand"><span className="brand-mark"><Presentation size={23} /></span><h1>AISlide</h1><span className="version">STUDIO / 01</span></div>
      <div className="document-name"><button className="document-name-button" aria-label="Rename presentation" disabled={disable} onClick={() => setNameDialog({ kind: 'presentation', value: deck?.title ?? '' })}><strong>{deck?.title ?? 'Untitled presentation'}</strong></button><span className={dirty ? 'dirty-indicator' : ''}>{dirty ? 'Unsaved changes' : filename}</span></div>
      <div className="header-actions">
        <Tool label="File operations" disabled={disable} onClick={() => { const anchor = document.activeElement as HTMLElement; const bounds = anchor.getBoundingClientRect(); openContext('file', { x: bounds.left, y: bounds.bottom, anchor }) }}><Menu size={18} /></Tool>
        <Tool label="New presentation" disabled={disable} onClick={() => replacePresentation(newPresentation)}><FilePlus2 size={18} /></Tool>
        <Tool label="Sources" disabled={disable || importedMode} onClick={() => setModal('sources')}><Database size={18} /></Tool>
        <Tool className="secondary" label="Generate with AI" disabled={disable || hasOrigin} onClick={() => void run(async () => { setProvider(await core<ProviderStatus>({ op: 'provider_status' })); setModal('generate') })}><Sparkles size={20} /><span>Generate</span></Tool>
        <Tool className="secondary" label="Report data" disabled={disable || hasOrigin || !report} onClick={() => { setReportJson(JSON.stringify(report, null, 2)); setError(''); setModal('report') }}><FileJson2 size={20} /><span>Report data</span></Tool>
        <Tool label="Open PPTX" disabled={disable} onClick={() => nativeInput.current?.click()}><FolderOpen size={19} /></Tool>
        <Tool label="Inspect PPTX" disabled={disable} onClick={() => pptxInput.current?.click()}><FileInput size={19} /></Tool>
        <Tool className="primary" label="Save PPTX" disabled={disable} onClick={() => void run(async () => { await savePresentation() })}><Save size={20} /><span>Save .pptx</span></Tool>
        <Tool label="Save as" disabled={disable} onClick={() => setNameDialog({ kind: 'save', value: filename })}><FileDown size={18} /></Tool>
      </div>
      <input ref={pptxInput} type="file" accept=".pptx" hidden aria-label="Inspect PPTX file" onChange={(event) => {
        const file = event.target.files?.[0]
        event.target.value = ''
        if (!file) return
        void run(async () => {
          const base64 = await fileBase64(file)
          const result = await core<Inspection>({ op: 'inspect', base64 })
          setImported(base64); setInspection(result); setImportedSlide(0); setImportedRun(0); setPatch(result.slides[0]?.texts[0]?.text ?? ''); setModal('inspect')
        })
      }} />
      <input ref={nativeInput} type="file" accept=".pptx" hidden aria-label="Open PPTX file" onChange={(event) => {
        const file = event.target.files?.[0]; event.target.value = ''
        if (!file) return
        replacePresentation(async () => {
          if (!file.name.toLowerCase().endsWith('.pptx')) throw new Error('Choose an Open XML .pptx presentation')
          const imported = await client.openPresentation(crypto.randomUUID(), await fileBase64(file))
          session.current = imported.session; setDocumentState(imported.session.document); changeSlide(0)
          setFilename(file.name); setSavedHash(imported.session.document.hash)
          setImportWarnings(imported.warnings); setModal(imported.warnings.length > 1 ? 'import' : null); setStatus(`Opened PPTX / ${file.name}`)
        })
      }} />
    </header>

    <div className="ribbon">
      <div className="ribbon-group"><Tool label="Undo" disabled={disable || !historyState.undo} onClick={undo}><Undo2 size={18} /></Tool><Tool label="Redo" disabled={disable || !historyState.redo} onClick={redo}><Redo2 size={18} /></Tool></div>
      <div className="ribbon-group"><Tool label="Add text" disabled={disable} onClick={() => addElement('text')}><Type size={19} /></Tool><Tool label="Add rectangle" disabled={disable} onClick={() => addElement('rect')}><Square size={18} /></Tool><Tool label="Add chart" disabled={disable} onClick={() => addElement('chart')}><ChartNoAxesCombined size={18} /></Tool>
        <Tool label="Insert objects" disabled={disable || importedMode} onClick={() => void run(async () => { setCatalog(await core<ObjectCatalog>({ op: 'object_catalog' })); setModal('insert') })}><Shapes size={18} /></Tool>
        <Tool label="Parts library" disabled={disable || importedMode} onClick={() => void run(async () => { setPartCatalog(await client.partCatalog()); setPartEditing(null); setModal('parts') })}><Blocks size={18} /></Tool>
        <Tool label="Add picture" disabled={disable} onClick={() => { pictureTarget.current = null; pictureInput.current?.click() }}><ImagePlus size={18} /></Tool>
        <Tool label="Insert icons" disabled={disable || importedMode} onClick={() => setModal('assets')}><Sticker size={18} /></Tool>
        <Tool label="Architecture diagram" disabled={disable || importedMode} onClick={() => void run(async () => { setGraphCatalog(await client.graphCatalog()); setPartEditing(null); setModal('graph') })}><Network size={18} /></Tool>
        <Tool label="Add process diagram" disabled={disable} onClick={() => void run(async () => {
          if (!deck || !slide) return
          const next = await core<Element>({ op: 'create_diagram', id: `process-${crypto.randomUUID().slice(0, 8)}`, steps: ['Collect', 'Verify', 'Publish'] })
          await apply({ ...deck, slides: deck.slides.map((entry) => entry.id === slide.id ? { ...entry, elements: [...entry.elements, next] } : entry) })
          setSelected(next.id)
        })}><Workflow size={18} /></Tool>
      </div>
      <input ref={pictureInput} type="file" accept="image/png,image/jpeg" hidden aria-label="Open image file" onChange={(event) => {
        const file = event.target.files?.[0]
        event.target.value = ''
        if (!file || !deck || !slide) return
        const target = pictureTarget.current
        void run(async () => {
          if (file.size > 1024 * 1024) throw new Error('Picture must be at most 1 MiB')
          const next = await core<Element>({ op: 'create_picture', id: target ?? `picture-${crypto.randomUUID().slice(0, 8)}`, base64: await fileBase64(file), mime_type: file.type, alt: file.name })
          await apply({ ...deck, slides: deck.slides.map((entry) => entry.id === slide.id ? { ...entry, elements: target ? entry.elements.map((old) => old.id === target ? { ...next, x: old.x, y: old.y, width: old.width, height: old.height } : old) : [...entry.elements, next] } : entry) })
          setSelected(next.id)
        })
      }} />
      <div className="ribbon-group"><Tool label="New slide" disabled={disable || importedMode || (deck?.slides.length ?? 0) >= 32} onClick={() => newSlide()}><Plus size={18} /></Tool><Tool label="Duplicate slide" disabled={disable || !slide || importedMode || (deck?.slides.length ?? 0) >= 32} onClick={() => duplicateSlide()}><Copy size={18} /></Tool><Tool label="Delete element" disabled={disable || !selected} onClick={() => { if (slide && selected) elementsCommand(slide.id, [{ op: 'remove', id: selected }]) }}><Trash2 size={18} /></Tool></div>
      <span className="ribbon-space" />
      <Tool label="Edit theme" disabled={disable || importedMode} onClick={() => void run(async () => { setDesignDefaults(deck?.design ?? await core<Design>({ op: 'design_defaults' })); setModal('theme') })}><Palette size={18} /></Tool>
      <Tool label="Edit masters and layouts" disabled={disable || importedMode} onClick={() => void run(async () => { setDesignDefaults(deck?.design ?? await core<Design>({ op: 'design_defaults' })); setModal('design') })}><LayoutTemplate size={18} /></Tool>
      {deck?.design && slide && <><select aria-label="Slide layout" className="layout-select" disabled={disable || importedMode} value={slide.layout_id ?? deck.design.layouts[0].id} onChange={(event) => { const layout_id = event.target.value; void run(async () => apply(await core<Deck>({ op: 'assign_layout', deck, slide_id: slide.id, layout_id }))) }}>{deck.design.layouts.map((layout) => <option key={layout.id} value={layout.id}>{layout.name}</option>)}</select><Tool label="Reset layout" disabled={disable || importedMode || !slide.layout_id} onClick={() => void run(async () => apply(await core<Deck>({ op: 'assign_layout', deck, slide_id: slide.id, layout_id: slide.layout_id })))}><RotateCcw size={18} /></Tool></>}
      <Tool label="New report" disabled={disable} onClick={() => replacePresentation(async () => {
        const report = await core<Report>({ op: 'sample' }); const result = await core<Compiled>({ op: 'compile', report })
        session.current = await client.createDocument({ id: crypto.randomUUID(), deck: result.deck, report }); setDocumentState(session.current.document)
        setSavedHash(null); setFilename('sample-report.pptx'); setImportWarnings([]); changeSlide(0); setStatus('New synthetic report')
      })}><Presentation size={18} /></Tool>
      <Tool label="Validate layout" disabled={disable} onClick={() => void run(async () => { setLayoutReport(await core<LayoutReport>({ op: 'measure_layout', deck })); setModal('layout') })}><ScanLine size={18} /></Tool>
      <Tool label="PPTX details" disabled={disable || !hasOrigin} onClick={() => setModal('import')}><AlertCircle size={18} /></Tool>
      <select aria-label="Zoom" className="zoom-select" value={zoom} onChange={(event) => setZoom(event.target.value)}><option value="fit">Fit</option><option value="0.75">75%</option><option value="1">100%</option></select>
      <Tool label="Toggle inspector" pressed={showInspector} onClick={() => setShowInspector(!showInspector)}><PanelRight size={18} /></Tool>
    </div>

    {error && !modal && !nameDialog && !pendingReplacement && <div className="error-strip" role="alert"><AlertCircle size={17} />{error}<button aria-label="Dismiss error" onClick={() => setError('')}><X size={16} /></button></div>}

    <div className={`workbench ${showInspector ? '' : 'inspector-hidden'}`}>
      <aside className="slide-list" aria-label="Slides">
        <div className="panel-heading"><h2>Slides</h2><span>{deck?.slides.length ?? 0}</span></div>
        <div className="thumbnails">{deck?.slides.map((item, index) => <button key={item.id} disabled={busy} className={`thumbnail ${index === slideIndex ? 'active' : ''}`} aria-label={`Slide ${index + 1}: ${item.title.replaceAll('\n', ' ')}`} aria-current={index === slideIndex ? 'true' : undefined} onClick={() => changeSlide(index)} onContextMenu={(event) => handleContext(event, 'slide', item.id)} onKeyDown={(event) => handleContext(event, 'slide', item.id)}>
          <div className="thumbnail-image"><SlideSurface slide={item} design={deck?.design} /></div><div className="thumbnail-caption"><span>{String(index + 1).padStart(2, '0')}</span><strong>{item.title.split('\n')[0]}</strong></div>
        </button>)}</div>
      </aside>

      <main className={`canvas-workspace ${assetDrag ? 'asset-drag' : ''}`} onContextMenu={(event) => handleContext(event, 'canvas', slide?.id)} onKeyDown={(event) => handleContext(event, 'canvas', slide?.id)} onDragOver={(event) => { if (disable || modal) return; if (event.dataTransfer.types.includes('Files')) { event.preventDefault(); event.dataTransfer.dropEffect = 'copy'; setAssetDrag(true) } }} onDragLeave={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node)) setAssetDrag(false) }} onDrop={(event) => {
        event.preventDefault(); setAssetDrag(false)
        if (disable || modal || pendingReplacement || nameDialog) return
        const stage = event.currentTarget.querySelector('.slide-stage')?.getBoundingClientRect()
        const location = stage ? { x: (event.clientX - stage.left) * 1280 / stage.width, y: (event.clientY - stage.top) * 1280 / stage.width } : undefined
        void importDropped([...event.dataTransfer.files], location)
      }} onPaste={(event) => {
        if (disable || modal || pendingReplacement || nameDialog || (event.target as HTMLElement).closest('input,textarea,[contenteditable=true]')) return
        const files = [...event.clipboardData.files]
        if (files.length) { event.preventDefault(); void run(async () => insertAssets(await fileAssets(files))); return }
        const text = event.clipboardData.getData('text/plain').trim()
        if (/^(?:<\?xml[\s\S]*?\?>\s*)?<svg\b/i.test(text)) { event.preventDefault(); void run(async () => insertAssets([svgAsset(text)])) }
      }}>
        <div className="canvas-heading"><span>{String(slideIndex + 1).padStart(2, '0')} <span className="slash">/</span> {String(deck?.slides.length ?? 0).padStart(2, '0')}</span><h2>{slide?.title.split('\n')[0] ?? 'Opening report'}</h2><span className="native-badge">NATIVE OBJECTS</span></div>
        <div className="canvas-scroll">
          <div className="slide-stage" style={{ width: zoom === 'fit' ? '100%' : `${1280 * Number(zoom)}px` }}>
            {slide ? <SlideSurface slide={slide} design={deck?.design} selected={selected} onEdit={editOnSlide} onDraftChange={setInlineDraft} editRequest={editRequest} onContextMenu={(position, id) => openContext(id ? 'element' : 'canvas', position, slide.id, id)} onSelect={busy ? undefined : setSelected} onMove={(id, x, y) => {
              const original = slide.elements.find((item) => item.id === id)
              if (original && !busy) replaceElement({ ...original, x, y })
            }} onResize={(id, width, height) => {
              const original = slide.elements.find((item) => item.id === id)
              if (original && !busy) replaceElement({ ...original, width, height })
            }} /> : <div className="loading-state">{error ? 'Core unavailable' : 'Opening local report...'}</div>}
          </div>
        </div>
        <div className="canvas-footer"><span>{selected ? `${element?.type ?? ''} / ${selected}` : 'No selection'}</span><div><Tool label="Previous slide" disabled={slideIndex === 0} onClick={() => changeSlide(slideIndex - 1)}><ChevronLeft size={17} /></Tool><Tool label="Next slide" disabled={!deck || slideIndex === deck.slides.length - 1} onClick={() => changeSlide(slideIndex + 1)}><ChevronRight size={17} /></Tool></div></div>
        <div className="notes-preview"><FileText size={16} /><span>{slide?.notes.split('\n')[0]}</span></div>
      </main>

      {showInspector && <aside className="inspector" aria-label="Inspector">
        <div className="panel-tabs" role="tablist" aria-label="Inspector views"><button role="tab" aria-selected={panel === 'elements'} onClick={() => setPanel('elements')}><Layers size={16} />Elements</button><button role="tab" aria-selected={panel === 'notes'} onClick={() => setPanel('notes')}><FileText size={16} />Notes</button></div>
        {panel === 'elements' ? <>
          <div className="layer-list">{slide?.elements.map((item) => <button key={item.id} disabled={busy} className={selected === item.id ? 'active' : ''} aria-label={`Select ${item.id}`} onClick={() => setSelected(item.id)} onContextMenu={(event) => handleContext(event, 'element', slide.id, item.id)} onKeyDown={(event) => handleContext(event, 'element', slide.id, item.id)}>{item.type === 'text' ? <Type size={15} /> : item.type === 'table' ? <Table2 size={15} /> : <Square size={15} />}<span>{item.id}</span></button>)}</div>
          {selectedPart && <div className="part-data-action"><button className="secondary" aria-label={selectedPart.spec.data.kind === 'diagram' ? 'Edit graph' : 'Edit part data'} disabled={busy || selectedPart.stale} onClick={() => void run(async () => { setPartEditing(selectedPart); if (selectedPart.spec.data.kind === 'diagram') { setGraphCatalog(await client.graphCatalog()); setModal('graph') } else { setPartCatalog(await client.partCatalog()); setModal('parts') } })}><Blocks size={16} />{selectedPart.spec.data.kind === 'diagram' ? 'Edit graph' : 'Edit part data'}</button>{selectedPart.stale && <p>Part data is out of sync with native edits.</p>}</div>}
          {element ? <><div className="selection-heading"><strong>{element.id}</strong><span>{element.type}</span></div><Properties key={JSON.stringify(element)} element={element} theme={deck?.design?.theme} busy={busy} onDraftChange={setPropertiesDraft} onApply={async (value) => editOnSlide(value.type === 'text' && value.format?.inherit_layout ? { ...value, format: { ...value.format, inherit_layout: false } } : value)} onReplaceImage={() => { pictureTarget.current = element.id; pictureInput.current?.click() }} /></> : <div className="empty-properties"><Layers size={24} /><p>No element selected</p></div>}
          {slide && deck && <div className="slide-design-properties"><ColorField label="Slide background" value={slide.background} theme={deck.design?.theme} onChange={(background) => void run(() => apply({ ...deck, slides: deck.slides.map((entry) => entry.id === slide.id ? { ...entry, background, inherit_background: false } : entry) }))} />{deck.design && <><label className="checkbox"><input type="checkbox" disabled={busy} checked={Boolean(slide.inherit_background)} onChange={(event) => { const inherit_background = event.target.checked; void run(() => apply({ ...deck, slides: deck.slides.map((entry) => entry.id === slide.id ? { ...entry, inherit_background } : entry) })) }} />Use layout background</label><label className="checkbox"><input type="checkbox" disabled={busy} checked={!slide.hide_master_graphics} onChange={(event) => { const hide_master_graphics = !event.target.checked; void run(() => apply({ ...deck, slides: deck.slides.map((entry) => entry.id === slide.id ? { ...entry, hide_master_graphics } : entry) })) }} />Show master graphics</label></>}</div>}
          <div className="slide-order"><span>Slide order</span><Tool label="Move slide up" disabled={disable || importedMode || slideIndex === 0} onClick={() => { if (slide) slidesCommand([{ op: 'move', slide_id: slide.id, index: slideIndex - 1 }]) }}><ArrowUp size={16} /></Tool><Tool label="Move slide down" disabled={disable || importedMode || !deck || slideIndex >= deck.slides.length - 1} onClick={() => { if (slide) slidesCommand([{ op: 'move', slide_id: slide.id, index: slideIndex + 1 }]) }}><ArrowDown size={16} /></Tool></div>
        </> : <div className="notes-panel"><h3>Speaker notes & sources</h3><p>{slide?.notes}</p></div>}
        <div className="validation-note"><AlertCircle size={16} /><span>Office text layout unverified</span></div>
      </aside>}
    </div>
    <footer className="status-bar"><span className={busy ? 'status busy' : 'status'}>{busy ? 'Processing' : status}</span><span>{deck?.slides.length ?? 0} slides<span className="status-divider">|</span>16:9<span className="status-divider">|</span>Core 0.1</span></footer>

    {context && <ContextMenu position={context.position} label={`${context.kind} actions`} commands={context.commands} onClose={closeContext} />}
    {modal === 'assets' && <Modal title="Icons and assets" onClose={closeModal}><Suspense fallback={<p role="status">Loading icons</p>}><AssetPanel onBusy={setBusy} onInsert={async (assets) => { await insertAssets(assets); setModal(null) }} /></Suspense></Modal>}
    {pendingReplacement && <Modal title="Unsaved changes" onClose={() => { if (!busy) setPendingReplacement(null) }}><div className="workspace-form"><p>Save changes to {deck?.title}?</p>{error && <p role="alert" className="error">{error}</p>}<div className="form-actions"><button className="secondary" disabled={busy} onClick={() => setPendingReplacement(null)}>Cancel</button><button className="secondary" disabled={busy} onClick={() => void run(async () => { await pendingReplacement(); setPendingReplacement(null) })}>Discard changes</button><button className="primary" disabled={busy} onClick={() => void run(async () => { if (await savePresentation()) { await pendingReplacement(); setPendingReplacement(null) } })}><Save size={16} />Save and continue</button></div></div></Modal>}
    {nameDialog && <Modal title={nameDialog.kind === 'save' ? 'Save presentation' : nameDialog.kind === 'slide' ? 'Rename slide' : 'Rename presentation'} onClose={() => { if (!busy) setNameDialog(null) }}><form className="workspace-form" onSubmit={(event) => {
      event.preventDefault()
      void run(async () => {
        if (nameDialog.kind === 'save') { if (!await savePresentation(nameDialog.value)) return }
        else if (nameDialog.kind === 'presentation' && deck) { await apply({ ...deck, title: nameDialog.value.trim() }) }
        else if (nameDialog.slideId && session.current) setDocumentState(await session.current.editSlides([{ op: 'rename', slide_id: nameDialog.slideId, title: nameDialog.value.trim() }]))
        setNameDialog(null)
      })
    }}><label className="field">{nameDialog.kind === 'save' ? 'PPTX filename' : nameDialog.kind === 'slide' ? 'Slide title' : 'Presentation title'}<input autoFocus aria-label={nameDialog.kind === 'save' ? 'PPTX filename' : nameDialog.kind === 'slide' ? 'Slide title' : 'Presentation title'} value={nameDialog.value} required maxLength={nameDialog.kind === 'save' ? 180 : 120} disabled={busy} onChange={(event) => setNameDialog({ ...nameDialog, value: event.target.value })} /></label>{error && <p role="alert" className="error">{error}</p>}<div className="form-actions"><button type="button" className="secondary" disabled={busy} onClick={() => setNameDialog(null)}>Cancel</button><button type="submit" className="primary" disabled={busy}>{nameDialog.kind === 'save' ? 'Save PPTX' : 'Apply'}</button></div></form></Modal>}

    {modal === 'generate' && provider && <Modal title="Generate report" onClose={closeModal}>
      <GenerationPanel provider={provider} onBusy={setBusy} onApply={(draft) => void run(async () => {
        await apply(draft.compiled.deck, { report: draft.report, sources: [], bindings: [] })
        changeSlide(0)
        setStatus('AI draft / Content unverified')
        setModal(null)
      })} />
      {error && <p role="alert" className="error modal-error">{error}</p>}
    </Modal>}

    {modal === 'insert' && catalog && deck && slide && <Modal title="Insert objects" onClose={closeModal}><InsertPanel catalog={catalog} onBusy={setBusy} onInsert={async (next) => {
      await apply({ ...deck, slides: deck.slides.map((entry) => entry.id === slide.id ? { ...entry, elements: [...entry.elements, next] } : entry) }); setSelected(next.id); setModal(null)
    }} /></Modal>}
    {modal === 'parts' && partCatalog && deck && slide && <Modal title="Parts library" onClose={closeModal}><PartsPanel catalog={partCatalog} theme={deck.design?.theme} instance={partEditing} onBusy={setBusy} onApply={async (spec) => {
      if (!session.current) throw new Error('No document is open')
      const id = partEditing?.element_id ?? `part-${crypto.randomUUID().slice(0, 8)}`
      const document = partEditing ? await session.current.updatePart(slide.id, { id, spec }) : await session.current.addPart(slide.id, { id, spec })
      setDocumentState(document); setSelected(id); setStatus('Unsaved part changes'); setModal(null)
    }} /></Modal>}
    {modal === 'graph' && graphCatalog && deck && slide && <Modal title="Architecture diagram" onClose={closeModal}><Suspense fallback={<p role="status">Loading diagram editor</p>}><GraphEditor catalog={graphCatalog} initial={partEditing?.spec.data.kind === 'diagram' ? partEditing.spec.data.graph : undefined} editing={Boolean(partEditing)} theme={deck.design?.theme} onBusy={setBusy} onApply={async (spec) => {
      if (!session.current) throw new Error('No document is open')
      const id = partEditing?.element_id ?? `graph-${crypto.randomUUID().slice(0, 8)}`
      const document = partEditing ? await session.current.updateGraph(slide.id, { id, spec }) : await session.current.addGraph(slide.id, { id, spec })
      setDocumentState(document); setSelected(id); setStatus('Unsaved graph changes'); setModal(null)
    }} /></Suspense></Modal>}
    {modal === 'theme' && designDefaults && deck && <Modal title="Theme" onClose={closeModal}><ThemePanel theme={designDefaults.theme} onBusy={setBusy} onApply={async (theme) => {
      await apply(await core<Deck>({ op: 'apply_theme', deck, theme })); setModal(null)
    }} /></Modal>}
    {modal === 'design' && designDefaults && deck && <Modal title="Masters and layouts" onClose={closeModal}><DesignPanel design={designDefaults} deck={deck} preserveStructure={hasOrigin} onBusy={setBusy} onSave={async (design) => { await apply(await core<Deck>({ op: 'update_design', deck, design })); setModal(null) }} onPreset={async (preset_id, design) => {
      const updated = await core<Deck>({ op: 'update_design', deck, design })
      await apply(await core<Deck>({ op: 'apply_design_preset', deck: updated, preset_id })); setModal(null)
    }} /></Modal>}

    {modal === 'sources' && <Modal title="Sources" onClose={closeModal}>
      <SourcePanel sources={documentState?.sources ?? []} onBusy={setBusy} onApply={async (result, source) => {
        if (hasOrigin) { const next = await client.createDocument({ id: crypto.randomUUID(), deck: result.compiled.deck, report: result.report, sources: [source], bindings: result.bindings }); session.current = next; setDocumentState(next.document) }
        else await apply(result.compiled.deck, { report: result.report, sources: [source], bindings: result.bindings })
        changeSlide(0); setStatus('Source-bound report / Data not independently verified'); setModal(null)
      }} onAttach={async (source) => {
        if (!deck) return
        await apply(deck, { sources: [...(documentState?.sources ?? []).filter((entry) => entry.id !== source.id), source] })
        setStatus('Source attached'); setModal(null)
      }} />
    </Modal>}

    {modal === 'layout' && layoutReport && <Modal title="Layout validation" onClose={closeModal}>
      <div className="source-form"><div className="source-identity"><strong>{layoutReport.measurements.length} measured text frames/cells</strong><span>{layoutReport.engine}</span></div>
        <p className="warning">Office visual parity is not verified. Used fonts: {layoutReport.fonts.join(', ')}</p>
        <ul className="layout-issues">{layoutReport.issues.map((issue, index) => <li key={index} className={issue.severity === 'error' ? 'error' : 'warning'}><strong>{issue.code}</strong><span>{issue.message}</span></li>)}</ul>
      </div>
    </Modal>}
    {modal === 'import' && <Modal title="PPTX details" onClose={closeModal}>
      <div className="source-form"><p className="warning">Original package retained. Unsupported content may be absent from preview. Slide/master/layout counts and order are currently preserved when reopening a PPTX. Unsupported edits fail without replacing the open document.</p><ul className="layout-issues">{importWarnings.map((warning, index) => <li key={index}>{warning}</li>)}</ul></div>
    </Modal>}

    {modal === 'report' && <Modal title="Report data" onClose={closeModal}>
      <label className="field report-field">Report JSON<textarea aria-label="Report JSON" className="code-input" value={reportJson} onChange={(event) => setReportJson(event.target.value)} rows={19} spellCheck={false} /></label>
      {error && <p className="error modal-error" role="alert">{error}</p>}
      <div className="modal-actions"><label className="secondary upload-label"><FolderOpen size={16} />Open JSON<input type="file" accept=".json" onChange={(event) => { const file = event.target.files?.[0]; if (file) void run(async () => { if (file.size > 2 * 1024 * 1024) throw new Error('JSON must be smaller than 2 MiB'); setReportJson((await file.text()).replace(/^\uFEFF/, '')) }) }} /></label><button className="primary" disabled={busy} onClick={() => void run(async () => { const input = JSON.parse(reportJson); const result = await core<Compiled>({ op: 'compile', report: input }); await apply(result.deck, { report: input, sources: [], bindings: [] }); changeSlide(0); setModal(null) })}><Check size={16} />Compile report</button></div>
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