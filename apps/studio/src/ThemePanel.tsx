import { useState } from 'react'
import { Check } from 'lucide-react'
import type { Theme } from './types'

export function ThemePanel({ theme, onApply, onBusy }: { theme: Theme; onApply: (theme: Theme) => Promise<void>; onBusy: (busy: boolean) => void }) {
  const [draft, setDraft] = useState(structuredClone(theme))
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  async function apply() {
    if (busy) return
    setBusy(true); onBusy(true); setError('')
    try { await onApply(draft) }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { setBusy(false); onBusy(false) }
  }
  return <form className="theme-panel" onSubmit={(event) => { event.preventDefault(); void apply() }}>
    <fieldset disabled={busy} className="theme-fields">
      <label className="field">Theme name<input aria-label="Theme name" value={draft.name} maxLength={80} required onChange={(event) => setDraft({ ...draft, name: event.target.value })} /></label>
      <div className="theme-colors">{Object.entries(draft.colors).map(([key, color]) => <label className="field" key={key}>{key}<input type="color" aria-label={`Theme ${key}`} value={`#${color}`} onChange={(event) => setDraft({ ...draft, colors: { ...draft.colors, [key]: event.target.value.slice(1).toUpperCase() } })} /></label>)}</div>
      <div className="theme-fonts">{([['major', 'Heading font'], ['minor', 'Body font'], ['east_asian', 'East Asian font'], ['complex_script', 'Complex script font']] as const).map(([key, label]) => <label className="field" key={key}>{label}<input aria-label={label} list="theme-fonts" value={draft.fonts[key]} required maxLength={100} onChange={(event) => setDraft({ ...draft, fonts: { ...draft.fonts, [key]: event.target.value } })} /></label>)}</div>
      <datalist id="theme-fonts">{['Aptos', 'Arial', 'Yu Gothic', 'Meiryo', 'Segoe UI', 'Times New Roman', 'Georgia', 'Noto Sans JP'].map((font) => <option key={font} value={font} />)}</datalist>
    </fieldset>
    {error && <p className="error" role="alert">{error}</p>}
    <div className="modal-actions"><span>{draft.name}</span><button className="primary" type="submit" disabled={busy}><Check size={17} />Apply theme</button></div>
  </form>
}