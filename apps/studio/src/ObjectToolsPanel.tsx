import { useEffect, useRef, useState } from 'react'
import { Check, Plus, Trash2, RefreshCw, Merge, Split, ArrowUp, ArrowDown, Pipette, Image, WandSparkles, X, Settings } from 'lucide-react'
import { AislideClient, type DocumentSession } from '../../../packages/client/index.mjs'
import type { AislideDocument, Element, ChartKind, ChartSeries, ChartOptions, AxisOptions, TableOperation, TableCellStyle, VisualStyle, PathCommand, Gradient, ImageEditParams } from './types'
import { core } from './api'
import { Tool } from './Tool'
import { usePanelTask } from './DocumentSetupPanel'
import type { SegmentedImage } from './types'
import './object-tools.css'

export type ObjectToolsPanelProps = {
  session: DocumentSession
  element: Element
  slideId: string
  onDocument: (document: AislideDocument) => void
  onBusy: (busy: boolean) => void
}
type ChartElement = Extract<Element, { type: 'chart' }>
type TableElement = Extract<Element, { type: 'table' }>
type VisualElement = Exclude<Element, { type: 'chart' | 'table' }>
type ApplyElement = (element: Element | (() => Promise<Element>)) => Promise<void>
const client = new AislideClient(core)
const chartKinds: ChartKind[] = ['column', 'bar', 'line', 'pie', 'doughnut', 'area', 'scatter', 'stacked_column', 'stacked_bar', 'percent_stacked_column', 'percent_stacked_bar', 'combo', 'bubble', 'radar', 'radar_filled', 'column3d', 'bar3d', 'pie3d', 'funnel', 'waterfall', 'histogram', 'box_whisker', 'treemap', 'sunburst']
const typedChart = (kind: ChartKind) => ['histogram', 'box_whisker', 'treemap', 'sunburst'].includes(kind)
const labelOf = (value: string) => value.replaceAll('_', ' ')

