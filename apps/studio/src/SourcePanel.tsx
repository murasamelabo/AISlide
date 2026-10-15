import { useEffect, useRef, useState } from 'react'
import { Check, FileUp, LoaderCircle, Paperclip } from 'lucide-react'
import { AislideClient } from '../../../packages/client/index.mjs'
import { core, fileBase64 } from './api'
import type { ChartData, DataReport, SourceDocument, SourceFormat } from './types'

const client = new AislideClient(core)
export function SourcePanel({ sources, onApply, onAttach, onBusy }: { sources: SourceDocument[]; onApply: (result: DataReport, source: SourceDocument) => Promise<void>; onAttach: (source: SourceDocument) => Promise<void>; onBusy: (busy: boolean) => void }) {
  const [source, setSource] = useState<SourceDocument | null>(sources[0] ?? null)
  const [title, setTitle] = useState('Source-bound report')
  const [period, setPeriod] = useState('Provided dataset')
  const [tableIndex, setTableIndex] = useState(0)
  const [category, setCategory] = useState(0)
  const [values, setValues] = useState<number[]>([1])
  const [rowStart, setRowStart] = useState(1)
  const [rowCount, setRowCount] = useState(Math.min(32, sources[0]?.tables[0]?.rows.length ?? 3))
  const [kind, setKind] = useState<ChartData['kind']>('column')
  const [ocr, setOcr] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const running = useRef<AbortController | null>(null)
  const table = source?.tables[tableIndex]
  useEffect(() => () => running.current?.abort(), [])
  async function perform(action: (signal: AbortSignal) => Promise<void>) {
    if (running.current) return
    const controller = new AbortController(); running.current = controller
    setBusy(true); onBusy(true); setError('')
    try { await action(controller.signal) } catch (reason) { if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { running.current = null; setBusy(false); onBusy(false) }
  }
  function choose(next: SourceDocument, index = 0) {
    setSource(next); setTableIndex(index); setCategory(0)
    setValues(next.tables[index]?.columns.length > 1 ? [1] : [])
    setRowStart(1); setRowCount(Math.min(32, next.tables[index]?.rows.length ?? 0))
  }
  return <form className="source-form" onSubmit={(event) => {
    event.preventDefault()
    if (!source || !table) return
    void perform(async (signal) => {
      const result = await client.dataReport(source, { title, period, table_index: tableIndex, category_column: category, value_columns: values, row_start: rowStart - 1, row_count: rowCount, chart_kind: kind }, { signal })
      if (!signal.aborted) await onApply(result, source)
    })
  }}>
    <div className="source-toolbar">
      <label className="secondary upload-label"><FileUp size={16} />Open source<input aria-label="Source file" type="file" accept=".csv,.json,.xlsx,.md,.txt,.pdf,.png,.jpg,.jpeg" disabled={busy} onChange={(event) => {
        const file = event.target.files?.[0]; event.target.value = ''
        if (!file) return
        void perform(async (signal) => {
          if (file.size > 2 * 1024 * 1024) throw new Error('Source must be at most 2 MiB')
          const formats: Record<string, SourceFormat> = { csv: 'csv', json: 'json', xlsx: 'xlsx', md: 'markdown', txt: 'text', pdf: 'pdf', png: 'png', jpg: 'jpeg', jpeg: 'jpeg' }
          const format = formats[file.name.split('.').at(-1)?.toLowerCase() ?? '']
          if (!format) throw new Error('Unsupported source file format')
          const next = await client.ingest({ name: file.name, format, base64: await fileBase64(file), ocr: ocr && (format === 'png' || format === 'jpeg') }, { signal })
          if (!signal.aborted) choose(next)
        })
      }} /></label>
      <label className="checkbox"><input type="checkbox" checked={ocr} disabled={busy} onChange={(event) => setOcr(event.target.checked)} />Local image OCR</label>
      {sources.length > 0 && <label className="field">Attached source<select aria-label="Attached source" value={sources.some((entry) => entry.id === source?.id) ? source?.id : ''} disabled={busy} onChange={(event) => { const next = sources.find((entry) => entry.id === event.target.value); if (next) choose(next) }}><option value="" disabled>Selected file</option>{sources.map((entry) => <option value={entry.id} key={entry.id}>{entry.name}</option>)}</select></label>}
    </div>
    {source && <>
      <div className="source-identity"><strong>{source.name}</strong><span>{source.byte_length.toLocaleString()} bytes</span><code>SHA-256 {source.sha256}</code></div>
      {table ? <>
        <fieldset disabled={busy} className="source-fields">
          <label className="field">Report title<input aria-label="Report title" required maxLength={120} value={title} onChange={(event) => setTitle(event.target.value)} /></label>
          <label className="field">Period<input aria-label="Period" maxLength={80} value={period} onChange={(event) => setPeriod(event.target.value)} /></label>
          <label className="field">Source table<select aria-label="Source table" value={tableIndex} onChange={(event) => choose(source, Number(event.target.value))}>{source.tables.map((entry, index) => <option key={index} value={index}>{entry.name}</option>)}</select></label>
          <label className="field">Category column<select aria-label="Category column" value={category} onChange={(event) => setCategory(Number(event.target.value))}>{table.columns.map((column, index) => <option key={index} value={index}>{column}</option>)}</select></label>
          <fieldset className="source-value-columns"><legend>Numeric columns</legend>{table.columns.map((column, index) => <label key={index} className="checkbox"><input type="checkbox" checked={values.includes(index)} onChange={(event) => setValues(event.target.checked ? [...values, index] : values.filter((value) => value !== index))} />{column}</label>)}</fieldset>
          <div className="inline-fields"><label className="field">First row<input aria-label="First row" type="number" min={1} max={Math.max(1, table.rows.length)} required value={rowStart} onChange={(event) => setRowStart(event.currentTarget.valueAsNumber)} /></label><label className="field">Row count<input aria-label="Row count" type="number" min={1} max={32} required value={rowCount} onChange={(event) => setRowCount(event.currentTarget.valueAsNumber)} /></label></div>
          <label className="field">Chart style<select value={kind} onChange={(event) => setKind(event.target.value as ChartData['kind'])}><option value="column">Column</option><option value="bar">Bar</option><option value="line">Line</option></select></label>
        </fieldset>
        <div className="source-preview" tabIndex={0} aria-label="Source data preview"><table><thead><tr>{table.columns.map((column) => <th key={column}>{column}</th>)}</tr></thead><tbody>{table.rows.slice(Math.max(0, rowStart - 1), Math.max(0, rowStart - 1) + 8).map((row, index) => <tr key={index}>{row.map((value, column) => <td key={column}>{value === null ? '(missing)' : String(value)}</td>)}</tr>)}</tbody></table></div>
      </> : <pre className="source-text">{source.text || `${source.raster?.width ?? 0} x ${source.raster?.height ?? 0} pixels`}</pre>}
      <ul className="source-warnings">{source.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>
    </>}
    {error && <p className="error" role="alert">{error}</p>}
    <div className="generation-actions">
      {busy && <span role="status"><LoaderCircle size={16} className="working-icon" />Processing source</span>}
      {source && <button type="button" className="secondary" disabled={busy} onClick={() => void perform(async () => onAttach(source))}><Paperclip size={16} />Attach source</button>}
      {table && <button type="submit" className="primary" disabled={busy || !values.length || !table.rows.length}><Check size={16} />Compile data report</button>}
    </div>
  </form>
}