import { useEffect, useRef, useState } from 'react'
import { AlertCircle, Check, LoaderCircle, Sparkles, Square } from 'lucide-react'
import { core } from './api'
import { SlideSurface } from './SlideSurface'
import type { GeneratedReport, ProviderStatus } from './types'

export function GenerationPanel({ provider, onApply, onBusy }: { provider: ProviderStatus; onApply: (draft: GeneratedReport) => void; onBusy: (busy: boolean) => void }) {
  const [prompt, setPrompt] = useState('')
  const [source, setSource] = useState('')
  const [slideCount, setSlideCount] = useState(12)
  const [consent, setConsent] = useState(false)
  const [repair, setRepair] = useState(false)
  const [outline, setOutline] = useState('')
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')
  const [draft, setDraft] = useState<GeneratedReport | null>(null)
  const running = useRef<AbortController | null>(null)
  useEffect(() => () => running.current?.abort(), [])

  async function generate() {
    if (running.current) return
    const controller = new AbortController()
    running.current = controller
    setBusy(true)
    onBusy(true)
    setDraft(null)
    setError('')
    setMessage('Waiting for model response')
    try {
      const result = await core<GeneratedReport>({ op: 'generate', input: { prompt, source_text: source, slide_count: slideCount, allow_remote: consent, max_repairs: repair ? 1 : 0, outline: outline.trim() ? JSON.parse(outline) : [] } }, { signal: controller.signal })
      if (controller.signal.aborted) { setMessage('Generation cancelled'); return }
      setDraft(result)
      setMessage('Draft ready for review')
    } catch (reason) {
      if (controller.signal.aborted) setMessage('Generation cancelled')
      else { setMessage('Generation failed'); setError(reason instanceof Error ? reason.message : String(reason)) }
    } finally {
      running.current = null
      setBusy(false)
      onBusy(false)
    }
  }

  return <form className="generation-form" onSubmit={(event) => { event.preventDefault(); void generate() }}>
    <div className="provider-details"><strong>{provider.model ?? 'Provider not configured'}</strong><span>{provider.remote ? 'REMOTE / HTTPS' : 'LOCAL / LOOPBACK'}</span><code>{provider.endpoint}</code></div>
    {!provider.configured && <p className="warning">{provider.message}</p>}
    <fieldset disabled={busy} className="generation-fields">
      <label className="field generation-wide">Brief<textarea aria-label="Brief" rows={3} required maxLength={8000} value={prompt} onChange={(event) => setPrompt(event.target.value)} /></label>
      <label className="field generation-wide">Source text<textarea aria-label="Source text" rows={5} maxLength={24000} value={source} onChange={(event) => setSource(event.target.value)} /></label>
      <label className="field generation-count">Slide count<input aria-label="Slide count" type="number" min={1} max={32} step={1} required value={slideCount} onChange={(event) => setSlideCount(event.currentTarget.valueAsNumber)} /></label>
      <label className="checkbox"><input type="checkbox" checked={repair} onChange={(event) => setRepair(event.target.checked)} />Allow one validation repair</label>
      <details><summary>Approved outline</summary><label className="field">Outline JSON<textarea aria-label="Outline JSON" className="code-input" rows={5} value={outline} onChange={(event) => setOutline(event.target.value)} /></label></details>
      {provider.remote && <label className="checkbox generation-wide remote-consent"><input type="checkbox" checked={consent} onChange={(event) => setConsent(event.target.checked)} /><span>I approve sending this brief and source text to the remote endpoint shown above.</span></label>}
    </fieldset>
    {message && <div className="generation-status" role="status">{busy && <LoaderCircle size={16} className="working-icon" />}{message}</div>}
    {error && <p className="error" role="alert">{error}</p>}
    {draft && <section className="generation-review" aria-label="Generated draft">
      <div className="draft-heading"><h3>{draft.report.title}</h3><span>{draft.compiled.deck.slides.length} slides</span></div>
      <p className="draft-warning"><AlertCircle size={16} />AI content unverified</p>
      <div className="draft-preview"><SlideSurface slide={draft.compiled.deck.slides[0]} /></div>
      <ol className="draft-outline">{draft.compiled.deck.slides.map((slide) => <li key={slide.id}>{slide.title}</li>)}</ol>
      <p className="warning">{draft.compiled.issues.find((issue) => issue.code === 'AI_CONTENT_UNVERIFIED')?.message}</p>
    </section>}
    <div className="generation-actions">
      {busy ? <button key="cancel" type="button" className="secondary" onClick={(event) => { event.preventDefault(); setMessage('Cancelling generation'); running.current?.abort() }}><Square size={15} />Cancel generation</button>
        : <button key="generate" type="submit" className="secondary" disabled={!provider.configured || !prompt.trim() || (provider.remote && !consent)}><Sparkles size={16} />Generate draft</button>}
      {draft && <button type="button" className="primary" disabled={busy} onClick={() => onApply(draft)}><Check size={16} />Apply draft</button>}
    </div>
  </form>
}