export function PanelNumber({ label, value, onChange, min, max, step = 'any' }: { label: string; value: number | null | undefined; onChange: (value: number | undefined) => void; min?: number; max?: number; step?: number | 'any' }) {
  return <label>{label}<input type="number" aria-label={label} min={min} max={max} step={step} value={value ?? ''} onChange={(event) => onChange(event.target.value === '' ? undefined : event.target.valueAsNumber)} /></label>
}
export function PanelColor({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return <label>{label}<input aria-label={label} type="color" value={/^[a-f\d]{6}$/i.test(value) ? `#${value}` : '#000000'} onChange={(event) => onChange(event.target.value.slice(1).toUpperCase())} />{value.startsWith('@') && <small>{value}</small>}</label>
}
export function PanelCheck({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return <label className="object-tools-check"><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />{label}</label>
}
function elementPath(elements: Element[], id: string, prefix: string): string | undefined {
  for (const [index, element] of elements.entries()) {
    const path = `${prefix}/${index}`
    if (element.id === id) return path
    if (element.type === 'group') { const found = elementPath(element.children, id, `${path}/children`); if (found) return found }
  }
}

export function ObjectToolsPanel(props: ObjectToolsPanelProps) {
  const [reload, setReload] = useState(0)
  const [identity, setIdentity] = useState(props.session)
  if (identity !== props.session) { setIdentity(props.session); setReload((value) => value + 1) }
  return <ObjectToolsForm key={`${props.session.document.id}:${props.slideId}:${props.element.id}:${reload}`} {...props} onReload={() => setReload((value) => value + 1)} />
}
function ObjectToolsForm({ session, element, slideId, onDocument, onBusy, onReload }: ObjectToolsPanelProps & { onReload: () => void }) {
  const [revision, setRevision] = useState(session.revision)
  const [draft, setDraft] = useState(() => structuredClone(element))
  const [version, setVersion] = useState(0)
  const [aiBusy, setAiBusy] = useState(false)
  const task = usePanelTask(session, onBusy)
  const stale = revision !== session.revision
  function accept(document: AislideDocument, active: () => boolean) {
    if (!active()) return
    setRevision(document.revision)
    const slide = document.deck.slides.find((item) => item.id === slideId)
    const find = (elements: Element[]): Element | undefined => {
      for (const item of elements) { if (item.id === element.id) return item; if (item.type === 'group') { const found = find(item.children); if (found) return found } }
    }
    const current = slide && find(slide.elements)
    if (current) setDraft(current)
    setVersion((value) => value + 1)
    onDocument(document)
  }
  const apply: ApplyElement = (input) => task.run(async (active) => {
    const next = typeof input === 'function' ? await input() : input
    if (!active()) return
    if (next.type === 'chart') {
      const statistical = next.kind === 'histogram' || next.kind === 'box_whisker'
      if ((next.kind !== 'histogram' && !next.categories.length) || next.categories.length > 32 || !next.series.length || next.series.length > 6) throw new Error('Chart category or series count is invalid.')
      if (next.series.some((series) => series.values.length !== (statistical ? 0 : next.categories.length) || series.values.some((value) => !Number.isFinite(value)))) throw new Error('Every chart value must be a finite number; statistical charts use raw samples only.')
      for (const series of next.series) {
        for (const values of [series.bubble_sizes, series.error_bars?.plus, series.error_bars?.minus]) {
          if (values && (values.length !== next.categories.length || values.some((value) => !Number.isFinite(value) || value < 0))) throw new Error('Bubble sizes and error arrays must match the category count and contain finite nonnegative values.')
        }
      }
    }
    const document = session.document
    const slideIndex = document.deck.slides.findIndex((slide) => slide.id === slideId)
    const path = slideIndex < 0 ? undefined : elementPath(document.deck.slides[slideIndex].elements, element.id, `/deck/slides/${slideIndex}/elements`)
    if (!path) throw new Error('Selected object is no longer on this slide.')
    accept(await session.transact([{ op: 'replace', path, value: next }], { expectedRevision: revision }), active)
  }, revision)
  const table = (operations: TableOperation[]) => task.run(async (active) => {
    if (!operations.length) return
    accept(await session.editTable(slideId, { id: element.id, operations }, { expectedRevision: revision }), active)
  }, revision)
  return <section className="object-tools-panel" aria-label="Object tools" aria-busy={task.busy} onKeyDown={(event) => event.stopPropagation()}>
    <div className="object-tools-heading"><h2>Object tools</h2><Tool label="Reload object tools" disabled={task.busy || aiBusy} onClick={onReload}><RefreshCw /></Tool></div>
    {stale && <p role="alert">Document changed. Reload before applying.</p>}
    <fieldset disabled={task.busy || stale || aiBusy} key={`object-tools-${version}`}>
      {draft.type === 'table' && <TableTools element={draft} apply={table} />}
      {draft.type === 'chart' && <ChartTools element={draft} apply={apply} />}
      {draft.type === 'picture' && <PictureTools element={draft} apply={(params) => task.run(async (active) => {
        const image = await client.editImage({ base64: draft.base64, mime_type: draft.mime_type, params })
        if (!active()) return
        accept(await session.applyImageEdit(slideId, { id: draft.id, image }, { expectedRevision: revision }), active)
      }, revision)} />}
      {draft.type !== 'table' && draft.type !== 'chart' && <VisualTools element={draft} apply={apply} />}
      {['rect', 'polygon', 'shape', 'text'].includes(draft.type) && <SlideColorTools session={session} slideId={slideId} element={draft} revision={revision} task={task} apply={apply} />}
    </fieldset>
    {draft.type === 'picture' && <PictureAiTools key={`picture-ai-${version}`} session={session} element={draft} slideId={slideId} revision={revision} disabled={task.busy || stale} onDocument={document => accept(document, () => true)} onBusy={busy => { setAiBusy(busy); onBusy(busy) }} />}
    {task.error && <p role="alert">{task.error}</p>}
  </section>
}

function TableTools({ element, apply }: { element: TableElement; apply: (operations: TableOperation[]) => Promise<void> }) {
  const [row, setRow] = useState(0)
  const [column, setColumn] = useState(0)
  const [rowSpan, setRowSpan] = useState(1)
  const [colSpan, setColSpan] = useState(1)
  const [rows, setRows] = useState(() => structuredClone(element.rows))
  const [widths, setWidths] = useState(element.format?.column_widths?.values ?? element.rows[0].map(() => 1))
  const [heights, setHeights] = useState(element.format?.row_heights?.values ?? element.rows.map(() => 1))
  const selectedStyle = element.format?.cells?.find((cell) => cell.row === row && cell.column === column)?.style ?? {}
  const selection = `${row}:${column}`
  return <details open><summary>Table</summary>
    <div className="object-tools-grid"><label>Row<select value={row} onChange={(event) => setRow(Number(event.target.value))}>{rows.map((_, index) => <option key={index} value={index}>{index + 1}</option>)}</select></label><label>Column<select value={column} onChange={(event) => setColumn(Number(event.target.value))}>{rows[0].map((_, index) => <option key={index} value={index}>{index + 1}</option>)}</select></label>
      <PanelNumber label="Row span" min={1} max={rows.length - row} step={1} value={rowSpan} onChange={(value) => setRowSpan(value ?? 1)} /><PanelNumber label="Column span" min={1} max={rows[0].length - column} step={1} value={colSpan} onChange={(value) => setColSpan(value ?? 1)} />
    </div>
    <div className="object-tools-actions"><Tool label="Merge selected cells" onClick={() => void apply([{ op: 'merge', region: { row, column, row_span: rowSpan, col_span: colSpan } }])}><Merge /></Tool><Tool label="Split selected cell" onClick={() => void apply([{ op: 'split', row, column }])}><Split /></Tool>
      <Tool label="Insert table row" disabled={rows.length >= 12} onClick={() => void apply([{ op: 'insert_row', index: row + 1, values: rows[0].map(() => '') }])}><Plus /></Tool><Tool label="Delete table row" disabled={rows.length <= 1} onClick={() => void apply([{ op: 'remove_row', index: row }])}><Trash2 /></Tool><Tool label="Insert table column" disabled={rows[0].length >= 8} onClick={() => void apply([{ op: 'insert_column', index: column + 1, values: rows.map(() => '') }])}><Plus /></Tool><Tool label="Delete table column" disabled={rows[0].length <= 1} onClick={() => void apply([{ op: 'remove_column', index: column }])}><Trash2 /></Tool>
    </div>
    <details><summary>Cell data</summary><div className="object-tools-scroll" tabIndex={0} role="region" aria-label="Table cell grid"><table><tbody>{rows.map((cells, rowIndex) => <tr key={rowIndex}>{cells.map((value, columnIndex) => <td key={columnIndex}><input aria-label={`Cell ${rowIndex + 1}, ${columnIndex + 1}`} maxLength={200} value={value} onChange={(event) => setRows(rows.map((item, index) => index === rowIndex ? item.map((text, cell) => cell === columnIndex ? event.target.value : text) : item))} /></td>)}</tr>)}</tbody></table></div>
      <Tool label="Apply table data" onClick={() => void apply(rows.flatMap((cells, row) => cells.flatMap((text, column): TableOperation[] => text === element.rows[row][column] ? [] : [{ op: 'set_cell_text', row, column, text }])))}><Check /></Tool>
    </details>
    <details><summary>Relative dimensions</summary><div className="object-tools-grid">{widths.map((value, index) => <PanelNumber key={`column-${index}`} label={`Column ${index + 1} width`} min={0.01} value={value} onChange={(next) => setWidths(widths.map((item, position) => position === index ? next ?? 0 : item))} />)}{heights.map((value, index) => <PanelNumber key={`row-${index}`} label={`Row ${index + 1} height`} min={0.01} value={value} onChange={(next) => setHeights(heights.map((item, position) => position === index ? next ?? 0 : item))} />)}</div>
      <Tool label="Apply table dimensions" onClick={() => void apply([{ op: 'update_format', format: { ...element.format, column_widths: { unit: 'relative', values: widths }, row_heights: { unit: 'relative', values: heights } } }])}><Check /></Tool>
    </details>
    <CellStyleTools key={selection} style={selectedStyle} apply={(style) => apply([{ op: 'set_cell_style', row, column, style }])} />
  </details>
}
function CellStyleTools({ style, apply }: { style: TableCellStyle; apply: (style: TableCellStyle) => Promise<void> }) {
  const [draft, setDraft] = useState(() => structuredClone(style))
  return <details><summary>Cell style</summary><div className="object-tools-grid">
    <PanelColor label="Cell fill" value={draft.fill ?? 'FFFFFF'} onChange={(fill) => setDraft({ ...draft, fill })} />
    <PanelColor label="Cell text color" value={draft.text_style?.color ?? '000000'} onChange={(color) => setDraft({ ...draft, text_style: { ...draft.text_style, color } })} />
    <PanelNumber label="Cell font size" min={8} max={96} value={draft.text_style?.font_size} onChange={(font_size) => setDraft({ ...draft, text_style: { ...draft.text_style, font_size } })} />
    <label>Cell font<input maxLength={100} value={draft.text_style?.font_family ?? ''} onChange={(event) => setDraft({ ...draft, text_style: { ...draft.text_style, font_family: event.target.value || null } })} /></label>
    <label>Cell alignment<select value={draft.text_format?.alignment ?? 'left'} onChange={(event) => setDraft({ ...draft, text_format: { ...draft.text_format, alignment: event.target.value as 'left' } })}>{['left', 'center', 'right', 'justify'].map((value) => <option key={value}>{value}</option>)}</select></label>
    <label>Cell vertical alignment<select value={draft.vertical ?? 'top'} onChange={(event) => setDraft({ ...draft, vertical: event.target.value as 'top' })}>{['top', 'middle', 'bottom'].map((value) => <option key={value}>{value}</option>)}</select></label>
    <PanelColor label="Cell border color" value={draft.outline?.color ?? '000000'} onChange={(color) => setDraft({ ...draft, outline: { width: 1, ...draft.outline, color } })} />
    <PanelNumber label="Cell border width" min={0} max={20} value={draft.outline?.width ?? 0} onChange={(width) => setDraft({ ...draft, outline: { color: '000000', ...draft.outline, width: width ?? 0 } })} />
    {(['left', 'right', 'top', 'bottom'] as const).map((side) => <PanelNumber key={side} label={`Cell padding ${side}`} min={0} max={100} value={draft.padding?.[side] ?? 6} onChange={(value) => setDraft({ ...draft, padding: { left: 6, right: 6, top: 6, bottom: 6, ...draft.padding, [side]: value ?? 0 } })} />)}
    </div><div className="object-tools-actions">{(['bold', 'italic', 'underline'] as const).map((key) => <PanelCheck key={key} label={`Cell ${key}`} checked={Boolean(draft.text_style?.[key])} onChange={(value) => setDraft({ ...draft, text_style: { ...draft.text_style, [key]: value } })} />)}</div>
    <Tool label="Apply cell style" onClick={() => void apply(draft)}><Check /></Tool>
  </details>
}

function ChartTools(props: { element: ChartElement; apply: ApplyElement }) {
  return typedChart(props.element.kind) ? <TypedChartTools {...props} /> : <ClassicChartTools {...props} />
}

function TypedChartTools({ element, apply }: { element: ChartElement; apply: ApplyElement }) {
  const [draft, setDraft] = useState(() => structuredClone(element))
  const [samples, setSamples] = useState(() => element.options?.histogram ? [element.options.histogram.samples.join(', ')] : element.options?.box_whisker?.samples.map((group) => group.join(', ')) ?? [])
  const histogram = draft.options?.histogram
  const box = draft.options?.box_whisker
  const hierarchy = draft.options?.hierarchy
  const patchOptions = (patch: Partial<ChartOptions>) => setDraft({ ...draft, options: { ...draft.options, ...patch } })
  const patchSeries = (patch: Partial<ChartSeries>) => setDraft({ ...draft, series: [{ ...draft.series[0], ...patch }] })
  const parseSamples = (value: string) => {
    const tokens = value.trim().split(/[\s,;]+/)
    if (!value.trim() || tokens.length > 4096 || tokens.some((token) => !token || !Number.isFinite(Number(token)) || Math.abs(Number(token)) > 1e15)) throw new Error('Samples require finite numbers in +/-1e15, at most 4096 total; empty entries are not numbers.')
    return tokens.map(Number)
  }
  function remove(index: number) {
    setSamples(samples.filter((_, position) => index !== position))
    setDraft({ ...draft, categories: draft.categories.filter((_, position) => position !== index), series: [{ ...draft.series[0], values: draft.series[0].values.filter((_, position) => position !== index) }], options: { ...draft.options, ...(hierarchy ? { hierarchy: { ...hierarchy, paths: hierarchy.paths.filter((_, position) => position !== index) } } : {}) } })
  }
  return <details open><summary>Chart</summary>
    <label>Chart kind<select value={draft.kind} onChange={(event) => setDraft({ ...draft, kind: event.target.value as ChartKind, options: { ...draft.options, hierarchy: hierarchy ? { ...hierarchy, parent_labels: event.target.value === 'sunburst' ? undefined : hierarchy.parent_labels } : undefined } })}>{chartKinds.map((kind) => <option key={kind} value={kind} disabled={kind !== draft.kind && !(hierarchy && ['treemap', 'sunburst'].includes(kind))}>{labelOf(kind)}</option>)}</select></label>
    <label>Series name<input aria-label="Series 1 name" maxLength={80} value={draft.series[0].name} onChange={(event) => patchSeries({ name: event.target.value })} /></label>
    <PanelColor label="Series color" value={draft.series[0].color} onChange={(color) => patchSeries({ color })} />
    <label>Legend position<select value={draft.options?.legend ?? ''} onChange={(event) => patchOptions({ legend: (event.target.value || null) as ChartOptions['legend'] })}><option value="">Default</option>{['bottom', 'top', 'left', 'right', 'top_right', 'hidden'].map((value) => <option key={value} value={value}>{labelOf(value)}</option>)}</select></label>
    {(histogram || box) && <details open><summary>Raw samples</summary>{samples.map((value, index) => <fieldset key={index}><legend>{histogram ? 'Samples' : `Group ${index + 1}`}</legend>
      {box && <label>Group label<input aria-label={`Group ${index + 1} label`} maxLength={80} value={draft.categories[index]} onChange={(event) => setDraft({ ...draft, categories: draft.categories.map((label, position) => position === index ? event.target.value : label) })} /></label>}
      <label>Raw samples {index + 1}<textarea aria-label={`Raw samples ${index + 1}`} rows={4} maxLength={100000} value={value} onChange={(event) => setSamples(samples.map((group, position) => position === index ? event.target.value : group))} /></label>
      {box && <Tool label={`Delete sample group ${index + 1}`} disabled={samples.length <= 1} onClick={() => remove(index)}><Trash2 /></Tool>}
    </fieldset>)}{box && <Tool label="Add sample group" disabled={samples.length >= 32} onClick={() => { setSamples([...samples, '']); setDraft({ ...draft, categories: [...draft.categories, ''] }) }}><Plus /></Tool>}</details>}
    {histogram && <fieldset><legend>Binning</legend>
      <label>Bin rule<select aria-label="Bin rule" value={histogram.binning.rule} onChange={(event) => patchOptions({ histogram: { ...histogram, binning: event.target.value === 'count' ? { rule: 'count', count: 4 } : { rule: 'width', width: 1 } } })}><option value="count">Count</option><option value="width">Width</option></select></label>
      {histogram.binning.rule === 'count' ? <PanelNumber label="Bin count" min={1} max={128} step={1} value={histogram.binning.count} onChange={(count) => patchOptions({ histogram: { ...histogram, binning: { rule: 'count', count: count ?? 0 } } })} /> : <PanelNumber label="Bin width" min={0} max={1e15} value={histogram.binning.width} onChange={(width) => patchOptions({ histogram: { ...histogram, binning: { rule: 'width', width: width ?? 0 } } })} />}
      <label>Interval closed<select aria-label="Interval closed" value={histogram.interval_closed} onChange={(event) => patchOptions({ histogram: { ...histogram, interval_closed: event.target.value as 'left' | 'right' } })}><option value="left">Left</option><option value="right">Right</option></select></label>
      <PanelNumber label="Underflow threshold" value={histogram.underflow} onChange={(underflow) => patchOptions({ histogram: { ...histogram, underflow } })} />
      <PanelNumber label="Overflow threshold" value={histogram.overflow} onChange={(overflow) => patchOptions({ histogram: { ...histogram, overflow } })} />
    </fieldset>}
    {box && <fieldset><legend>Statistics</legend><label>Quartile method<select aria-label="Quartile method" value={box.quartile_method} onChange={(event) => patchOptions({ box_whisker: { ...box, quartile_method: event.target.value as 'inclusive' | 'exclusive' } })}><option value="inclusive">Inclusive</option><option value="exclusive">Exclusive</option></select></label>
      {(['mean_line', 'mean_marker', 'nonoutliers', 'outliers'] as const).map((key) => <PanelCheck key={key} label={labelOf(key)} checked={box[key]} onChange={(value) => patchOptions({ box_whisker: { ...box, [key]: value } })} />)}
    </fieldset>}
    {hierarchy && <details open><summary>Hierarchy</summary>
      <label>Hierarchy depth<select value={hierarchy.paths[0]?.length ?? 2} onChange={(event) => { const depth = Number(event.target.value); const paths = hierarchy.paths.map((path) => [...Array.from({ length: depth - 1 }, (_, index) => index < path.length - 1 ? path[index] : ''), path.at(-1)!]); patchOptions({ hierarchy: { ...hierarchy, paths } }) }}>{[2, 3, 4].map((depth) => <option key={depth} value={depth}>{depth}</option>)}</select></label>
      {draft.kind === 'treemap' && <label>Parent labels<select aria-label="Parent labels" value={hierarchy.parent_labels ?? ''} onChange={(event) => patchOptions({ hierarchy: { ...hierarchy, parent_labels: (event.target.value || undefined) as 'none' | 'banner' | 'overlapping' | undefined } })}><option value="">Default</option>{['none', 'banner', 'overlapping'].map((value) => <option key={value}>{value}</option>)}</select></label>}
      {hierarchy.paths.map((path, index) => <fieldset key={index}><legend>Leaf {index + 1}</legend>{path.map((label, level) => <label key={level}>Level {level + 1}<input aria-label={`Leaf ${index + 1} level ${level + 1}`} value={label} maxLength={80} onChange={(event) => setDraft({ ...draft, categories: draft.categories.map((category, position) => position === index && level === path.length - 1 ? event.target.value : category), options: { ...draft.options, hierarchy: { ...hierarchy, paths: hierarchy.paths.map((current, position) => position === index ? current.map((part, depth) => depth === level ? event.target.value : part) : current) } } })} /></label>)}
        <PanelNumber label={`Leaf ${index + 1} value`} min={0} max={1e15} value={draft.series[0].values[index]} onChange={(value) => patchSeries({ values: draft.series[0].values.map((current, position) => position === index ? value ?? 0 : current) })} />
        <Tool label={`Delete leaf ${index + 1}`} disabled={hierarchy.paths.length <= 1} onClick={() => remove(index)}><Trash2 /></Tool>
      </fieldset>)}
      <Tool label="Add hierarchy leaf" disabled={hierarchy.paths.length >= 32} onClick={() => setDraft({ ...draft, categories: [...draft.categories, ''], series: [{ ...draft.series[0], values: [...draft.series[0].values, 0] }], options: { ...draft.options, hierarchy: { ...hierarchy, paths: [...hierarchy.paths, hierarchy.paths[0].map(() => '')] } } })}><Plus /></Tool>
    </details>}
    <Tool label="Apply chart" onClick={() => void apply(async () => {
      const groups = samples.map(parseSamples)
      if (groups.reduce((total, group) => total + group.length, 0) > 4096) throw new Error('At most 4096 total samples.')
      return { ...draft, options: { ...draft.options, ...(histogram ? { histogram: { ...histogram, samples: groups[0] } } : {}), ...(box ? { box_whisker: { ...box, samples: groups } } : {}) } }
    })}><Check /></Tool>
  </details>
}

function ClassicChartTools({ element, apply }: { element: ChartElement; apply: ApplyElement }) {
  const [draft, setDraft] = useState(() => structuredClone(element))
  const [seriesIndex, setSeriesIndex] = useState(0)
  const [axis, setAxis] = useState<'primary_axis' | 'secondary_axis' | 'category_axis'>('primary_axis')
  const series = draft.series[seriesIndex]
  const extended = draft.kind === 'funnel' || draft.kind === 'waterfall'
  const axisOptions = draft.options?.[axis] ?? {}
  function options(patch: Partial<ChartOptions>) { setDraft({ ...draft, options: { ...draft.options, ...patch } }) }
  function updateSeries(patch: Partial<ChartSeries>) { setDraft({ ...draft, series: draft.series.map((item, index) => index === seriesIndex ? { ...item, ...patch } : item) }) }
  function axisChange(patch: Partial<AxisOptions>) { options({ [axis]: { ...axisOptions, ...patch } }) }
  function removeCategory(index: number) {
    const without = <Value,>(values: Value[]) => values.filter((_, position) => position !== index)
    setDraft({ ...draft, ...(draft.options?.waterfall_totals ? { options: { ...draft.options, waterfall_totals: draft.options.waterfall_totals.filter((position) => position !== index).map((position) => position > index ? position - 1 : position) } } : {}), categories: without(draft.categories), series: draft.series.map((item) => ({ ...item, values: without(item.values), ...(item.bubble_sizes ? { bubble_sizes: without(item.bubble_sizes) } : {}), ...(item.error_bars?.kind === 'custom' ? { error_bars: { ...item.error_bars, plus: item.error_bars.plus && without(item.error_bars.plus), minus: item.error_bars.minus && without(item.error_bars.minus) } } : {}) })) })
  }
  return <details open><summary>Chart</summary>
    <label>Chart kind<select value={draft.kind} onChange={(event) => setDraft({ ...draft, kind: event.target.value as ChartKind, ...(draft.options?.waterfall_totals && event.target.value !== 'waterfall' ? { options: { ...draft.options, waterfall_totals: undefined } } : {}) })}>{chartKinds.map((kind) => <option key={kind} value={kind} disabled={typedChart(kind)}>{labelOf(kind)}</option>)}</select></label>
    {draft.kind === 'waterfall' && <fieldset><legend>Total steps</legend>
      <PanelCheck label="All steps are changes" checked={draft.options?.waterfall_totals?.length === 0} onChange={(checked) => options({ waterfall_totals: checked ? [] : undefined })} />
      {draft.categories.map((_, index) => <PanelCheck key={index} label={`Step ${index + 1} is total`} checked={draft.options?.waterfall_totals?.includes(index) ?? false} onChange={(checked) => options({ waterfall_totals: checked ? [...(draft.options?.waterfall_totals ?? []), index].sort((left, right) => left - right) : (draft.options?.waterfall_totals ?? []).filter((position) => position !== index) })} />)}
    </fieldset>}
    {extended && <label>Legend position<select value={draft.options?.legend ?? ''} onChange={(event) => options({ legend: (event.target.value || null) as ChartOptions['legend'] })}><option value="">Default</option>{['bottom', 'top', 'left', 'right', 'top_right', 'hidden'].map((value) => <option key={value} value={value}>{labelOf(value)}</option>)}</select></label>}
    <details open><summary>Chart data</summary><div className="object-tools-scroll" role="region" aria-label="Chart data grid" tabIndex={0}><table><thead><tr><th scope="col">Category</th>{draft.series.map((item, index) => <th scope="col" key={index}><input aria-label={`Series ${index + 1} name`} value={item.name} maxLength={100} onChange={(event) => setDraft({ ...draft, series: draft.series.map((current, position) => position === index ? { ...current, name: event.target.value } : current) })} /></th>)}<th scope="col">Remove</th></tr></thead><tbody>{draft.categories.map((category, index) => <tr key={index}><td><input aria-label={`Category ${index + 1}`} value={category} maxLength={200} onChange={(event) => setDraft({ ...draft, categories: draft.categories.map((value, position) => position === index ? event.target.value : value) })} /></td>{draft.series.map((item, itemIndex) => <td key={itemIndex}><input aria-label={`Series ${itemIndex + 1} value ${index + 1}`} type="number" step="any" required value={Number.isFinite(item.values[index]) ? item.values[index] : ''} onChange={(event) => setDraft({ ...draft, series: draft.series.map((current, position) => position === itemIndex ? { ...current, values: current.values.map((value, row) => row === index ? event.target.valueAsNumber : value) } : current) })} /></td>)}<td><Tool label={`Delete category ${index + 1}`} disabled={draft.categories.length <= 1} onClick={() => removeCategory(index)}><Trash2 /></Tool></td></tr>)}</tbody></table></div>
      <div className="object-tools-actions"><Tool label="Add chart category" disabled={draft.categories.length >= 32} onClick={() => setDraft({ ...draft, categories: [...draft.categories, ''], series: draft.series.map((item) => ({ ...item, values: [...item.values, 0], ...(item.bubble_sizes ? { bubble_sizes: [...item.bubble_sizes, 1] } : {}), ...(item.error_bars?.kind === 'custom' ? { error_bars: { ...item.error_bars, plus: item.error_bars.plus && [...item.error_bars.plus, 0], minus: item.error_bars.minus && [...item.error_bars.minus, 0] } } : {}) })) })}><Plus /></Tool>
      <Tool label="Add chart series" disabled={extended || draft.series.length >= 6} onClick={() => setDraft({ ...draft, series: [...draft.series, { name: '', color: '007F73', values: draft.categories.map(() => 0), ...(draft.kind === 'bubble' ? { bubble_sizes: draft.categories.map(() => 1) } : {}) }] })}><Plus /></Tool></div>
    </details>
    <details><summary>Series</summary><label>Selected series<select value={seriesIndex} onChange={(event) => setSeriesIndex(Number(event.target.value))}>{draft.series.map((item, index) => <option key={index} value={index}>{item.name || `Series ${index + 1}`}</option>)}</select></label>
      <div className="object-tools-grid"><label>Series kind<select disabled={extended} value={series.kind ?? ''} onChange={(event) => updateSeries({ kind: (event.target.value || null) as ChartKind | null })}><option value="">Chart default</option>{chartKinds.filter((kind) => kind !== 'funnel' && kind !== 'waterfall').map((kind) => <option value={kind} key={kind}>{labelOf(kind)}</option>)}</select></label><label>Series axis<select disabled={extended} value={series.axis ?? ''} onChange={(event) => updateSeries({ axis: (event.target.value || null) as ChartSeries['axis'] })}><option value="">Default</option><option value="primary">Primary</option><option value="secondary">Secondary</option></select></label><PanelColor label="Series color" value={series.color} onChange={(color) => updateSeries({ color })} /></div>
      <Tool label="Delete selected series" disabled={draft.series.length <= 1} onClick={() => { setDraft({ ...draft, series: draft.series.filter((_, index) => index !== seriesIndex) }); setSeriesIndex(0) }}><Trash2 /></Tool>
      {(draft.kind === 'bubble' || series.kind === 'bubble' || series.bubble_sizes) && <fieldset><legend>Bubble sizes</legend>{draft.categories.map((_, index) => <PanelNumber key={index} label={`Bubble size ${index + 1}`} min={0.0001} value={series.bubble_sizes?.[index]} onChange={(value) => updateSeries({ bubble_sizes: draft.categories.map((_, position) => position === index ? value ?? 0 : series.bubble_sizes?.[position] ?? 1) })} />)}</fieldset>}
      <label>Trendline<select disabled={extended} value={series.trendline?.kind ?? ''} onChange={(event) => updateSeries({ trendline: event.target.value ? { ...series.trendline, kind: event.target.value as NonNullable<ChartSeries['trendline']>['kind'] } : null })}><option value="">None</option>{['linear', 'exponential', 'logarithmic', 'polynomial', 'power', 'moving_average'].map((kind) => <option value={kind} key={kind}>{labelOf(kind)}</option>)}</select></label>
      {series.trendline && <><div className="object-tools-grid">{(['order', 'period', 'intercept', 'forward', 'backward'] as const).map((key) => <PanelNumber key={key} label={`Trendline ${key}`} value={series.trendline?.[key]} onChange={(value) => updateSeries({ trendline: { ...series.trendline!, [key]: value } })} />)}</div>{(['display_equation', 'display_r_squared'] as const).map((key) => <PanelCheck key={key} label={labelOf(key)} checked={Boolean(series.trendline?.[key])} onChange={(value) => updateSeries({ trendline: { ...series.trendline!, [key]: value } })} />)}</>}
      <label>Error bars<select disabled={extended} value={series.error_bars?.kind ?? ''} onChange={(event) => updateSeries({ error_bars: event.target.value ? { ...series.error_bars, kind: event.target.value as NonNullable<ChartSeries['error_bars']>['kind'] } : null })}><option value="">None</option>{['fixed_value', 'percentage', 'standard_deviation', 'standard_error', 'custom'].map((kind) => <option value={kind} key={kind}>{labelOf(kind)}</option>)}</select></label>
      {series.error_bars && <><div className="object-tools-grid"><label>Error direction<select value={series.error_bars.direction ?? 'y'} onChange={(event) => updateSeries({ error_bars: { ...series.error_bars!, direction: event.target.value as 'x' | 'y' } })}><option>x</option><option>y</option></select></label><label>Error bar type<select value={series.error_bars.bar_type ?? 'both'} onChange={(event) => updateSeries({ error_bars: { ...series.error_bars!, bar_type: event.target.value as 'both' } })}>{['both', 'plus', 'minus'].map((value) => <option key={value}>{value}</option>)}</select></label><PanelNumber label="Error value" min={0} value={series.error_bars.value} onChange={(value) => updateSeries({ error_bars: { ...series.error_bars!, value } })} /></div>
      {series.error_bars.kind === 'custom' && draft.categories.map((_, index) => <div className="object-tools-grid" key={index}>{(['plus', 'minus'] as const).map((side) => <PanelNumber key={side} label={`Error ${side} ${index + 1}`} min={0} value={series.error_bars?.[side]?.[index]} onChange={(value) => updateSeries({ error_bars: { ...series.error_bars!, [side]: draft.categories.map((_, position) => position === index ? value ?? 0 : series.error_bars?.[side]?.[position] ?? 0) } })} />)}</div>)}</>}
    </details>
    <details hidden={extended}><summary>Axes and labels</summary><label>Axis<select value={axis} onChange={(event) => setAxis(event.target.value as typeof axis)}>{['primary_axis', 'secondary_axis', 'category_axis'].map((value) => <option key={value} value={value}>{labelOf(value)}</option>)}</select></label>
      <div className="object-tools-grid">{(['min', 'max', 'major_unit', 'minor_unit', 'log_base'] as const).map((key) => <PanelNumber key={key} label={`Axis ${labelOf(key)}`} value={axisOptions[key]} onChange={(value) => axisChange({ [key]: value })} />)}</div><PanelCheck label="Reverse axis" checked={Boolean(axisOptions.reverse)} onChange={(reverse) => axisChange({ reverse })} /><label>Axis number format<input maxLength={100} value={axisOptions.number_format ?? ''} onChange={(event) => axisChange({ number_format: event.target.value || null })} /></label>
      <label>Legend position<select value={draft.options?.legend ?? ''} onChange={(event) => options({ legend: (event.target.value || null) as ChartOptions['legend'] })}><option value="">Default</option>{['bottom', 'top', 'left', 'right', 'top_right', 'hidden'].map((value) => <option key={value} value={value}>{labelOf(value)}</option>)}</select></label>
      {(['show_value', 'show_category_name', 'show_series_name', 'show_percent'] as const).map((key) => <PanelCheck key={key} label={labelOf(key)} checked={Boolean(draft.options?.data_labels?.[key])} onChange={(value) => options({ data_labels: { ...draft.options?.data_labels, [key]: value } })} />)}
      <label>Label position<select value={draft.options?.data_labels?.position ?? ''} onChange={(event) => options({ data_labels: { ...draft.options?.data_labels, position: (event.target.value || null) as 'center' | null } })}><option value="">Default</option>{['center', 'inside_end', 'outside_end', 'best_fit'].map((value) => <option key={value} value={value}>{labelOf(value)}</option>)}</select></label><label>Label number format<input maxLength={100} value={draft.options?.data_labels?.number_format ?? ''} onChange={(event) => options({ data_labels: { ...draft.options?.data_labels, number_format: event.target.value || null } })} /></label>
    </details>
    <Tool label="Apply chart" onClick={() => void apply(draft)}><Check /></Tool>
  </details>
}

function PictureTools({ element, apply }: { element: Extract<Element, { type: 'picture' }>; apply: (params: ImageEditParams) => Promise<void> }) {
  const [params, setParams] = useState<ImageEditParams>({ brightness: 0, contrast: 1, saturation: 1, grayscale: false, format: { kind: 'png' } })
  const rgb = (value: string): [number, number, number] => [parseInt(value.slice(0, 2), 16), parseInt(value.slice(2, 4), 16), parseInt(value.slice(4, 6), 16)]
  const hex = (value: [number, number, number]) => value.map((channel) => channel.toString(16).padStart(2, '0')).join('')
  return <details open><summary>Picture</summary><output>{element.mime_type === 'image/png' ? 'PNG' : 'JPEG'}</output>
    <div className="object-tools-grid">{(['brightness', 'contrast', 'saturation'] as const).map((key) => <PanelNumber key={key} label={labelOf(key)} min={key === 'brightness' ? -1 : 0} max={key === 'brightness' ? 1 : 4} step={0.05} value={params[key]} onChange={(value) => setParams({ ...params, [key]: value })} />)}</div>
    <PanelCheck label="Grayscale" checked={Boolean(params.grayscale)} onChange={(grayscale) => setParams({ ...params, grayscale })} />
    <PanelCheck label="Remove selected color" checked={Boolean(params.background_key)} onChange={(checked) => setParams({ ...params, background_key: checked ? { color: [255, 255, 255], tolerance: 0 } : null })} />
    {params.background_key && <div className="object-tools-grid"><PanelColor label="Selected color" value={hex(params.background_key.color)} onChange={(value) => setParams({ ...params, background_key: { ...params.background_key!, color: rgb(value) } })} /><PanelNumber label="Color tolerance" min={0} max={255} value={params.background_key.tolerance} onChange={(value) => setParams({ ...params, background_key: { ...params.background_key!, tolerance: value ?? 0 } })} /></div>}
    <PanelNumber label="Resize longest side" min={1} max={4096} step={1} value={params.resize_longest_side} onChange={(resize_longest_side) => setParams({ ...params, resize_longest_side })} />
    <label>Image output<select value={params.format?.kind ?? 'png'} onChange={(event) => setParams({ ...params, format: event.target.value === 'png' ? { kind: 'png' } : { kind: 'jpeg', quality: 90, matte: null } })}><option value="png">PNG</option><option value="jpeg">JPEG</option></select></label>
    {params.format?.kind === 'jpeg' && <><PanelNumber label="JPEG quality" min={1} max={100} step={1} value={params.format.quality} onChange={(quality) => setParams({ ...params, format: { ...params.format as Extract<NonNullable<ImageEditParams['format']>, { kind: 'jpeg' }>, quality: quality ?? 90 } })} /><PanelCheck label="Use JPEG matte" checked={Boolean(params.format.matte)} onChange={(checked) => setParams({ ...params, format: { ...params.format as Extract<NonNullable<ImageEditParams['format']>, { kind: 'jpeg' }>, matte: checked ? [255, 255, 255] : null } })} />{params.format.matte && <PanelColor label="JPEG matte" value={hex(params.format.matte)} onChange={(value) => setParams({ ...params, format: { ...params.format as Extract<NonNullable<ImageEditParams['format']>, { kind: 'jpeg' }>, matte: rgb(value) } })} />}</>}
    <Tool label="Apply picture edit" onClick={() => void apply(params)}><Check /></Tool>
  </details>
}

function VisualTools({ element, apply }: { element: VisualElement; apply: ApplyElement }) {
  const [visual, setVisual] = useState<VisualStyle>(() => structuredClone(element.visual ?? {}))
  const [rotation, setRotation] = useState(element.type === 'shape' ? element.rotation : visual.rotation ?? 0)
  const filled = element.type === 'rect' || element.type === 'shape' || element.type === 'polygon'
  const gradient = visual.gradient
  function update(patch: Partial<VisualStyle>) { setVisual({ ...visual, ...patch }) }
  function changeGradient(next: Gradient) { update({ gradient: next, opacity: null }) }
  function gradientStop(index: number, patch: Partial<Gradient['stops'][number]>) { if (gradient) changeGradient({ ...gradient, stops: gradient.stops.map((stop, position) => position === index ? { ...stop, ...patch } : stop) }) }
  return <details open><summary>Visual</summary>
    {element.type !== 'connector' && <><PanelNumber label="Rotation" min={-360} max={360} value={rotation} onChange={(value) => setRotation(value ?? 0)} /><div className="object-tools-actions"><PanelCheck label="Flip horizontally" checked={Boolean(visual.flip_h)} onChange={(flip_h) => update({ flip_h })} /><PanelCheck label="Flip vertically" checked={Boolean(visual.flip_v)} onChange={(flip_v) => update({ flip_v })} /></div></>}
    <PanelCheck label="Hidden" checked={Boolean(visual.hidden)} onChange={(hidden) => update({ hidden })} /><PanelCheck label="Locked" checked={Boolean(visual.locked)} onChange={(locked) => update({ locked })} />
    {(filled || element.type === 'picture') && !gradient && <PanelNumber label="Opacity" min={0} max={1} step={0.05} value={visual.opacity} onChange={(opacity) => update({ opacity })} />}
    {filled && <details><summary>Gradient</summary><label>Gradient kind<select value={gradient?.kind ?? ''} onChange={(event) => {
      const kind = event.target.value
      if (!kind) { update({ gradient: null }); return }
      const stops = gradient?.stops ?? [{ offset: 0, color: element.fill === 'none' ? 'FFFFFF' : element.fill, opacity: 1 }, { offset: 1, color: 'FFFFFF', opacity: 1 }]
      changeGradient(kind === 'linear' ? { kind, angle: 0, stops } : { kind: 'radial', center: [0.5, 0.5], stops })
    }}><option value="">None</option><option value="linear">Linear</option><option value="radial">Radial</option></select></label>
      {gradient?.kind === 'linear' && <PanelNumber label="Gradient angle" min={0} max={359.99} value={gradient.angle} onChange={(value) => changeGradient({ ...gradient, angle: value ?? 0 })} />}
      {gradient?.kind === 'radial' && <div className="object-tools-grid">{([0, 1] as const).map((axis) => <PanelNumber key={axis} label={`Gradient center ${axis === 0 ? 'x' : 'y'}`} min={0} max={1} value={gradient.center[axis]} onChange={(value) => changeGradient({ ...gradient, center: axis === 0 ? [value ?? 0.5, gradient.center[1]] : [gradient.center[0], value ?? 0.5] })} />)}</div>}
      {gradient?.stops.map((stop, index) => <fieldset key={index}><legend>Stop {index + 1}</legend><PanelColor label={`Stop ${index + 1} color`} value={stop.color} onChange={(color) => gradientStop(index, { color })} /><div className="object-tools-grid"><PanelNumber label={`Stop ${index + 1} position`} min={0} max={1} value={stop.offset} onChange={(offset) => gradientStop(index, { offset: offset ?? 0 })} /><PanelNumber label={`Stop ${index + 1} opacity`} min={0} max={1} value={stop.opacity} onChange={(opacity) => gradientStop(index, { opacity: opacity ?? 1 })} /></div><Tool label={`Delete gradient stop ${index + 1}`} disabled={gradient.stops.length <= 2} onClick={() => changeGradient({ ...gradient, stops: gradient.stops.filter((_, position) => position !== index) })}><Trash2 /></Tool></fieldset>)}
      {gradient && <Tool label="Add gradient stop" disabled={gradient.stops.length >= 16} onClick={() => changeGradient({ ...gradient, stops: [...gradient.stops, { offset: 1, color: 'FFFFFF', opacity: 1 }] })}><Plus /></Tool>}
    </details>}
    <details><summary>Effects</summary>
      <PanelCheck label="Shadow" checked={Boolean(visual.shadow)} onChange={(enabled) => update({ shadow: enabled ? { color: '000000', opacity: 0.3, blur: 8, distance: 4, angle: 45 } : null })} />
      {visual.shadow && <><PanelColor label="Shadow color" value={visual.shadow.color} onChange={(color) => update({ shadow: { ...visual.shadow!, color } })} /><div className="object-tools-grid">{(['opacity', 'blur', 'distance', 'angle'] as const).map((key) => <PanelNumber key={key} label={`Shadow ${key}`} min={0} max={key === 'opacity' ? 1 : key === 'angle' ? 359.99 : key === 'distance' ? 200 : 100} value={visual.shadow?.[key]} onChange={(value) => update({ shadow: { ...visual.shadow!, [key]: value ?? 0 } })} />)}</div></>}
      <PanelCheck label="Glow" checked={Boolean(visual.glow)} onChange={(enabled) => update({ glow: enabled ? { color: 'FFFFFF', opacity: 0.5, radius: 5 } : null })} />
      {visual.glow && <><PanelColor label="Glow color" value={visual.glow.color} onChange={(color) => update({ glow: { ...visual.glow!, color } })} /><div className="object-tools-grid">{(['opacity', 'radius'] as const).map((key) => <PanelNumber key={key} label={`Glow ${key}`} min={0} max={key === 'opacity' ? 1 : 100} value={visual.glow?.[key]} onChange={(value) => update({ glow: { ...visual.glow!, [key]: value ?? 0 } })} />)}</div></>}
      <PanelNumber label="Soft edge" min={0} max={100} value={visual.soft_edge} onChange={(soft_edge) => update({ soft_edge })} />
      <PanelCheck label="Reflection" checked={Boolean(visual.reflection)} onChange={(enabled) => update({ reflection: enabled ? { blur: 0, distance: 0, start_opacity: 0.5, end_opacity: 0, end_position: 1 } : null })} />
      {visual.reflection && <div className="object-tools-grid">{(['blur', 'distance', 'start_opacity', 'end_opacity', 'end_position'] as const).map((key) => <PanelNumber key={key} label={`Reflection ${labelOf(key)}`} min={0} max={key === 'blur' ? 100 : key === 'distance' ? 200 : 1} value={visual.reflection?.[key]} onChange={(value) => update({ reflection: { ...visual.reflection!, [key]: value ?? 0 } })} />)}</div>}
    </details>
    {element.type === 'picture' && <label>Picture mask<select value={visual.picture_mask ?? ''} onChange={(event) => update({ picture_mask: (event.target.value || null) as VisualStyle['picture_mask'] })}><option value="">None</option>{['ellipse', 'round_rect', 'diamond', 'hexagon'].map((value) => <option value={value} key={value}>{labelOf(value)}</option>)}</select></label>}
    {(element.type === 'text' || element.type === 'shape') && <label>Text warp<select value={visual.text_warp ?? ''} onChange={(event) => update({ text_warp: (event.target.value || null) as VisualStyle['text_warp'] })}><option value="">None</option>{['arch_up', 'arch_down', 'wave1', 'wave2', 'inflate', 'deflate', 'slant_up', 'slant_down'].map((value) => <option value={value} key={value}>{labelOf(value)}</option>)}</select></label>}
    {element.type === 'shape' && ['roundRect', 'chevron', 'triangle'].includes(element.preset) && <PanelNumber label="Shape adjustment" min={0} max={element.preset === 'roundRect' ? 50000 : 100000} step={1} value={visual.adjustments?.[0]?.value} onChange={(value) => update({ adjustments: value === undefined ? [] : [{ name: 'adj', value }] })} />}
    <Tool label="Apply visual style" onClick={() => {
      const nextVisual = { ...visual }
      if (element.type !== 'shape' && element.type !== 'connector') nextVisual.rotation = rotation
      const next = { ...element, visual: nextVisual }
      if (next.type === 'shape') next.rotation = rotation
      if (filled && gradient && 'fill' in next) next.fill = gradient.stops[0].color
      void apply(next)
    }}><Check /></Tool>
    {element.type === 'polygon' && <PathTools element={element} apply={apply} />}
  </details>
}

function PathTools({ element, apply }: { element: Extract<Element, { type: 'polygon' }>; apply: ApplyElement }) {
  const [commands, setCommands] = useState<PathCommand[]>(() => structuredClone(element.visual?.path?.commands ?? [...element.points.map((point, index): PathCommand => ({ op: index === 0 ? 'move' : 'line', point })), { op: 'close' }]))
  function replace(index: number, next: PathCommand) { setCommands(commands.map((command, position) => position === index ? next : command)) }
  return <details><summary>Path vertices</summary>{commands.map((command, index) => command.op === 'close' ? null : <fieldset key={index}><legend>Vertex {index + 1}</legend>
    {index > 0 && <label>Vertex {index + 1} command<select value={command.op} onChange={(event) => {
      const op = event.target.value
      replace(index, op === 'cubic' ? { op, point: command.point, control1: command.point, control2: command.point } : op === 'quadratic' ? { op, point: command.point, control: command.point } : { op: 'line', point: command.point })
    }}><option value="line">Line</option><option value="quadratic">Quadratic</option><option value="cubic">Cubic</option></select></label>}
    {(['point', 'control', 'control1', 'control2'] as const).map((key) => {
      if (!(key in command)) return null
      const point = (command as unknown as Record<string, [number, number]>)[key]
      return <div className="object-tools-grid" key={key}>{([0, 1] as const).map((axis) => <PanelNumber key={axis} label={`Vertex ${index + 1} ${key} ${axis === 0 ? 'x' : 'y'}`} min={0} max={1} value={point[axis]} onChange={(value) => replace(index, { ...command, [key]: axis === 0 ? [value ?? 0, point[1]] : [point[0], value ?? 0] })} />)}</div>
    })}
    {index > 0 && <div className="object-tools-actions"><Tool label={`Delete vertex ${index + 1}`} disabled={commands.length <= 4} onClick={() => setCommands(commands.filter((_, position) => position !== index))}><Trash2 /></Tool><Tool label={`Move vertex ${index + 1} up`} disabled={index <= 1} onClick={() => { const next = [...commands]; [next[index - 1], next[index]] = [next[index], next[index - 1]]; setCommands(next) }}><ArrowUp /></Tool><Tool label={`Move vertex ${index + 1} down`} disabled={index >= commands.length - 2} onClick={() => { const next = [...commands]; [next[index + 1], next[index]] = [next[index], next[index + 1]]; setCommands(next) }}><ArrowDown /></Tool></div>}
  </fieldset>)}
    <Tool label="Add path vertex" disabled={commands.length >= 256} onClick={() => setCommands([...commands.slice(0, -1), { op: 'line', point: [0.5, 0.5] }, { op: 'close' }])}><Plus /></Tool>
    <Tool label="Apply path vertices" onClick={() => void apply(() => client.editVector({ element, path: { commands } }))}><Check /></Tool>
  </details>
}

function SlideColorTools({ session, slideId, element, revision, task, apply }: { session: DocumentSession; slideId: string; element: Element; revision: number; task: ReturnType<typeof usePanelTask>; apply: ApplyElement }) {
  const [preview, setPreview] = useState('')
  const [point, setPoint] = useState({ x: 0, y: 0 })
  const [color, setColor] = useState('000000')
  const [warnings, setWarnings] = useState<string[]>([])
  const fields = (['fill', 'stroke', 'color'] as const).filter(field => field in element)
  const [field, setField] = useState(fields[0])
  const deck = session.document.deck
  const swatches = [...new Set(deck.slides.find(slide => slide.id === slideId)?.elements.flatMap(item => (['fill', 'stroke', 'color'] as const).flatMap(key => key in item ? [String((item as unknown as Record<string, unknown>)[key])] : [])).filter(value => /^[a-f\d]{6}$/i.test(value)))].slice(0, 12)
  const validPoint = Number.isInteger(point.x) && Number.isInteger(point.y) && point.x >= 0 && point.y >= 0 && point.x < deck.width && point.y < deck.height
  function sample(x: number, y: number) {
    setPoint({ x, y })
    return task.run(async active => {
      const result = await session.sampleSlidePixel(slideId, x, y)
      if (active()) { setColor(result.color); setWarnings(result.warnings.map(warning => warning.message)) }
    }, revision)
  }
  return <details><summary>Slide eyedropper</summary>
    <Tool label="Load slide color preview" onClick={() => void task.run(async active => {
      const page = deck.slides.findIndex(slide => slide.id === slideId)
      const result = await session.exportStatic({ format: 'png', page_indices: [page], scale: 0.5 })
      if (active()) { setPreview(`data:image/png;base64,${result.files[0].base64}`); setWarnings(result.warnings.map(warning => warning.message)) }
    }, revision)}><Image /></Tool>
    {preview && <button type="button" className="slide-color-preview" aria-label="Sample slide preview" style={{ aspectRatio: `${deck.width} / ${deck.height}` }} onClick={event => {
      if (event.detail === 0) { if (validPoint) void sample(point.x, point.y); return }
      const bounds = event.currentTarget.getBoundingClientRect()
      void sample(Math.max(0, Math.min(deck.width - 1, Math.floor((event.clientX - bounds.left) / bounds.width * deck.width))), Math.max(0, Math.min(deck.height - 1, Math.floor((event.clientY - bounds.top) / bounds.height * deck.height))))
    }}><img src={preview} alt="Current slide color preview" draggable={false} /></button>}
    <div className="object-tools-grid"><PanelNumber label="Sample X" min={0} max={deck.width - 1} step={1} value={point.x} onChange={x => setPoint({ ...point, x: x ?? -1 })} /><PanelNumber label="Sample Y" min={0} max={deck.height - 1} step={1} value={point.y} onChange={y => setPoint({ ...point, y: y ?? -1 })} /></div>
    <Tool label="Sample slide pixel" disabled={!validPoint} onClick={() => void sample(point.x, point.y)}><Pipette /></Tool>
    <div className="object-tools-actions" role="group" aria-label="Slide color swatches">{swatches.map(value => <button type="button" key={value} aria-label={`Use color ${value}`} title={value} style={{ width: 44, height: 44, backgroundColor: `#${value}` }} onClick={() => setColor(value)} />)}</div>
    <PanelColor label="Sampled color" value={color} onChange={setColor} />
    <label>Color HEX<input aria-label="Color HEX" value={color} maxLength={6} pattern="[A-Fa-f0-9]{6}" onChange={event => setColor(event.target.value.toUpperCase())} /></label>
    <label>Color property<select value={field} onChange={event => setField(event.target.value as typeof field)}>{fields.map(field => <option key={field} value={field}>{labelOf(field)}</option>)}</select></label>
    <Tool label="Apply sampled color" disabled={!/^[a-f\d]{6}$/i.test(color)} onClick={() => void apply(async () => {
      if (field === 'color' && (element.type === 'text' || element.type === 'shape') && element.text) return client.formatTextElement({ element, start: 0, end: Array.from(element.text).length, style: { color } })
      const next = { ...element, [field]: color } as Element
      if (field === 'fill' && 'visual' in next && next.visual?.gradient) next.visual = { ...next.visual, gradient: null }
      return next
    })}><Check /></Tool>
    {warnings.map((warning, index) => <p role="status" key={index}>{warning}</p>)}
  </details>
}

function PictureAiTools({ session, element, slideId, revision, disabled, onDocument, onBusy }: ObjectToolsPanelProps & { element: Extract<Element, { type: 'picture' }>; revision: number; disabled: boolean }) {
  const [candidate, setCandidate] = useState<SegmentedImage | null>(null)
  const [working, setWorking] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const pending = useRef<AbortController | null>(null)
  const mounted = useRef(true)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; pending.current?.abort() } }, [session, element.id])
  const busy = working || disabled || session.busy
  const blocked = element.visual?.locked || element.visual?.hidden
  async function run(action: (signal: AbortSignal) => Promise<void>) {
    if (busy || pending.current) return
    const controller = new AbortController(); pending.current = controller
    setWorking(true); onBusy(true); setError(''); setMessage('')
    try { await action(controller.signal) }
    catch (reason) { if (mounted.current && !controller.signal.aborted) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { pending.current = null; if (mounted.current) setWorking(false); onBusy(false) }
  }
  return <details open aria-label="Local AI background removal"><summary>Local AI background removal</summary>
    <div className="object-tools-actions"><button type="button" disabled={busy || blocked} onClick={() => { setCandidate(null); void run(async signal => {
      const result = await client.segmentImage({ base64: element.base64, mime_type: element.mime_type }, { signal })
      if (signal.aborted || session.revision !== revision) return
      setCandidate(result); setMessage('Candidate ready. Inspect foreground edges before applying.')
    }) }}><WandSparkles size={18} aria-hidden="true" />Generate cutout</button>
      <button type="button" disabled={busy} onClick={() => void run(async signal => { const status = await client.segmentationStatus({ signal }); if (!signal.aborted) setMessage(status.message) })}><Settings size={18} aria-hidden="true" />Check image AI setup</button></div>
    {candidate && <section aria-label="AI image review"><div className="object-tools-grid">{[{ label: 'Original image', image: element }, { label: 'AI cutout candidate', image: candidate.image }].map(({ label, image }) => <figure key={label} style={{ margin: 0, minWidth: 0 }}><figcaption>{label}</figcaption><img alt={label} src={`data:${image.mime_type};base64,${image.base64}`} style={{ width: '100%', height: 160, objectFit: 'contain', background: 'repeating-conic-gradient(#ddd 0% 25%, #fff 0% 50%) 0 / 16px 16px' }} /></figure>)}</div>
      <small>{candidate.provenance.model} · {candidate.provenance.elapsed_ms} ms · Local CPU</small>
      <div className="object-tools-actions"><button type="button" disabled={busy || blocked} onClick={() => void run(async signal => {
        const updated = await session.applyImageEdit(slideId, { id: element.id, image: candidate.image }, { expectedRevision: revision, signal })
        if (signal.aborted) return
        onDocument(updated)
      })}><Check size={18} aria-hidden="true" />Apply AI cutout</button><button type="button" disabled={busy} onClick={() => { setCandidate(null); setMessage('Candidate discarded') }}><X size={18} aria-hidden="true" />Cancel cutout</button></div>
    </section>}
    {working && <button type="button" onClick={() => { pending.current?.abort(); setCandidate(null); setMessage('Cancellation requested. Waiting for native inference to finish; its result will be discarded.') }}><X size={18} aria-hidden="true" />Cancel image AI</button>}
    <p role="status">{message}</p>{error && <p role="alert">{error}</p>}
    <details><summary>Model setup and terms</summary><p>U²-NetP is optional and not bundled. Run node tools/local-image-model-setup.mjs --accept-model-and-dataset-terms only after reviewing the model and DUTS dataset terms. A preinstalled verified model can be supplied with --file.</p><p>Inference does not download files. CPU inference cannot be interrupted in-process; cancellation discards the result. Saliency is not a guaranteed object or hair mask.</p></details>
  </details>
}