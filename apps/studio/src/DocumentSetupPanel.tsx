import { useEffect, useRef, useState } from 'react'
import { Check, Download, FolderOpen, RefreshCw, Trash2 } from 'lucide-react'
import { AislideClient, CAPACITY_PROFILES, type CapacityProfile, type DocumentSession } from '../../../packages/client/index.mjs'
import type { AislideDocument, CanvasResizeInput, TemplateKind } from './types'
import { core, decodeBase64, fileBase64 } from './api'
import { Tool } from './Tool'
import './object-tools.css'

export type PanelExport = (bytes: Uint8Array, filename: string, mime: string) => Promise<boolean | void>
export type DocumentSetupPanelProps = {
  session: DocumentSession
  onDocument: (document: AislideDocument) => void
  onBusy: (busy: boolean) => void
  onReplaceSession: (session: DocumentSession) => void
  onExport: PanelExport
  onBeforeOpen?: () => Promise<boolean>
}

export function usePanelTask(session: DocumentSession, onBusy: (busy: boolean) => void) {
  const [working, setWorking] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const pending = useRef(false)
  const alive = useRef(true)
  const currentSession = useRef(session)
  useEffect(() => { currentSession.current = session }, [session])
  useEffect(() => { alive.current = true; return () => { alive.current = false } }, [])
  async function run(action: (active: () => boolean) => Promise<void>, expectedRevision = session.revision) {
    if (pending.current || session.busy) return
    pending.current = true
    setWorking(true); setError(''); setMessage(''); onBusy(true)
    const active = () => alive.current && currentSession.current === session
    try {
      if (session.revision !== expectedRevision) throw new Error('Document changed. Reload this panel before applying the draft.')
      await action(active)
    } catch (reason) {
      if (active()) setError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      pending.current = false
      if (alive.current) setWorking(false)
      onBusy(false)
    }
  }
  return { run, busy: working || session.busy, error, message, setMessage }
}

type Favorite = { id: string; name: string; kind: TemplateKind; base64: string }
async function favorites<T>(action: (store: IDBObjectStore) => IDBRequest<T>, write = false): Promise<T> {
  const database = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('aislide-user-templates', 1)
    request.onupgradeneeded = () => request.result.createObjectStore('templates', { keyPath: 'id' })
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
    request.onblocked = () => reject(new Error('Local template storage is blocked.'))
  })
  try {
    return await new Promise<T>((resolve, reject) => {
      const transaction = database.transaction('templates', write ? 'readwrite' : 'readonly')
      const request = action(transaction.objectStore('templates'))
      transaction.oncomplete = () => resolve(request.result)
      transaction.onabort = () => reject(transaction.error ?? new Error('Local template storage failed.'))
      transaction.onerror = () => reject(transaction.error)
    })
  } finally { database.close() }
}

export function DocumentSetupPanel(props: DocumentSetupPanelProps) {
  const [reload, setReload] = useState(0)
  const [identity, setIdentity] = useState(props.session)
  if (identity !== props.session) { setIdentity(props.session); setReload((value) => value + 1) }
  const [template, setTemplate] = useState<{ session: DocumentSession; name: string } | null>(null)
  return <DocumentSetupForm key={`${props.session.document.id}:${reload}`} {...props} templateName={template?.session === props.session ? template.name : ''} onImported={(session, name) => { setTemplate({ session, name }); props.onReplaceSession(session) }} onReload={() => setReload((value) => value + 1)} />
}

