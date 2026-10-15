import { useEffect, useId, useRef, useState } from 'react'
import { RefreshCw, RotateCcw, Trash2 } from 'lucide-react'
import { recoveryError, recoveryStore } from './recovery'
import type { RecoveryEntry, RecoveryStore, RecoveryValidator } from './recovery'
import type { AislideDocument } from './types'

export type RecoveryPanelProps = {
  enabled: boolean
  readOnlyPolicy?: boolean
  onEnabledChange: (enabled: boolean) => void | Promise<void>
  onRestore: (document: AislideDocument, filename: string) => void | Promise<void>
  validate: RecoveryValidator
  onError?: (error: Error) => void
  store?: RecoveryStore
}

export function RecoveryPanel({ enabled, readOnlyPolicy = false, onEnabledChange, onRestore, validate, onError, store = recoveryStore }: RecoveryPanelProps) {
  const consentId = useId()
  const descriptionId = useId()
  const [entries, setEntries] = useState<readonly RecoveryEntry[]>([])
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [ready, setReady] = useState(false)
  const [clearRequested, setClearRequested] = useState(false)
  const operation = useRef(false)
  const mounted = useRef(false)
  const errorHandler = useRef(onError)
  useEffect(() => { errorHandler.current = onError }, [onError])

  useEffect(() => {
    let active = true
    mounted.current = true
    void store.list().then(result => {
      if (active) { setEntries(result); setReady(true) }
    }, reason => {
      if (active) {
        const failure = recoveryError(reason)
        setError(failure.message); setReady(true); errorHandler.current?.(failure)
      }
    })
    return () => { active = false; mounted.current = false }
  }, [store])

  async function perform(action: () => Promise<void>) {
    if (operation.current) return
    operation.current = true; setBusy(true); setError('')
    try { await action() }
    catch (reason) {
      if (mounted.current) {
        const failure = recoveryError(reason)
        setError(failure.message); errorHandler.current?.(failure)
      }
    } finally {
      operation.current = false
      if (mounted.current) setBusy(false)
    }
  }
  async function refresh() {
    const result = await store.list()
    if (mounted.current) setEntries(result)
  }
  async function restore(entry: RecoveryEntry) {
    const recovered = await store.load(entry.id, validate, entry)
    if (mounted.current) await onRestore(structuredClone(recovered.document) as AislideDocument, recovered.filename)
  }

  return <section aria-label="Local work recovery" style={{ minWidth: 0, maxWidth: '100%', overflowWrap: 'anywhere', padding: 16 }}>
    <h2 style={{ fontSize: 20, marginTop: 0 }}>Local work recovery</h2>
    <label htmlFor={consentId} style={{ display: 'flex', gap: 12, alignItems: 'flex-start' }}>
      <input id={consentId} type="checkbox" checked={enabled} disabled={readOnlyPolicy || busy || !ready} aria-describedby={descriptionId}
        onChange={event => { const next = event.target.checked; void perform(async () => { await store.config(next); if (mounted.current) await onEnabledChange(next) }) }} />
      Store recovery copies on this browser profile, including sensitive source content and original presentation bytes.
    </label>
    <p id={descriptionId}>Recovery copies are not encrypted by AISlide. Anyone with access to this browser or WebView2 profile may read them. Profile backup or sync depends on your device configuration.</p>
    <p>At most five documents, 2 MiB per snapshot and 10 MiB total. Only completed snapshot writes survive a crash; browser eviction can remove copies. Turning recovery off retains existing copies until you discard them.</p>
    <p>Restore starts fresh Undo/Redo history. Current unsaved changes must be resolved before replacement.</p>
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
      <button type="button" className="secondary" disabled={busy || !ready} onClick={() => void perform(refresh)}><RefreshCw size={18} aria-hidden="true" />Refresh recovery</button>
      <button type="button" className="secondary" disabled={busy || !ready} onClick={() => setClearRequested(true)}><Trash2 size={18} aria-hidden="true" />Delete all recovery copies</button>
    </div>
    {clearRequested && <div role="group" aria-label="Confirm deletion" style={{ marginBlock: 16 }}>
      <p>Delete all AISlide recovery copies in this profile? This does not delete source files or change the recovery setting.</p>
      <button type="button" className="secondary" disabled={busy} onClick={() => setClearRequested(false)}>Cancel</button>
      <button type="button" className="secondary" disabled={busy} onClick={() => void perform(async () => { await store.clear(); setClearRequested(false); await refresh() })}><Trash2 size={18} aria-hidden="true" />Confirm delete all</button>
    </div>}
    {error && <p className="error" role="alert">{error}</p>}
    <p role="status">{busy ? 'Recovery operation pending' : !ready ? 'Loading recovery copies' : `${entries.length} recovery ${entries.length === 1 ? 'copy' : 'copies'}`}</p>
    <ul style={{ padding: 0, listStyle: 'none' }}>{entries.map(entry => <li key={entry.id} style={{ borderTop: '1px solid #b8b8b8', paddingBlock: 16 }}>
      <strong>{entry.filename}</strong>
      <div>Revision {entry.revision} / <time dateTime={new Date(entry.savedAt).toISOString()}>{new Date(entry.savedAt).toLocaleString()}</time> / {Math.ceil(entry.byteLength / 1024)} KiB</div>
      <div>Snapshot hash: <code>{entry.hash}</code></div>
      {entry.originalHash && <div>Original hash: <code>{entry.originalHash}</code></div>}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 8 }}>
        <button type="button" className="secondary" aria-label={`Restore ${entry.filename}`} disabled={busy} onClick={() => void perform(() => restore(entry))}><RotateCcw size={18} aria-hidden="true" />Restore</button>
        <button type="button" className="secondary" aria-label={`Discard ${entry.filename}`} disabled={busy} onClick={() => void perform(async () => { await store.remove(entry.id, entry); await refresh() })}><Trash2 size={18} aria-hidden="true" />Discard</button>
      </div>
    </li>)}</ul>
  </section>
}