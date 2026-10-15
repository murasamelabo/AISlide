import { useEffect, useRef, useState } from 'react'
import { Check, FilePlus2, Type } from 'lucide-react'
import { AislideClient, CAPACITY_PROFILES, FONT_LIMITS } from '../../../packages/client/index.mjs'
import type { AislideDocument, DocumentSession, EmbeddedFont, Element, FontInfo } from '../../../packages/client/index.mjs'
import { core, decodeBase64, fileBase64 } from './api'

const client = new AislideClient(core)

export function DocumentFonts({ fonts }: { fonts?: EmbeddedFont[] }) {
  const [failure, setFailure] = useState<{ source: EmbeddedFont[] | undefined; message: string } | null>(null)
  useEffect(() => {
    let cancelled = false
    const owned: FontFace[] = []
    for (const font of fonts ?? []) {
      if (!font.license_acknowledged) continue
      void Promise.resolve().then(async () => {
        if (cancelled) return
        const face = new FontFace(JSON.stringify(font.family), decodeBase64(font.base64), {
          weight: font.style === 'bold' || font.style === 'bold_italic' ? '700' : '400',
          style: font.style === 'italic' || font.style === 'bold_italic' ? 'italic' : 'normal',
        })
        owned.push(face)
        await face.load()
        if (!cancelled) document.fonts.add(face)
      }).catch(() => {
        if (!cancelled) setFailure({ source: fonts, message: `Font could not be loaded: ${font.family}. Fallback remains active.` })
      })
    }
    return () => { cancelled = true; for (const face of owned) document.fonts.delete(face) }
  }, [fonts])
  return failure && failure.source === fonts ? <div className="error-strip" role="alert">{failure.message}</div> : null
}

function FontDetails({ info }: { info: FontInfo }) {
  return <section style={{ overflowWrap: 'anywhere' }}>
    <h3>{info.family}</h3>
    <dl><dt>Style</dt><dd>{info.style.replaceAll('_', ' ')}</dd><dt>Embedding bits</dt><dd>{info.fs_type === null ? 'Unknown' : `0x${info.fs_type.toString(16).padStart(4, '0')} / ${info.permission.replaceAll('_', ' ')}`}</dd>
      <dt>Size</dt><dd>{info.byte_length.toLocaleString()} bytes</dd><dt>SHA-256</dt><dd>{info.sha256}</dd></dl>
    <p>Full font only{info.no_subsetting ? ' / Subsetting prohibited' : ''}. Office compatibility unverified.</p>
    {!info.usable && <p>Not a supported complete static outline font.</p>}
    {(info.copyright || info.license_description || info.license_url) && <details><summary>Font notices</summary><pre style={{ whiteSpace: 'pre-wrap', maxHeight: 180, overflow: 'auto' }}>{[info.copyright, info.license_description, info.license_url].filter(Boolean).join('\n\n')}</pre></details>}
  </section>
}

export function FontPanel({ session, element, slideId, onDocument, onBusy }: {
  session: DocumentSession; element?: Element; slideId?: string
  onDocument: (document: AislideDocument) => void; onBusy: (busy: boolean) => void
}) {
  const [selected, setSelected] = useState<{ info: FontInfo; base64: string } | null>(null)
  const [consent, setConsent] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [status, setStatus] = useState('')
  const pending = useRef(false)
  const mounted = useRef(false)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false } }, [])
  const fonts = session.document.deck.embedded_fonts ?? []
  const canAssign = element && (element.type === 'text' || element.type === 'shape') && !element.visual?.locked && Boolean(element.text) && slideId
  const permitted = selected?.info.usable && ['installable', 'editable'].includes(selected.info.permission)
  async function run(action: () => Promise<void>) {
    if (pending.current) return
    pending.current = true; setBusy(true); onBusy(true); setError(''); setStatus('')
    try { await action() } catch (reason) { if (mounted.current) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { pending.current = false; if (mounted.current) { setBusy(false); onBusy(false) } }
  }
  async function choose(file: File) {
    setSelected(null); setConsent(false)
    if (file.size > FONT_LIMITS.face_bytes) throw new Error('Font exceeds 12 MiB per face')
    const base64 = await fileBase64(file)
    const info = await client.inspectFont(base64)
    if (mounted.current) setSelected({ info, base64 })
  }
  return <div className="workspace-form" style={{ maxWidth: 760, overflowWrap: 'anywhere' }}>
    <fieldset disabled={busy} style={{ border: 0, padding: 0, minWidth: 0 }}>
      <label className="field">Font file<input type="file" accept=".ttf,.otf" aria-label="Font file" onChange={(event) => {
        const file = event.currentTarget.files?.[0]; event.currentTarget.value = ''
        if (file) void run(() => choose(file))
      }} /></label>
      <p>Embedding bits do not prove a license. Confirm the font owner's terms permit document embedding and editing.</p>
      <p>Static TTF/OTF only, 12 MiB per face, 24 MiB combined, up to 8 faces. Active {session.capacityProfile} document limit: {CAPACITY_PROFILES[session.capacityProfile].document_bytes / 1048576} MiB including immutable original bytes and fonts. Full CJK fonts may still exceed the complete-document limit. Variable fonts and TTC collections remain unsupported; no automatic subsetting.</p>
      {selected && <><FontDetails info={selected.info} />
        <label className="checkbox"><input type="checkbox" checked={consent} disabled={!permitted} onChange={(event) => setConsent(event.target.checked)} />I have permission to embed this font and edit documents using it.</label>
        <button className="primary" type="button" disabled={!consent || !permitted} onClick={() => void run(async () => {
          const result = await session.embedFont({ base64: selected.base64, license_acknowledged: consent }, { expectedRevision: session.revision })
          onDocument(result); setSelected(null); setConsent(false); setStatus('Font attached. Unused families are omitted from PPTX.')
        })}><FilePlus2 size={18} />Embed full font</button>
      </>}
      <h3>Document fonts</h3>
      {fonts.length === 0 && <p>No supported embedded fonts.</p>}
      {fonts.map((font) => <section key={`${font.family}:${font.style}`} style={{ borderTop: '1px solid #b8bdc2', paddingBlock: 12 }}>
        <strong>{font.family}</strong><p>{font.style.replaceAll('_', ' ')} / {font.license_acknowledged ? 'Consent recorded for this session' : 'Consent required'}</p>
        <button type="button" className="secondary" onClick={() => void run(async () => { setSelected({ info: await client.inspectFont(font.base64), base64: font.base64 }); setConsent(false) })}><Check size={16} />Inspect {font.family}</button>
        {!font.license_acknowledged && <p>Inspect and confirm permission before loading.</p>}
        <button type="button" className="secondary" disabled={!canAssign || !font.license_acknowledged} aria-label={`Apply ${font.family} to selected text`} onClick={() => void run(async () => {
          if (!element || !slideId || (element.type !== 'text' && element.type !== 'shape')) return
          const result = await session.formatText(slideId, { id: element.id, start: 0, end: [...element.text].length, style: { font_family: font.family } }, { expectedRevision: session.revision })
          onDocument(result); setStatus('Font assigned')
        })}><Type size={16} />Apply to selected text</button>
      </section>)}
    </fieldset>
    {error && <p className="error" role="alert">{error}</p>}
    <p role="status">{busy ? 'Processing font' : status}</p>
  </div>
}