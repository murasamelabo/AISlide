import { useEffect, useRef, useState } from 'react'
import { Eye, Plus, X } from 'lucide-react'
import { AislideClient, type AislideDocument, type DocumentSession, type MasterImportInput, type MasterImportPreview, type MasterSourceInspection, type MasterSourceInput } from '../../../packages/client/index.mjs'
import { core, fileBase64 } from './api'
import { usePanelTask } from './DocumentSetupPanel'
import { SlideSurface } from './SlideSurface'
import './object-tools.css'

type Props = { session: DocumentSession; onDocument: (document: AislideDocument) => void; onBusy: (busy: boolean) => void; onCancel: () => void }
type Source = { input: MasterSourceInput; inspection: MasterSourceInspection; filename: string }
type Preview = { input: MasterImportInput; result: MasterImportPreview }

export function MasterImportPanel({ session, onDocument, onBusy, onCancel }: Props) {
  const [source, setSource] = useState<Source | null>(null)
  const [mode, setMode] = useState<MasterImportInput['mode']>('masters')
  const [ids, setIds] = useState<string[]>([])
  const [name, setName] = useState('Imported masters')
  const [preview, setPreview] = useState<Preview | null>(null)
  const [previewIndex, setPreviewIndex] = useState(0)
  const generation = useRef(0)
  const controller = useRef<AbortController | null>(null)
  const task = usePanelTask(session, onBusy)
  const document = session.document
  const entries = source ? mode === 'masters' ? source.inspection.masters : source.inspection.slides : []
  const stale = preview && (preview.result.base_revision !== session.revision || preview.result.base_hash !== document.hash)
  const candidate = stale ? null : preview
  const previewSlides = candidate?.result.preview_slides.slice(0, 32) ?? []
  const previewSlide = previewSlides[previewIndex]
  const warnings = [...new Set([...(source?.inspection.warnings ?? []), ...(candidate?.result.warnings ?? [])])]
  const valid = Boolean(source && name.trim() && name.length <= 60 && ids.length >= 1 && ids.length <= 8)

  useEffect(() => () => { generation.current += 1; controller.current?.abort() }, [session])

  function invalidate() {
    generation.current += 1
    controller.current?.abort()
    setPreview(null); setPreviewIndex(0)
  }

  function readSource(file: File) {
    invalidate(); setSource(null); setIds([])
    const version = generation.current
    const request = new AbortController()
    controller.current = request
    void task.run(async active => {
      const kind = file.name.split('.').at(-1)?.toLowerCase()
      if (kind !== 'pptx' && kind !== 'potx') throw new Error('Choose a PPTX or POTX file.')
      const base64 = await fileBase64(file, session.capacityProfile)
      if (!active() || request.signal.aborted || version !== generation.current) return
      const input: MasterSourceInput = { kind, base64 }
      const client = new AislideClient(core, { capacityProfile: session.capacityProfile })
      const inspection = await client.inspectMasterSource(input, { signal: request.signal })
      if (!active() || request.signal.aborted || version !== generation.current) return
      setSource({ input, inspection, filename: file.name })
      setName(file.name.replace(/\.(pptx|potx)$/i, '').trim().slice(0, 60) || 'Imported masters')
    })
  }

  function prepare() {
    if (!source || !valid || task.busy) return
    invalidate()
    const version = generation.current
    const request = new AbortController()
    controller.current = request
    const input: MasterImportInput = { ...source.input, source_sha256: source.inspection.source_sha256, mode, ids: [...ids], prefix: `mi-${crypto.randomUUID().replaceAll('-', '').slice(0, 28)}`, name: name.trim() }
    Object.freeze(input.ids); Object.freeze(input)
    const expectedRevision = session.revision
    const expectedHash = document.hash
    void task.run(async active => {
      const result = await session.previewMasterImport(input, { expectedRevision, expectedHash, signal: request.signal })
      if (!active() || request.signal.aborted || version !== generation.current) return
      if (result.base_revision !== session.revision || result.base_hash !== session.document.hash || result.source_sha256 !== input.source_sha256) throw new Error('Document or source changed. Preview again before adding masters.')
      setPreview({ input, result })
    }, expectedRevision)
  }

  function apply() {
    if (!candidate || task.busy) return
    const frozen = candidate
    const version = generation.current
    const request = new AbortController()
    controller.current = request
    void task.run(async active => {
      const result = await session.importMasters(frozen.input, frozen.result.candidate_hash, { expectedRevision: frozen.result.base_revision, expectedHash: frozen.result.base_hash, signal: request.signal })
      if (!active() || request.signal.aborted || version !== generation.current) return
      setPreview(null); setSource(null); setIds([])
      onDocument(result)
    }, frozen.result.base_revision)
  }

  return <section className="object-tools-panel master-import-panel" aria-label="Master import" aria-busy={task.busy} onKeyDown={event => { if (event.key !== 'Escape') event.stopPropagation() }}>
    <fieldset disabled={task.busy}><legend>Source</legend>
      <label>Master source file<input type="file" accept=".pptx,.potx" onChange={event => {
        const file = event.target.files?.[0]; event.target.value = ''
        if (file) readSource(file)
      }} /></label>
      {source && <p>{source.filename} / {source.inspection.width} x {source.inspection.height}</p>}
      <label>Import mode<select aria-label="Import mode" value={mode} onChange={event => { invalidate(); setMode(event.target.value as typeof mode); setIds([]) }}><option value="masters">Existing masters</option><option value="slides">Sample slides</option></select></label>
      <label>Imported master name<input required maxLength={60} value={name} onChange={event => { invalidate(); setName(event.target.value) }} /></label>
    </fieldset>
    {source && <fieldset disabled={task.busy}><legend>{mode === 'masters' ? 'Existing masters' : 'Sample slides'} ({ids.length}/8 selected)</legend>
      <ul className="object-tools-list master-import-entries">{entries.map((entry, index) => <li key={entry.id}>
        <label className="object-tools-check"><input type="checkbox" aria-label={`Select ${entry.name}`} aria-describedby={`master-import-entry-${index}`} checked={ids.includes(entry.id)} disabled={!entry.importable || ids.length >= 8 && !ids.includes(entry.id)} onChange={event => { invalidate(); setIds(event.target.checked ? [...ids, entry.id] : ids.filter(id => id !== entry.id)) }} /><span>{entry.name}</span></label>
        <span id={`master-import-entry-${index}`}>{entry.importable ? 'layout_count' in entry ? `${entry.layout_count} layouts` : 'Sample slide' : entry.reason || 'Not importable'}</span>
      </li>)}</ul>
      {!entries.length && <p>No {mode === 'masters' ? 'masters' : 'slides'} in this source.</p>}
    </fieldset>}
    <div className="object-tools-actions">
      <button type="button" disabled={task.busy || !valid} onClick={prepare}><Eye aria-hidden="true" />Preview masters</button>
      <button type="button" disabled={task.busy || !candidate} onClick={apply}><Plus aria-hidden="true" />Add masters</button>
      <button type="button" disabled={task.busy} onClick={() => { invalidate(); onCancel() }}><X aria-hidden="true" />Cancel</button>
    </div>
    {stale && <p role="alert">Document changed. Preview again before adding masters.</p>}
    {candidate && previewSlide && <section aria-label="Master import preview">
      <label>Preview layout<select aria-label="Preview layout" disabled={task.busy} value={previewIndex} onChange={event => setPreviewIndex(Number(event.target.value))}>{previewSlides.map((slide, index) => <option key={slide.id} value={index}>{candidate.result.design.layouts.find(layout => layout.id === slide.layout_id)?.name ?? slide.title}</option>)}</select></label>
      <div className="master-import-preview"><SlideSurface slide={previewSlide} design={candidate.result.design} width={document.deck.width} height={document.deck.height} disabled /></div>
    </section>}
    {warnings.length > 0 && <details><summary>Import warnings ({warnings.length})</summary><ul className="object-tools-scroll">{warnings.map((warning, index) => <li key={index}>{warning}</li>)}</ul></details>}
    <p role="status">{task.busy ? 'Working on master import...' : candidate ? `${candidate.result.master_ids.length} masters / ${candidate.result.layout_ids.length} layouts ready to add` : ''}</p>
    {task.error && <p role="alert">{task.error}</p>}
  </section>
}