function DocumentSetupForm({ session, onDocument, onBusy, onExport, onBeforeOpen, onReload, templateName, onImported }: DocumentSetupPanelProps & { onReload: () => void; templateName: string; onImported: (session: DocumentSession, name: string) => void }) {
  const [revision, setRevision] = useState(session.revision)
  const [size, setSize] = useState({ width: session.document.deck.width, height: session.document.deck.height })
  const [mode, setMode] = useState<CanvasResizeInput['mode']>('scale')
  const [preset, setPreset] = useState('custom')
  const [kind, setKind] = useState<TemplateKind>('potx')
  const [replaceConfirmed, setReplaceConfirmed] = useState(false)
  const [remember, setRemember] = useState(false)
  const [saved, setSaved] = useState<Favorite[]>([])
  const [profile, setProfile] = useState(session.capacityProfile)
  const task = usePanelTask(session, onBusy)
  const client = new AislideClient(core, { capacityProfile: session.capacityProfile })
  const stale = revision !== session.revision
  async function open(template: Favorite, active: () => boolean) {
    if (!replaceConfirmed) throw new Error('Confirm replacement first.')
    if (onBeforeOpen && !await onBeforeOpen()) return
    if (!active() || session.revision !== revision) throw new Error('Document changed before template import.')
    const replacement = await client.importTemplate(`template-${crypto.randomUUID()}`, { kind: template.kind, base64: template.base64 })
    if (!active() || session.revision !== revision) throw new Error('Document changed during template import. The active session was retained.')
    if (remember) {
      await favorites((store) => store.put(template), true)
      if (!active() || session.revision !== revision) throw new Error('Document changed. The active session was retained.')
    }
    onImported(replacement, template.name)
  }
  return <section className="object-tools-panel" aria-label="Document setup" aria-busy={task.busy} onKeyDown={(event) => event.stopPropagation()}>
    <div className="object-tools-heading"><h2>Document setup</h2><Tool label="Reload document setup" disabled={task.busy} onClick={onReload}><RefreshCw /></Tool></div>
    {stale && <p role="alert">Document changed. Reload before applying.</p>}
    <fieldset disabled={task.busy || stale}><legend>Capacity</legend>
      <label>Capacity profile<select value={profile} onChange={event => setProfile(event.target.value as CapacityProfile)}>
        {(['legacy', 'standard', 'large'] as const).map(name => <option key={name} value={name}>{name} / {CAPACITY_PROFILES[name].slides} slides / {CAPACITY_PROFILES[name].document_bytes / 1048576} MiB document</option>)}
      </select></label>
      <p>Active: {session.capacityProfile}. Document bytes include the immutable original and embedded fonts. Archive limit: {CAPACITY_PROFILES[profile].archive_bytes / 1048576} MiB. Lowering validates the full document and retained history; it never truncates content.</p>
      <button type="button" disabled={profile === session.capacityProfile} onClick={() => void task.run(async active => {
        const document = await session.setCapacityProfile(profile)
        if (active()) { onDocument(document); task.setMessage('Capacity profile applied.') }
      }, revision)}><Check aria-hidden="true" />Apply capacity profile</button>
    </fieldset>
    <form onSubmit={(event) => { event.preventDefault(); void task.run(async (active) => {
      const document = await session.resizeCanvas({ ...size, mode }, { expectedRevision: revision })
      if (active()) { setRevision(document.revision); onDocument(document) }
    }, revision) }}><fieldset disabled={task.busy || stale}><legend>Page size</legend>
      <label>Preset<select value={preset} onChange={(event) => {
        const value = event.target.value; setPreset(value)
        if (value !== 'custom') setSize(value === '16:9' ? { width: 1280, height: 720 } : value === '4:3' ? { width: 960, height: 720 } : { width: 720, height: 1280 })
      }}><option value="16:9">16:9</option><option value="4:3">4:3</option><option value="portrait">Portrait</option><option value="custom">Custom</option></select></label>
      <div className="object-tools-grid">{(['width', 'height'] as const).map((axis) => <label key={axis}>{axis === 'width' ? 'Page width' : 'Page height'}<input type="number" required min={320} max={4096} value={size[axis]} onChange={(event) => { setPreset('custom'); setSize({ ...size, [axis]: Number(event.target.value) }) }} /></label>)}</div>
      <label>Content<select value={mode} onChange={(event) => setMode(event.target.value as typeof mode)}><option value="scale">Scale</option><option value="keep">Keep size</option></select></label>
      <button type="submit"><Check aria-hidden="true" />Apply page size</button>
    </fieldset></form>
    <fieldset disabled={task.busy || stale}><legend>Templates</legend>
      <label className="object-tools-check"><input type="checkbox" checked={replaceConfirmed} onChange={(event) => setReplaceConfirmed(event.target.checked)} />Replace active document</label>
      <label className="object-tools-check"><input type="checkbox" checked={remember} onChange={(event) => setRemember(event.target.checked)} />Remember imported template on this device</label>
      <label>Open POTX or THMX<input type="file" accept=".potx,.thmx" disabled={!replaceConfirmed} onChange={(event) => {
        const file = event.target.files?.[0]; event.target.value = ''
        if (!file) return
        void task.run(async (active) => {
          const extension = file.name.split('.').at(-1)?.toLowerCase()
          if (extension !== 'potx' && extension !== 'thmx') throw new Error('Choose a POTX or THMX file.')
          await open({ id: crypto.randomUUID(), name: file.name, kind: extension, base64: await fileBase64(file, session.capacityProfile) }, active)
        }, revision)
      }} /></label>
      {templateName && <output aria-label="Imported template">{templateName}</output>}
      <label>Export format<select value={kind} onChange={(event) => setKind(event.target.value as TemplateKind)}><option value="potx">POTX</option><option value="thmx">THMX</option></select></label>
      <Tool label="Download template" disabled={task.busy || stale} onClick={() => void task.run(async (active) => {
        const result = await session.exportTemplate(kind)
        if (!active() || session.revision !== revision) throw new Error('Document changed during export.')
        const saved = await onExport(decodeBase64(result.base64), result.filename, kind === 'potx' ? 'application/vnd.openxmlformats-officedocument.presentationml.template' : 'application/vnd.openxmlformats-officedocument.theme')
        if (active()) task.setMessage(saved === false ? 'Download cancelled.' : 'Template exported.')
      }, revision)}><Download /></Tool>
      <Tool label="Load local template favorites" disabled={task.busy} onClick={() => void task.run(async (active) => { const items = await favorites<Favorite[]>((store) => store.getAll()); if (active()) setSaved(items) })}><FolderOpen /></Tool>
      <ul className="object-tools-list">{saved.map((template) => <li key={template.id}><span>{template.name} ({template.kind.toUpperCase()})</span><Tool label={`Open ${template.name}`} disabled={task.busy || !replaceConfirmed} onClick={() => void task.run((active) => open(template, active), revision)}><FolderOpen /></Tool><Tool label={`Forget ${template.name}`} disabled={task.busy} onClick={() => void task.run(async (active) => { await favorites((store) => store.delete(template.id), true); if (active()) setSaved((items) => items.filter((item) => item.id !== template.id)) })}><Trash2 /></Tool></li>)}</ul>
    </fieldset>
    <p role="status">{task.message}</p>{task.error && <p role="alert">{task.error}</p>}
  </section>
}