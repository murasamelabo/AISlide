import { useState } from 'react'
import { Bold, Italic, Underline, Type, Check, Plus, Reply, Trash2, ArrowUp, ArrowDown, ScanSearch, Download, RefreshCw, Navigation } from 'lucide-react'
import type { DocumentSession } from '../../../packages/client/index.mjs'
import type { AislideDocument, AccessibilityReport, AuxiliaryDesign, DocumentInspection, Element, ElementAccessibility, InspectionCategory, KnownFieldKind, ModernCommentOperation, ModernCommentStatus, ModernThread, RichParagraph, RichRun, Slide, TableHeaders } from './types'
import { SlideSurface } from './SlideSurface'
import { decodeBase64 } from './api'
import { Tool } from './Tool'
import { usePanelTask, type PanelExport } from './DocumentSetupPanel'
import { PanelCheck } from './ObjectToolsPanel'
import './object-tools.css'

export type ReviewPanelProps = {
  session: DocumentSession
  slideId: string
  selectedId: string | null
  onDocument: (document: AislideDocument) => void
  onBusy: (busy: boolean) => void
  onExport: PanelExport
  onNavigate?: (slideId: string, elementId: string | null) => void
}
function findElement(elements: Element[], id: string | null): Element | undefined {
  for (const element of elements) { if (element.id === id) return element; if (element.type === 'group') { const found = findElement(element.children, id); if (found) return found } }
}
export function ReviewPanel(props: ReviewPanelProps) {
  const [reload, setReload] = useState(0)
  const [identity, setIdentity] = useState(props.session)
  if (identity !== props.session) { setIdentity(props.session); setReload((value) => value + 1) }
  return <ReviewForm key={`${props.session.document.id}:${props.slideId}:${props.selectedId}:${reload}`} {...props} onReload={() => setReload((value) => value + 1)} />
}
function ReviewForm({ session, slideId, selectedId, onDocument, onBusy, onExport, onNavigate, onReload }: ReviewPanelProps & { onReload: () => void }) {
  const [snapshot, setSnapshot] = useState(session.document)
  const [section, setSection] = useState('notes')
  const slide = snapshot.deck.slides.find((item) => item.id === slideId)
  const [notes, setNotes] = useState(slide?.notes ?? '')
  const [notesParagraphs, setNotesParagraphs] = useState<RichParagraph[]>(structuredClone(slide?.notes_paragraphs ?? []))
  const [richNotes, setRichNotes] = useState(Boolean(slide?.notes_paragraphs?.length))
  const [noteParagraph, setNoteParagraph] = useState(0)
  const [noteRun, setNoteRun] = useState(0)
  const activeParagraph = notesParagraphs[noteParagraph]
  const activeRun = activeParagraph?.runs[noteRun]
  const notesText = richNotes ? notesParagraphs.map((paragraph) => paragraph.runs.map((run) => run.text).join('')).join('\n') : notes
  function editNoteRun(patch: Partial<RichRun>) { setNotesParagraphs((previous) => previous.map((paragraph, index) => index === noteParagraph ? { ...paragraph, runs: paragraph.runs.map((run, index) => index === noteRun ? { ...run, ...patch } : run) } : paragraph)) }
  function editNoteParagraph(patch: Partial<RichParagraph>) { setNotesParagraphs((previous) => previous.map((paragraph, index) => index === noteParagraph ? { ...paragraph, ...patch } : paragraph)) }
  const [author, setAuthor] = useState('')
  const [initials, setInitials] = useState('')
  const [commentText, setCommentText] = useState('')
  const [replyTo, setReplyTo] = useState<string | null>(null)
  const [commentFormat, setCommentFormat] = useState(slide?.review?.modern_threads ? 'modern' : 'legacy')
  const [headers, setHeaders] = useState<TableHeaders>(selectedId ? slide?.review?.table_headers?.[selectedId] ?? 'unknown' : 'unknown')
  const [metadata, setMetadata] = useState<ElementAccessibility>(() => selectedId ? slide?.review?.accessibility?.[selectedId] ?? {} : {})
  const [order, setOrder] = useState(() => slide?.review?.reading_order?.length ? slide.review.reading_order : slide?.elements.map((element) => element.id) ?? [])
  const [acknowledged, setAcknowledged] = useState(false)
  const [report, setReport] = useState<AccessibilityReport | null>(null)
  const [inspection, setInspection] = useState<DocumentInspection | null>(null)
  const [categories, setCategories] = useState<InspectionCategory[]>([])
  const [confirmed, setConfirmed] = useState(false)
  const [referenceDate, setReferenceDate] = useState(new Date().toISOString().slice(0, 10))
  const [referenceTime, setReferenceTime] = useState('')
  const [fieldKind, setFieldKind] = useState<KnownFieldKind>('slidenum')
  const [fieldScope, setFieldScope] = useState('selected')
  const task = usePanelTask(session, onBusy)
  const selected = slide && findElement(slide.elements, selectedId)
  const stale = snapshot.revision !== session.revision
  function accept(document: AislideDocument, active: () => boolean) {
    if (!active()) return
    setSnapshot(document); setReport(null); setInspection(null); setCategories([]); setConfirmed(false)
    onDocument(document)
  }
  const mutate = (action: (revision: number) => Promise<AislideDocument>) => task.run(async (active) => accept(await action(snapshot.revision), active), snapshot.revision)
  function move(index: number, offset: number) {
    const next = [...order]
    const target = index + offset
    if (target < 0 || target >= next.length) return
    ;[next[index], next[target]] = [next[target], next[index]]
    setOrder(next); setAcknowledged(false)
  }
  return <section className="object-tools-panel" aria-label="Review" aria-busy={task.busy} onKeyDown={(event) => event.stopPropagation()}>
    <div className="object-tools-heading"><h2>Review</h2><Tool label="Reload review" disabled={task.busy} onClick={onReload}><RefreshCw /></Tool></div>
    {stale && <p role="alert">Document changed. Reload before applying.</p>}
    {!slide && <p role="alert">Slide not found.</p>}
    <label>Review section<select value={section} onChange={(event) => setSection(event.target.value)}>{['notes', 'comments', 'accessibility', 'inspection'].map((value) => <option key={value} value={value}>{value[0].toUpperCase() + value.slice(1)}</option>)}</select></label>
    <fieldset disabled={task.busy || stale || !slide}>
      {section === 'notes' && <><label>Speaker notes<textarea aria-label="Speaker notes" value={notesText} readOnly={richNotes} maxLength={16000} onChange={(event) => setNotes(event.target.value)} /></label>
        <div className="object-tools-actions"><Tool label="Format notes" disabled={richNotes} onClick={() => { setNotesParagraphs(notes.split('\n').map((text) => ({ runs: [{ text }] }))); setRichNotes(true) }}><Type /></Tool><Tool label="Apply speaker notes" onClick={() => void mutate((expectedRevision) => richNotes ? session.updateRichNotes(slideId, notesParagraphs, { expectedRevision }) : session.updateNotes(slideId, notes, { expectedRevision }))}><Check /></Tool></div>
        {richNotes && <fieldset><legend>Notes formatting</legend>
          <div className="object-tools-grid"><label>Notes paragraph<select value={noteParagraph} onChange={(event) => { setNoteParagraph(Number(event.target.value)); setNoteRun(0) }}>{notesParagraphs.map((_, index) => <option key={index} value={index}>{index + 1}</option>)}</select></label><label>Notes run<select value={noteRun} onChange={(event) => setNoteRun(Number(event.target.value))}>{activeParagraph?.runs.map((_, index) => <option key={index} value={index}>{index + 1}</option>)}</select></label></div>
          {activeRun && <><label>Notes run text<textarea aria-label="Notes run text" value={activeRun.text} readOnly={Boolean(activeRun.field)} maxLength={16000} onChange={(event) => editNoteRun({ text: event.target.value })} /></label>
            <div className="object-tools-actions"><Tool label="Notes bold" pressed={Boolean(activeRun.style?.bold)} onClick={() => editNoteRun({ style: { ...activeRun.style, bold: !activeRun.style?.bold } })}><Bold /></Tool><Tool label="Notes italic" pressed={Boolean(activeRun.style?.italic)} onClick={() => editNoteRun({ style: { ...activeRun.style, italic: !activeRun.style?.italic } })}><Italic /></Tool><Tool label="Notes underline" pressed={Boolean(activeRun.style?.underline)} onClick={() => editNoteRun({ style: { ...activeRun.style, underline: !activeRun.style?.underline } })}><Underline /></Tool></div>
            <label>Notes font size<input type="number" min={1} max={400} value={activeRun.style?.font_size ?? 16} onChange={(event) => editNoteRun({ style: { ...activeRun.style, font_size: event.currentTarget.valueAsNumber } })} /></label>
          </>}
          <label>Notes alignment<select value={activeParagraph?.alignment ?? 'left'} onChange={(event) => editNoteParagraph({ alignment: event.target.value as RichParagraph['alignment'] })}>{['left', 'center', 'right', 'justify'].map((value) => <option key={value}>{value}</option>)}</select></label>
          <label>Notes list<select value={activeParagraph?.bullet ?? 'none'} onChange={(event) => editNoteParagraph({ bullet: event.target.value as RichParagraph['bullet'], bullet_character: null, numbering: null, number_start: null })}>{['none', 'bullet', 'numbered'].map((value) => <option key={value}>{value}</option>)}</select></label>
          <div className="object-tools-actions"><Tool label="Add notes run" onClick={() => { editNoteParagraph({ runs: [...(activeParagraph?.runs ?? []), { text: '' }] }); setNoteRun(activeParagraph?.runs.length ?? 0) }}><Plus /></Tool><Tool label="Add notes paragraph" onClick={() => { setNotesParagraphs([...notesParagraphs, { runs: [{ text: '' }] }]); setNoteParagraph(notesParagraphs.length); setNoteRun(0) }}><Type /></Tool><Tool label="Delete notes paragraph" disabled={notesParagraphs.length < 2} onClick={() => { setNotesParagraphs(notesParagraphs.filter((_, index) => index !== noteParagraph)); setNoteParagraph(0); setNoteRun(0) }}><Trash2 /></Tool></div>
        </fieldset>}
        <fieldset><legend>Dynamic fields</legend><label>Field type<select value={fieldKind} onChange={(event) => setFieldKind(event.target.value as KnownFieldKind)}><option value="slidenum">Slide number</option><option value="datetime1">Date</option></select></label><label>Reference date<input type="date" required min="0001-01-01" max="9999-12-31" value={referenceDate} onChange={(event) => setReferenceDate(event.target.value)} /></label>
          <label>Field scope<select value={fieldScope} onChange={(event) => setFieldScope(event.target.value)}><option value="selected">Selected text</option><option value="master">Current master slides</option><option value="layout">Current layout slides</option></select></label>
          <label>Reference time<input type="time" step={1} value={referenceTime} onChange={(event) => setReferenceTime(event.target.value)} /></label>
          <div className="object-tools-actions"><Tool label={fieldScope === 'selected' ? 'Add field to selected text' : 'Apply inherited field'} disabled={fieldScope === 'selected' && (!selected || (selected.type !== 'text' && selected.type !== 'shape'))} onClick={() => void mutate(async (expectedRevision) => {
            if (fieldScope !== 'selected') {
              const layout = snapshot.deck.design?.layouts.find((layout) => layout.id === slide?.layout_id) ?? snapshot.deck.design?.layouts[0]
              if (!layout) throw new Error('Select a slide with a layout.')
              return session.setDesignField({ master_id: layout.master_id, layout_id: fieldScope === 'layout' ? layout.id : null, kind: fieldKind === 'slidenum' ? 'slide_number' : 'date', reference_date: referenceDate }, { expectedRevision })
            }
            if (!selected || (selected.type !== 'text' && selected.type !== 'shape')) throw new Error('Select a text box or shape.')
            if (!/^\d{4}-\d{2}-\d{2}$/.test(referenceDate) || !Number.isFinite(Date.parse(referenceDate))) throw new Error('Select a valid reference date.')
            const paragraphs: RichParagraph[] = selected.format?.paragraphs?.length ? structuredClone(selected.format.paragraphs) : selected.text.split('\n').map((text) => ({ runs: [{ text }] }))
            const [year, month, day] = referenceDate.split('-').map(Number)
            const text = fieldKind === 'slidenum' ? String(snapshot.deck.slides.findIndex((item) => item.id === slideId) + 1) : `${month}/${day}/${year}`
            paragraphs[paragraphs.length - 1].runs.push({ text, field: { id: crypto.randomUUID(), kind: fieldKind } })
            return session.updateParagraphs(slideId, { id: selected.id, paragraphs }, { expectedRevision })
          })}><Plus /></Tool><Tool label="Refresh document fields" disabled={!referenceDate} onClick={() => void mutate((expectedRevision) => session.refreshFields(referenceDate, { expectedRevision, referenceTime: referenceTime ? (referenceTime.length === 5 ? `${referenceTime}:00` : referenceTime) : undefined, locale: 'en-US' }))}><RefreshCw /></Tool></div>
          <p role="note">Date caches use en-US. Time formats require a reference time. Other locales and unknown field types retain their cached text.</p>
        </fieldset>
      </>}
      {section === 'comments' && <><label>Comment format<select value={commentFormat} onChange={event => { setCommentFormat(event.target.value); setReplyTo(null) }}><option value="modern">Modern</option><option value="legacy">Legacy</option></select></label><div className="object-tools-grid"><label>Comment author<input required maxLength={256} autoComplete="off" value={author} onChange={(event) => setAuthor(event.target.value)} /></label><label>Author initials<input maxLength={64} autoComplete="off" value={initials} onChange={(event) => setInitials(event.target.value)} /></label></div>
        <label>{replyTo ? 'Reply text' : 'Comment text'}<textarea maxLength={8000} value={commentText} onChange={(event) => setCommentText(event.target.value)} /></label>
        <div className="object-tools-actions"><Tool label={commentFormat === 'modern' ? (replyTo ? 'Add modern reply' : 'Add modern thread') : (replyTo ? 'Add reply' : 'Add comment')} disabled={!author.trim() || !commentText.trim()} onClick={() => void task.run(async (active) => {
          if (!author.trim() || !commentText.trim()) throw new Error('Author and comment text are required.')
          const comment = { id: crypto.randomUUID(), author: author.trim(), initials: initials.trim(), timestamp: new Date(Date.now()).toISOString(), text: commentText.replaceAll('\r\n', '\n').replaceAll('\r', '\n') }
          const options = { expectedRevision: snapshot.revision }
          const draft = { author_name: comment.author, initials: comment.initials, created: comment.timestamp, body: comment.text.split('\n').map(text => ({ runs: [{ text }] })) }
          const document = await (commentFormat === 'modern' ? session.modernComment(slideId, replyTo ? { type: 'reply', thread_id: replyTo, draft } : { type: 'create', draft, anchor: { kind: 'unknown' } }, options) : replyTo ? session.replyComment(slideId, replyTo, comment, options) : session.addComment(slideId, comment, options))
          if (active()) { accept(document, active); setCommentText(''); setReplyTo(null) }
        }, snapshot.revision)}><Plus /></Tool>{replyTo && <button type="button" onClick={() => setReplyTo(null)}>Cancel reply</button>}</div>
        {commentFormat === 'modern' ? <ModernComments threads={slide?.review?.modern_threads ?? []} onReply={setReplyTo} onMutate={operation => mutate(expectedRevision => session.modernComment(slideId, operation, { expectedRevision }))} /> : <ul className="object-tools-list">{slide?.review?.comments?.map((comment) => <li className="object-tools-comment" key={comment.id}><strong>{comment.author}</strong> <time dateTime={comment.timestamp}>{comment.timestamp}</time>{comment.parent_id && <small> Reply to {slide.review?.comments?.find((item) => item.id === comment.parent_id)?.author ?? comment.parent_id}</small>}<p>{comment.text}</p>{comment.resolved && <span>Resolved</span>}<div className="object-tools-actions"><Tool label={`Reply to comment by ${comment.author}`} onClick={() => setReplyTo(comment.id)}><Reply /></Tool><Tool label={`${comment.resolved ? 'Reopen' : 'Resolve'} comment by ${comment.author}`} onClick={() => void mutate((expectedRevision) => session.resolveComment(slideId, comment.id, !comment.resolved, { expectedRevision }))}><Check /></Tool><Tool label={`Delete comment by ${comment.author} and its replies`} onClick={() => void mutate((expectedRevision) => session.removeComment(slideId, comment.id, { expectedRevision }))}><Trash2 /></Tool></div></li>)}</ul>}
      </>}
      {section === 'accessibility' && <><fieldset disabled={!selected}><legend>Selected object</legend><label>Accessible title<input maxLength={500} value={metadata.title ?? ''} onChange={(event) => setMetadata({ ...metadata, title: event.target.value })} /></label><label>Accessible description<textarea maxLength={4000} value={metadata.description ?? ''} onChange={(event) => setMetadata({ ...metadata, description: event.target.value })} /></label><PanelCheck label="Decorative" checked={Boolean(metadata.decorative)} onChange={(decorative) => setMetadata({ ...metadata, decorative })} /><Tool label="Apply accessibility" disabled={!selectedId} onClick={() => { if (selectedId) void mutate((expectedRevision) => session.setAccessibility(slideId, selectedId, metadata, { expectedRevision })) }}><Check /></Tool></fieldset>
        {selected?.type === 'table' && <fieldset><legend>Semantic table headers</legend><label>Header policy<select value={headers} onChange={event => setHeaders(event.target.value as TableHeaders)}>{(['unknown', 'none', 'first_row', 'first_column', 'both'] as const).map(policy => <option key={policy} value={policy}>{policy.replaceAll('_', ' ')}</option>)}</select></label><Tool label="Apply table headers" onClick={() => void mutate(expectedRevision => session.setTableHeaders(slideId, selected.id, headers, { expectedRevision }))}><Check /></Tool></fieldset>}
        <fieldset><legend>Reading order</legend><ol className="object-tools-list">{order.map((id, index) => <li key={id}><span>{index + 1}. {id}</span><Tool label={`Move ${id} earlier`} disabled={index === 0} onClick={() => move(index, -1)}><ArrowUp /></Tool><Tool label={`Move ${id} later`} disabled={index === order.length - 1} onClick={() => move(index, 1)}><ArrowDown /></Tool></li>)}</ol><PanelCheck label="I acknowledge this also changes object z-order" checked={acknowledged} onChange={setAcknowledged} /><Tool label="Apply reading order" disabled={!acknowledged || !order.length} onClick={() => void task.run(async (active) => {
          if (!acknowledged) throw new Error('Acknowledge the z-order change first.')
          const result = await session.setReadingOrder(slideId, order, { expectedRevision: snapshot.revision })
          accept(result.document, active)
          if (active()) { setAcknowledged(false); task.setMessage(result.warnings.join('\n')) }
        }, snapshot.revision)}><Check /></Tool></fieldset>
        <Tool label="Check accessibility" onClick={() => void task.run(async (active) => { const result = await session.checkAccessibility(); if (active() && snapshot.revision === session.revision) setReport(result) }, snapshot.revision)}><ScanSearch /></Tool>
        {report && <><ul className="object-tools-list">{report.issues.map((issue, index) => <li key={index}><span><strong>{issue.status}</strong>: {issue.code} ({issue.slide_id}{issue.element_id ? `, ${issue.element_id}` : ''}){issue.contrast_ratio != null && `; contrast ${issue.contrast_ratio.toFixed(2)} / ${issue.required_ratio}`}</span>{onNavigate && <Tool label={`Go to accessibility issue ${index + 1}`} onClick={() => onNavigate(issue.slide_id, issue.element_id)}><Navigation /></Tool>}</li>)}</ul><ul>{report.limitations.map((limit) => <li key={limit}>{limit}</li>)}</ul></>}
      </>}
      {section === 'inspection' && <><Tool label="Inspect document" onClick={() => void task.run(async (active) => { const result = await session.inspectDocument(); if (active() && snapshot.revision === session.revision) { setInspection(result); setCategories([]); setConfirmed(false) } }, snapshot.revision)}><ScanSearch /></Tool>
        {inspection && <>{Boolean(inspection.candidates?.length) && <div style={{ overflowX: 'auto' }}><table aria-label="Personal data candidates"><caption>Manual review candidates</caption><thead><tr><th scope="col">Rule</th><th scope="col">Scope</th><th scope="col">Surface</th><th scope="col">Count</th><th scope="col">Value</th></tr></thead><tbody>{inspection.candidates.map(candidate => <tr key={`${candidate.rule}:${candidate.scope}:${candidate.surface}`}><th scope="row">{candidate.rule}</th><td>{candidate.scope}</td><td>{candidate.surface}</td><td>{candidate.count}</td><td>{candidate.masked_value}</td></tr>)}</tbody></table></div>}{inspection.candidate_scan_truncated && <p role="status">Candidate scan limit reached. Manual review required.</p>}<ul className="object-tools-list">{inspection.findings.map((finding) => <li key={finding.category}><PanelCheck label={`${finding.category.replaceAll('_', ' ')} (${finding.count})`} checked={categories.includes(finding.category)} onChange={(checked) => { setCategories((current) => checked ? [...current, finding.category] : current.filter((category) => category !== finding.category)); setConfirmed(false) }} /></li>)}</ul><ul>{inspection.limitations.map((limit) => <li key={limit}>{limit}</li>)}</ul><PanelCheck label="Remove selected categories from a new copy" checked={confirmed} onChange={setConfirmed} />
          <Tool label="Download clean PPTX copy" disabled={!confirmed || !categories.length} onClick={() => void task.run(async (active) => {
            if (!confirmed || !categories.length) throw new Error('Select categories and confirm cleaning a new copy.')
            const result = await session.exportCleanCopy({ new_document_id: `clean-${crypto.randomUUID()}`, categories, confirmed: true })
            if (!active() || session.revision !== snapshot.revision) throw new Error('Document changed during inspection. Inspect again before exporting.')
            const saved = await onExport(decodeBase64(result.base64), `clean-copy-${crypto.randomUUID()}.pptx`, 'application/vnd.openxmlformats-officedocument.presentationml.presentation')
            if (active()) task.setMessage(saved === false ? 'Download cancelled.' : 'Clean copy exported. Active document unchanged.')
          }, snapshot.revision)}><Download /></Tool></>}
      </>}
    </fieldset>
    {task.error && <p role="alert">{task.error}</p>}<p role="status">{task.message}</p>
  </section>
}

