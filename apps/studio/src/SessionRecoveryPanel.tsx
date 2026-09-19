import { useEffect, useRef, useState } from 'react'
import { History, RefreshCw, RotateCcw, Trash2 } from 'lucide-react'
import type { SessionRecovery } from '../../../packages/client/index.mjs'
import type { AislideDocument } from './types'
import { core } from './api'
import { RecoveryPanel } from './RecoveryPanel'
import { sessionRecoveryStore, type SessionRecoveryEntry } from './recovery-v2'

export function SessionRecoveryPanel({ enabled, onEnabledChange, onRestore, onLegacyRestore, onBusy }: {
  enabled: boolean
  onEnabledChange: (enabled: boolean) => Promise<void>
  onRestore: (envelope: SessionRecovery, filename: string) => void | Promise<void>
  onLegacyRestore: (document: AislideDocument, filename: string) => void | Promise<void>
  onBusy: (busy: boolean) => void
}) {
  const [entries, setEntries] = useState<SessionRecoveryEntry[]>([])
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(true)
  const [legacy, setLegacy] = useState(false)
  const [clear, setClear] = useState(false)
  const mounted = useRef(false)
  const active = useRef(false)
  useEffect(() => {
    let disposed = false
    mounted.current = true
    onBusy(true)
    void sessionRecoveryStore.list().then(value => { if (!disposed) setEntries(value) }, reason => { if (!disposed) setError(String(reason)) })
      .finally(() => { if (!disposed) { setBusy(false); onBusy(false) } })
    return () => { disposed = true; mounted.current = false; onBusy(false) }
  }, [onBusy])
  async function run(action: () => Promise<void>) {
    if (active.current) return
    active.current = true; setBusy(true); onBusy(true); setError('')
    try { await action() } catch (reason) { if (mounted.current) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { active.current = false; if (mounted.current) { setBusy(false); onBusy(false) } }
  }
  async function refresh() { const value = await sessionRecoveryStore.list(); if (mounted.current) setEntries(value) }
  return <section className="object-tools-panel" aria-label="Session recovery" aria-busy={busy}>
    <h2>Local recovery v2</h2>
    <label className="object-tools-check"><input type="checkbox" checked={enabled} disabled={busy} onChange={event => {
      const next = event.target.checked
      void run(async () => { await onEnabledChange(next); await refresh() })
    }} />Store recovery copies including original presentation bytes, sensitive sources and Undo/Redo history</label>
    <p>Copies are plaintext, not encrypted by AISlide. Desktop copies use this user's application data folder; web copies use this browser profile. Device backup and access policies still apply.</p>
    <p>Up to 5 entries, 48 MiB each and 100 MiB total. Oldest v2 copies may be removed to stay within these limits. Undo and Redo each retain up to 30 receipts and 4 MiB. Larger edits create a visible history boundary.</p>
    <p>V2 copies expire 7 days after the last successful checkpoint, only when the enabled store is opened. No expiry runs while the app is closed. Turning recovery off retains copies. Recovery never overwrites the original PPTX.</p>
    <div className="object-tools-grid">
      <button type="button" className="secondary" disabled={busy} onClick={() => void run(refresh)}><RefreshCw size={18} aria-hidden="true" />Refresh recovery</button>
      <button type="button" className="secondary" disabled={busy} onClick={() => setClear(true)}><Trash2 size={18} aria-hidden="true" />Delete v2 recovery copies</button>
      <button type="button" className="secondary" disabled={busy} onClick={() => setLegacy(value => !value)}><History size={18} aria-hidden="true" />Legacy v1 copies</button>
    </div>
    {clear && <div role="group" aria-label="Confirm v2 deletion"><p>Delete all v2 work copies? Original PPTX files and legacy v1 copies remain unchanged.</p>
      <button type="button" disabled={busy} onClick={() => setClear(false)}>Cancel</button>
      <button type="button" disabled={busy} onClick={() => void run(async () => { await sessionRecoveryStore.clear(); if (mounted.current) setClear(false); await refresh() })}>Confirm delete v2 copies</button>
    </div>}
    {error && <p role="alert">{error}</p>}
    <p role="status">{busy ? 'Recovery operation pending' : `${entries.length} recovery copies`}</p>
    <ul className="object-tools-list">{entries.map(entry => <li key={entry.id}>
      <div style={{ minWidth: 0, overflowWrap: 'anywhere' }}><strong>{entry.filename}</strong><div>Revision {entry.revision} / {new Date(entry.saved_at).toLocaleString()} / {Math.ceil(entry.byte_length / 1024)} KiB</div><div>Snapshot hash: <code>{entry.hash}</code></div></div>
      <button type="button" className="secondary" disabled={busy} aria-label={`Restore ${entry.filename}`} onClick={() => void run(async () => {
        const restored = await sessionRecoveryStore.load(entry.id, entry)
        if (mounted.current) await onRestore(restored.envelope, restored.filename)
      })}><RotateCcw size={18} aria-hidden="true" />Restore</button>
      <button type="button" className="secondary" disabled={busy} aria-label={`Discard ${entry.filename}`} onClick={() => void run(async () => { await sessionRecoveryStore.remove(entry.id, entry); await refresh() })}><Trash2 size={18} aria-hidden="true" />Discard</button>
    </li>)}</ul>
    {legacy && <><p>Legacy v1 copies are retained without automatic migration or expiry. Restoring a legacy copy starts empty history; v2 persistence requires the new consent above.</p>
      <RecoveryPanel enabled={false} readOnlyPolicy onEnabledChange={() => {}} validate={document => core<AislideDocument>({ op: 'verify_recovery', document })} onRestore={onLegacyRestore} />
    </>}
  </section>
}