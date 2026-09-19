import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Download, FileOutput, Printer } from 'lucide-react'
import type { DocumentSession, StaticExport, StaticExportFile, StaticExportFormat } from '../../../packages/client/index.mjs'
import { decodeBase64, downloadBytes } from './api'
import './static-export.css'

type PrintPages = { id: string; urls: string[]; indices: number[]; width: number; height: number; revision: number }

export function ExportPanel({ session, pageIndex, onBusy }: { session: DocumentSession; pageIndex: number; onBusy: (busy: boolean) => void }) {
  const [format, setFormat] = useState<StaticExportFormat>('pdf')
  const [pages, setPages] = useState('all')
  const [scale, setScale] = useState(1)
  const [transparent, setTransparent] = useState(false)
  const [quality, setQuality] = useState(90)
  const [matte, setMatte] = useState('#ffffff')
  const [denyWarnings, setDenyWarnings] = useState(false)
  const [output, setOutput] = useState<StaticExport | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [status, setStatus] = useState('')
  const [owner, setOwner] = useState({ session, pageIndex })
  const mounted = useRef(false)
  const pending = useRef(false)
  const lifetime = useRef({ generation: 0 })
  const outputRevision = useRef<number | null>(null)
  const printRoot = useRef<HTMLDivElement | null>(null)
  const printUrls = useRef<string[]>([])
  const [printPages, setPrintPages] = useState<PrintPages | null>(null)
  const [printReady, setPrintReady] = useState(false)
  const printRequested = useRef(false)
  const printObserved = useRef(false)
  const [printing, setPrinting] = useState(false)
  if (owner.session !== session || owner.pageIndex !== pageIndex) {
    setOwner({ session, pageIndex })
    setOutput(null); setPrintPages(null); setPrintReady(false); setPrinting(false); setStatus(''); setError('')
  }
  useLayoutEffect(() => {
    const ownership = lifetime.current
    mounted.current = true
    outputRevision.current = null
    return () => {
      mounted.current = false; ownership.generation++; printRequested.current = false
      printUrls.current.forEach(url => URL.revokeObjectURL(url)); printUrls.current = []; onBusy(false)
    }
  }, [session, pageIndex, onBusy])
  useEffect(() => {
    if (!printPages) return
    let disposed = false
    const images = Array.from(printRoot.current?.querySelectorAll('img') ?? [])
    void Promise.all(images.map(image => image.decode())).then(() => {
      if (disposed || !mounted.current) return
      if (images.length !== printPages.urls.length || images.some(image => image.naturalWidth !== printPages.width || image.naturalHeight !== printPages.height)) throw new Error('Invalid print images')
      setPrintReady(true); setStatus('Print pages ready; confirmation required')
    }).catch(() => {
      if (disposed || !mounted.current) return
      printPages.urls.forEach(url => URL.revokeObjectURL(url)); printUrls.current = []
      setPrintPages(null); setPrintReady(false); setError('Print images could not be loaded. Download the PDF and use your PDF viewer.')
    })
    const before = () => { if (printRequested.current) printObserved.current = true }
    const after = () => {
      if (!printRequested.current) return
      printRequested.current = false
      printPages.urls.forEach(url => URL.revokeObjectURL(url)); printUrls.current = []
      setPrintPages(null); setPrintReady(false); setPrinting(false); onBusy(false)
      setStatus('Print dialog closed. AISlide cannot determine whether you printed or cancelled; prepared pages were removed.')
    }
    window.addEventListener('beforeprint', before)
    window.addEventListener('afterprint', after)
    return () => { disposed = true; window.removeEventListener('beforeprint', before); window.removeEventListener('afterprint', after) }
  }, [printPages, onBusy])
  function clearPrint() {
    lifetime.current.generation++; setPrintPages(null); setPrintReady(false)
    printUrls.current.forEach(url => URL.revokeObjectURL(url)); printUrls.current = []
  }
  function clearOutput() {
    setOutput(null); outputRevision.current = null; setStatus(''); setError('')
    clearPrint()
  }
  async function perform(action: () => Promise<void>) {
    if (pending.current || printRequested.current) return
    pending.current = true; setBusy(true); onBusy(true); setError('')
    try { await action() }
    catch (reason) { if (mounted.current) setError(reason instanceof Error ? reason.message : 'Export failed') }
    finally { pending.current = false; if (mounted.current) { setBusy(false); onBusy(false) } }
  }
  async function prepare() {
    clearOutput()
    const current = lifetime.current.generation
    const revision = session.revision
    const result = await session.exportStatic({ format, page_indices: pages === 'all' ? null : [pageIndex], scale,
      transparent, jpeg_quality: quality, jpeg_matte: [parseInt(matte.slice(1, 3), 16), parseInt(matte.slice(3, 5), 16), parseInt(matte.slice(5, 7), 16)], deny_warnings: denyWarnings })
    if (!mounted.current || current !== lifetime.current.generation || revision !== session.revision) return
    outputRevision.current = revision
    setOutput(result)
    setStatus(`${result.files.length} ${result.files.length === 1 ? 'file' : 'files'} ready`)
  }
  async function download(files: StaticExportFile[]) {
    let completed = 0
    for (const file of files) {
      if (!mounted.current) return
      if (!await downloadBytes(decodeBase64(file.base64), file.filename, file.mime_type)) {
        throw new Error(`Download cancelled. ${completed} completed file requests were retained.`)
      }
      completed++
      if (mounted.current) setStatus(`${completed} file download ${completed === 1 ? 'request' : 'requests'} completed`)
    }
  }
  async function preparePrint() {
    clearPrint()
    setStatus('')
    const current = lifetime.current.generation
    const revision = session.revision
    const selected = output?.files.find(file => file.mime_type === 'application/pdf')?.page_indices
    if (!selected || outputRevision.current !== revision) throw new Error('Prepare a current PDF before printing')
    if (!selected.length || selected.length > 32 || selected.some(index => !Number.isInteger(index) || index < 0)) throw new Error('Print requires 1 to 32 selected slides')
    const result = await session.exportStatic({ format: 'png', page_indices: selected, scale: 1, transparent: false, deny_warnings: denyWarnings })
    if (!mounted.current || current !== lifetime.current.generation || revision !== session.revision) return
    if (result.files.length !== selected.length || result.files.some((file, index) => file.mime_type !== 'image/png' || file.page_indices.length !== 1 || file.page_indices[0] !== selected[index])) throw new Error('Print preparation returned invalid images')
    const { width, height } = result.files[0]
    if (![width, height].every(value => Number.isInteger(value) && value > 0 && value <= 8192) || result.files.some(file => file.width !== width || file.height !== height)) throw new Error('Invalid print page size')
    if (result.files.reduce((size, file) => size + file.base64.length, 0) > 3 * 1024 * 1024 - 1024) throw new Error('Print images exceed the static bundle byte budget')
    const images = result.files.map(file => {
      const bytes = decodeBase64(file.base64)
      if (bytes.byteLength !== file.byte_length || ![137, 80, 78, 71, 13, 10, 26, 10].every((byte, index) => bytes[index] === byte)) throw new Error('Invalid print PNG bytes')
      return bytes
    })
    try { for (const bytes of images) printUrls.current.push(URL.createObjectURL(new Blob([bytes], { type: 'image/png' }))) }
    catch (reason) { clearPrint(); throw reason }
    setPrintPages({ id: `aislide-print-${crypto.randomUUID()}`, urls: [...printUrls.current], indices: [...selected], width, height, revision })
    setStatus('Loading print pages')
  }
  function requestPrint() {
    if (!printReady || !printPages || pending.current || printRequested.current) return
    if (printPages.revision !== session.revision) { clearOutput(); setError('The document changed. Prepare a current PDF before printing.'); return }
    const unsupported = () => {
      printRequested.current = false; setPrinting(false); onBusy(false); clearPrint(); setStatus('')
      setError('This WebView did not confirm a print dialog. Download the PDF and use your PDF viewer.')
    }
    if (typeof window.print !== 'function') { unsupported(); return }
    try {
      printRequested.current = true; printObserved.current = false; setPrinting(true); onBusy(true)
      setStatus('Print dialog requested. Confirm or cancel in the system dialog.')
      window.print()
      if (!printObserved.current && printRequested.current) unsupported()
    } catch { unsupported() }
  }
  return <section className="static-export" aria-label="Static export options">
    <form onSubmit={event => { event.preventDefault(); void perform(prepare) }}>
      <fieldset disabled={busy || printing} className="static-export-options" onChange={clearOutput}>
        <label className="field">Format<select aria-label="Export format" value={format} onChange={event => setFormat(event.target.value as StaticExportFormat)}><option value="pdf">PDF</option><option value="png">PNG</option><option value="jpeg">JPEG</option></select></label>
        <label className="field">Pages<select aria-label="Export pages" value={pages} onChange={event => setPages(event.target.value)}><option value="all">All slides</option><option value="selected">Current slide ({pageIndex + 1})</option></select></label>
        <label className="field">Scale<input aria-label="Export scale" type="number" min={0.01} max={16} step={0.01} required value={scale} onChange={event => setScale(event.currentTarget.valueAsNumber)} /></label>
        {format !== 'pdf' && <label className="checkbox"><input type="checkbox" checked={transparent} onChange={event => setTransparent(event.target.checked)} />Omit slide background</label>}
        {format === 'jpeg' && <><label className="field">JPEG quality<input aria-label="JPEG quality" type="number" min={1} max={100} required value={quality} onChange={event => setQuality(event.currentTarget.valueAsNumber)} /></label><label className="field">JPEG matte<input aria-label="JPEG matte" type="color" value={matte} onChange={event => setMatte(event.target.value)} /></label></>}
        <label className="checkbox"><input type="checkbox" checked={denyWarnings} onChange={event => setDenyWarnings(event.target.checked)} />Reject render warnings</label>
      </fieldset>
      <p className="warning">Office visual parity is unverified. Limits are 8192 px per edge, 32 MiB core output and a separate static bundle below 3 MiB. Static output limits are independent of the document capacity profile; fewer pages or a lower scale may be required.</p>
      {format === 'pdf' && <p className="warning">PDF appearance uses outlined text with a positioned searchable/selectable Unicode layer and basic structure tags. Visible text is not editable. Not PDF/A, PDF/UA or WCAG certified. Reading order and alternative text need review.</p>}
      <button className="primary" disabled={busy || printing} type="submit"><FileOutput size={18} aria-hidden="true" />Prepare export</button>
    </form>
    {error && <p role="alert" className="error">{error}</p>}
    <p role="status">{busy ? 'Export operation pending' : status}</p>
    {output && <>
      <ul className="static-export-files">{output.files.map(file => <li key={file.filename}><span><strong>{file.filename}</strong><small>Pages {file.page_indices.map(index => index + 1).join(', ')} / {file.width} x {file.height} / {Math.ceil(file.byte_length / 1024)} KiB</small></span><button className="secondary" disabled={busy || printing} aria-label={`Download ${file.filename}`} onClick={() => void perform(() => download([file]))}><Download size={18} aria-hidden="true" />Download</button></li>)}</ul>
      <div className="static-export-actions">
        {output.files.length > 1 && <button className="secondary" disabled={busy || printing} onClick={() => void perform(() => download(output.files))}><Download size={18} aria-hidden="true" />Download all images</button>}
        {output.files.some(file => file.mime_type === 'application/pdf') && <button className="secondary" disabled={busy || printing} title="Prepare slide images for the system print dialog" aria-label="Print PDF" onClick={() => void perform(preparePrint)}><Printer size={18} aria-hidden="true" />Prepare print</button>}
      </div>
      {printPages && <>
        <p className="warning">Direct print uses 96 dpi slide images, not the searchable PDF. Requested paper size: {printPages.width / 96} x {printPages.height / 96} in. Paper sizing and output depend on the system dialog.</p>
        {createPortal(<>
          <style data-aislide-print-style={printPages.id} media="print">{`
            @page { size: ${printPages.width / 96}in ${printPages.height / 96}in; margin: 0; }
            html:has(body > .${printPages.id}), body:has(> .${printPages.id}) { margin: 0 !important; padding: 0 !important; height: auto !important; min-height: 0 !important; overflow: visible !important; background: white !important; }
            body:has(> .${printPages.id}) > :not(.${printPages.id}) { display: none !important; }
            body:has(> .${printPages.id}) dialog::backdrop { display: none !important; }
            body > .${printPages.id} { display: block !important; margin: 0 !important; padding: 0 !important; }
            body > .${printPages.id} > img { display: block; width: ${printPages.width / 96}in; height: ${printPages.height / 96}in; margin: 0; break-inside: avoid; break-after: page; print-color-adjust: exact; }
            body > .${printPages.id} > img:last-child { break-after: auto; }
          `}</style>
          <div ref={printRoot} className={printPages.id} data-aislide-print={printPages.id} style={{ display: 'none' }}>
            {printPages.urls.map((url, index) => <img key={url} src={url} alt={`Slide ${printPages.indices[index] + 1}`} />)}
          </div>
        </>, document.body)}
        <button className="secondary" disabled={busy || printing || !printReady} onClick={requestPrint}><Printer size={18} aria-hidden="true" />Open print dialog</button>
      </>}
      {output.warnings.length > 0 && <ul className="static-export-warnings">{output.warnings.map((warning, index) => <li key={index}>Page {warning.page_index + 1}: {warning.code} / {warning.message}</li>)}</ul>}
    </>}
  </section>
}