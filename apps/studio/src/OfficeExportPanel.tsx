import { useRef, useState } from 'react'
import type { DocumentSession } from '../../../packages/client/index.mjs'
import type { CopilotInteractiveExport, PowerpointInteractiveExport } from './types'
import { decodeBase64, downloadBytes } from './api'

export function OfficeExportPanel({ session, onBusy }: { session: DocumentSession; onBusy: (busy: boolean) => void }) {
  const [runtime, setRuntime] = useState('https://localhost:4198/office.html')
  const [catalog, setCatalog] = useState('')
  const [target, setTarget] = useState('aislide')
  const [acknowledged, setAcknowledged] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [result, setResult] = useState<PowerpointInteractiveExport | CopilotInteractiveExport | null>(null)
  const pending = useRef(false)
  const update = () => { setResult(null); setError('') }
  async function prepare() {
    if (pending.current) return
    pending.current = true; setBusy(true); onBusy(true); setError(''); setResult(null)
    try {
      if (target === 'copilot') {
        if (!acknowledged) throw new Error('Acknowledge the experimental Copilot catalog contract before exporting.')
        setResult(await session.exportCopilotInteractive({ acknowledged_experimental: true }))
      } else {
        setResult(await session.exportPowerpointInteractive({ runtime_url: runtime, catalog: { kind: 'file_system', location: catalog } }))
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    } finally { pending.current = false; setBusy(false); onBusy(false) }
  }
  return <section aria-label="PowerPoint content add-in export">
    <label className="field">Playback target<select aria-label="PowerPoint playback target" value={target} disabled={busy} onChange={event => { setTarget(event.target.value); setAcknowledged(false); update() }}><option value="aislide">AISlide own content add-in</option><option value="copilot">Copilot catalog (experimental)</option></select></label>
    <p>Nothing is installed or trusted by this dialog. Keep the normal editable PPTX as your source; this playback export is read-only when reopened in AISlide.</p>
    {target === 'copilot' ? <div>
      <p>This experiment references Microsoft's observed catalog ID fa000000210 (sdxcatalog, en-US) and embeds AISlide-generated HTML in contentBlockHtml. This is not a supported public Copilot authoring API or Microsoft-generated content. The Microsoft host may reject it or change; licensing, rollout and organization policy still apply. No bypass is provided.</p>
      <p>No AISlide runtime server or own manifest is used for this target. PowerPoint still needs its Microsoft interactive-slide runtime. Static snapshots do not prove HTML playback.</p>
      <label className="field"><span><input aria-label="Acknowledge experimental Copilot catalog" type="checkbox" checked={acknowledged} disabled={busy} onChange={event => setAcknowledged(event.target.checked)} /> I acknowledge the unpublished contract and unverified Copilot playback.</span></label>
    </div> : <>
    <p>This development export uses an AISlide-owned Office content add-in. It can run generated HTML controls inside a supported PowerPoint host only after the recipient installs the manifest and trusts/reaches its HTTPS runtime. Network-share catalogs are Windows testing only, not production deployment.</p>
    <label className="field">Trusted HTTPS runtime URL<input aria-label="PowerPoint runtime URL" value={runtime} maxLength={2048} disabled={busy} onChange={event => { setRuntime(event.target.value); update() }} /></label>
    <label className="field">Trusted catalog UNC path<input aria-label="PowerPoint catalog UNC path" placeholder="\\server\AISlideAddins" value={catalog} maxLength={2048} disabled={busy} onChange={event => { setCatalog(event.target.value); update() }} /></label>
    </>}
    {error && <p className="error" role="alert">{error}</p>}
    <button className="primary" disabled={busy || (target === 'copilot' ? !acknowledged : !catalog.trim())} onClick={() => void prepare()}>{busy ? 'Preparing playback export' : 'Prepare PowerPoint interactive export'}</button>
    {result && <div>
      <p role="status">Prepared {result.components} content add-ins. PowerPoint playback has not been verified by this export.</p>
      {result.warnings.length > 0 && <div role="alert"><p>Static snapshot warnings: {result.warnings.length}. Review the fallback before distribution.</p><ul>{[...new Set(result.warnings.map(item => item.warning.message))].map(message => <li key={message}>{message}</li>)}</ul></div>}
      <div className="form-actions">
        <button className="primary" onClick={() => downloadBytes(decodeBase64(result.base64), result.filename, 'application/vnd.openxmlformats-officedocument.presentationml.presentation')}>Download interactive PPTX</button>
        {'manifest' in result && <button className="secondary" onClick={() => downloadBytes(new TextEncoder().encode(result.manifest), 'aislide.addin.xml', 'application/xml')}>Download add-in manifest</button>}
      </div>
      {'manifest' in result ? <ol><li>Start the AISlide content runtime using a certificate trusted by the target machine.</li><li>Place this manifest in the indicated network catalog and explicitly trust/install it in PowerPoint.</li><li>Open the downloaded playback PPTX. If the add-in is inactive, PowerPoint may display its saved static snapshot.</li></ol> : <p>Open this experimental file only in an approved PowerPoint environment and verify actual selections/filters in slide show. If its Microsoft host does not activate, keep the static source; this export does not silently change runtime targets.</p>}
      <p>Generated HTML and its underlying data are stored in the PPTX. Do not treat snapshots, compiled states or hidden views as confidentially removed content.</p>
    </div>}
  </section>
}
