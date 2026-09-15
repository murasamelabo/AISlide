import { useState } from 'react'
import { ArrowRight, ChartNoAxesCombined, Minus, Table2, Type } from 'lucide-react'
import { core } from './api'
import { chartNames } from './design'
import { ShapeSurface } from './ShapeSurface'
import type { Element, ObjectCatalog, ObjectKind } from './types'

export function InsertPanel({ catalog, onInsert, onBusy }: { catalog: ObjectCatalog; onInsert: (element: Element) => Promise<void>; onBusy: (busy: boolean) => void }) {
  const [rows, setRows] = useState(4)
  const [columns, setColumns] = useState(3)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  async function insert(kind: ObjectKind, preset?: string) {
    if (busy) return
    setBusy(true); onBusy(true); setError('')
    try { await onInsert(await core<Element>({ op: 'create_object', id: `${kind}-${crypto.randomUUID().slice(0, 8)}`, kind, preset, ...(kind === 'table' ? { rows, columns } : {}) })) }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { setBusy(false); onBusy(false) }
  }
  return <div className="insert-panel">
    <fieldset disabled={busy} className="insert-controls">
      <div className="insert-quick"><button className="secondary" onClick={() => void insert('text')}><Type size={17} />Text</button><button className="tool" title="Insert line" aria-label="Insert line" onClick={() => void insert('line')}><Minus size={18} /></button><button className="tool" title="Insert arrow" aria-label="Insert arrow" onClick={() => void insert('arrow')}><ArrowRight size={18} /></button></div>
      <h3>Shapes</h3>
      <div className="shape-catalog">{catalog.shapes.map((shape) => <button key={shape.id} aria-label={`Insert ${shape.name}`} title={shape.name} onClick={() => void insert('shape', shape.id)}><ShapeSurface preset={shape.id} fill="#edf3f0" stroke="#087f73" /><span>{shape.name}</span></button>)}</div>
      <h3>Table</h3><div className="insert-table-controls"><label className="field">Rows<input type="number" min={1} max={catalog.table.max_rows} value={rows} onChange={(event) => setRows(event.currentTarget.valueAsNumber)} /></label><label className="field">Columns<input type="number" min={1} max={catalog.table.max_columns} value={columns} onChange={(event) => setColumns(event.currentTarget.valueAsNumber)} /></label><button className="secondary" aria-label="Insert table" onClick={() => void insert('table')}><Table2 size={17} />Insert table</button></div>
      <h3>Charts</h3><div className="chart-catalog">{catalog.charts.map((kind) => <button key={kind} className="secondary" aria-label={`Insert ${chartNames[kind]} chart`} onClick={() => void insert('chart', kind)}><ChartNoAxesCombined size={18} />{chartNames[kind]}</button>)}</div>
    </fieldset>
    {error && <p className="error" role="alert">{error}</p>}
  </div>
}