function ModernComments({ threads, onReply, onMutate }: { threads: ModernThread[]; onReply: (id: string) => void; onMutate: (operation: ModernCommentOperation) => unknown }) {
  const [editing, setEditing] = useState<string | null>(null)
  const [body, setBody] = useState<RichParagraph[]>([])
  return <ul className="object-tools-list">{threads.map((thread, threadIndex) => <li className="object-tools-comment" key={thread.id}>
    {[thread, ...thread.replies].map((entry, entryIndex) => {
      const label = entryIndex === 0 ? `Modern thread ${threadIndex + 1}` : `Modern thread ${threadIndex + 1} reply ${entryIndex}`
      return <div key={entry.id}><strong>{entry.author.name}</strong> <time dateTime={entry.created}>{entry.created}</time>
        {entry.body.map((paragraph, paragraphIndex) => <p key={paragraphIndex}>{paragraph.runs.map((run, runIndex) => <span key={runIndex} style={{ fontWeight: run.style?.bold ? 700 : undefined, fontStyle: run.style?.italic ? 'italic' : undefined, textDecoration: run.style?.underline ? 'underline' : undefined }}>{run.text}</span>)}</p>)}
        <label>{label} status<select aria-label={`${label} status`} value={entry.status} onChange={event => void onMutate({ type: 'set_status', comment_id: entry.id, status: event.target.value as ModernCommentStatus })}>{(['active', 'resolved', 'closed'] as const).map(status => <option key={status}>{status}</option>)}</select></label>
        <div className="object-tools-actions">{entryIndex === 0 && <Tool label={`Reply to modern thread ${threadIndex + 1}`} onClick={() => onReply(thread.id)}><Reply /></Tool>}<Tool label={`Edit ${label.toLowerCase()} body`} onClick={() => { setEditing(entry.id); setBody(structuredClone(entry.body.length ? entry.body : [{ runs: [{ text: '' }] }])) }}><Type /></Tool><Tool label={`Delete ${label.toLowerCase()}`} onClick={() => void onMutate({ type: 'remove', comment_id: entry.id })}><Trash2 /></Tool></div>
        {editing === entry.id && <fieldset><legend>Comment body</legend>{body.map((paragraph, paragraphIndex) => paragraph.runs.map((run, runIndex) => {
          const update = (patch: Partial<RichRun>) => setBody(previous => previous.map((item, index) => index === paragraphIndex ? { ...item, runs: item.runs.map((value, index) => index === runIndex ? { ...value, ...patch } : value) } : item))
          return <div key={`${paragraphIndex}:${runIndex}`}><label>Paragraph {paragraphIndex + 1} run {runIndex + 1}<textarea maxLength={8000} readOnly={Boolean(run.field)} value={run.text} onChange={event => update({ text: event.target.value })} /></label><Tool label={`Comment paragraph ${paragraphIndex + 1} run ${runIndex + 1} bold`} pressed={Boolean(run.style?.bold)} onClick={() => update({ style: { ...run.style, bold: !run.style?.bold } })}><Bold /></Tool><Tool label={`Comment paragraph ${paragraphIndex + 1} run ${runIndex + 1} italic`} pressed={Boolean(run.style?.italic)} onClick={() => update({ style: { ...run.style, italic: !run.style?.italic } })}><Italic /></Tool></div>
        }))}<Tool label="Apply modern comment body" onClick={() => void onMutate({ type: 'update_body', comment_id: entry.id, body })}><Check /></Tool><button type="button" onClick={() => setEditing(null)}>Cancel body edit</button></fieldset>}
      </div>
    })}
  </li>)}</ul>
}

export function NotesPagePreview({ slide, auxiliary, pageNumber }: { slide: Slide; auxiliary: AuxiliaryDesign; pageNumber: number }) {
  const master = auxiliary.notes_master
  if (!master) return null
  const body = master.elements.find((element) => element.type === 'text' && element.format?.placeholder?.kind === 'body')
  const notes: Element = body?.type === 'text' ? { ...body, text: slide.notes, format: { ...body.format, inherit_layout: false, paragraphs: slide.notes_paragraphs } }
    : { type: 'text', id: 'notes-body-preview', x: 40, y: auxiliary.height * 0.35, width: auxiliary.width - 80, height: auxiliary.height * 0.5, text: slide.notes, font_size: 16, color: '@dk1', bold: false, format: { paragraphs: slide.notes_paragraphs } }
  const preview: Slide = { id: `notes-${slide.id}`, title: 'Notes page', background: master.background, notes: '', elements: [...master.elements.filter((element) => element !== body), notes] }
  return <div aria-label={`Notes page ${pageNumber}`}><SlideSurface slide={preview} width={auxiliary.width} height={auxiliary.height} pageNumber={pageNumber} design={{ theme: master.theme, masters: [], layouts: [] }} /></div>
}