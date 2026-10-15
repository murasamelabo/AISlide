import { useEffect, useRef, useState } from 'react'
import { Search, Replace, ReplaceAll, SpellCheck, Languages, X, Check, WandSparkles, Settings } from 'lucide-react'
import type { DocumentSession } from '../../../packages/client/index.mjs'
import type { AislideDocument, Element, SearchOptions, TextMatch } from './types'
import type { ProofingDictionary, ProofResult, ProofIssue, TextAssistance, TextAssistInput } from './types'
import './text-tools.css'

export type TextToolsPanelProps = {
  session: DocumentSession
  onDocument: (document: AislideDocument) => void
  onNavigate: (slideIndex: number, elementId?: string, path?: string) => void
  onNavigatePath?: (path: string) => void
  onBusy?: (busy: boolean) => void
  disabled?: boolean
  spellCheck?: boolean
  onSpellCheckChange?: (enabled: boolean) => void
  proofingSelection?: { slideId: string; id: string }
}

type SearchResult = { session: DocumentSession; revision: number; key: string; matches: TextMatch[] }

export function TextToolsPanel({ session, onDocument, onNavigate, onNavigatePath, onBusy, disabled = false, spellCheck = false, onSpellCheckChange, proofingSelection }: TextToolsPanelProps) {
  const [tab, setTab] = useState<'find' | 'proofing' | 'ai'>('find')
  const [search, setSearch] = useState<SearchOptions>({ query: '', case_sensitive: false, whole_word: false, include_notes: false, include_masters: false, max_matches: 200 })
  const [replacement, setReplacement] = useState('')
  const [result, setResult] = useState<SearchResult | null>(null)
  const [selected, setSelected] = useState<number[]>([])
  const [fromFont, setFromFont] = useState('')
  const [toFont, setToFont] = useState('')
  const [working, setWorking] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const pending = useRef(false)
  const generation = useRef(0)
  const mounted = useRef(true)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false } }, [])
  useEffect(() => { generation.current += 1; return () => { generation.current += 1 } }, [session])
  const key = JSON.stringify(search)
  const current = result?.session === session && result.revision === session.revision && result.key === key ? result : null
  const busy = disabled || working || session.busy
  const snapshot = session.document
  const staleParts = snapshot.parts?.filter((part) => part.stale).length ?? 0
  const staleBindings = snapshot.bindings.filter((binding) => binding.stale).length

  async function run(action: (active: () => boolean) => Promise<void>) {
    if (busy || pending.current) return
    pending.current = true; setWorking(true); onBusy?.(true); setError(''); setMessage('')
    const token = generation.current
    const active = () => token === generation.current
    try { await action(active) }
    catch (reason) { if (active()) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { pending.current = false; if (mounted.current) setWorking(false); onBusy?.(false) }
  }
  function find() {
    if (!search.query) return
    void run(async (active) => {
      const revision = session.revision
      const matches = await session.searchText(search)
      if (!active() || session.revision !== revision) return
      setResult({ session, revision, key, matches }); setSelected([])
      setMessage(matches.length === 200 ? '200 matches shown; additional matches may exist.' : `${matches.length} matches`)
    })
  }
  function replace(all: boolean) {
    if (!current || (!all && !selected.length)) return
    void run(async (active) => {
      const document = await session.replaceText({ search, replacement, ...(all ? { replace_all: true } : { selected: selected.map((index) => current.matches[index]) }) }, { expectedRevision: current.revision })
      if (!active()) return
      onDocument(document); setResult(null); setSelected([]); setMessage('Replacement applied.')
    })
  }
  function navigate(match: TextMatch) {
    const path = match.path.split('/').slice(1).map((part) => part.replaceAll('~1', '/').replaceAll('~0', '~'))
    if (path[0] !== 'slides') { onNavigatePath?.(match.path); return }
    const slideIndex = Number(path[1])
    const slide = snapshot.deck.slides[slideIndex]
    if (!Number.isInteger(slideIndex) || !slide) return
    let element: Element | undefined = path[2] === 'elements' ? slide.elements[Number(path[3])] : undefined
    for (let index = 4; path[index] === 'children' && element?.type === 'group'; index += 2) element = element.children[Number(path[index + 1])]
    onNavigate(slideIndex, element?.id, match.path)
  }
  function setOptions(patch: Partial<SearchOptions>) { setSearch((value) => ({ ...value, ...patch })); setResult(null); setSelected([]); setMessage('') }

  return <section className="text-tools-panel" aria-label="Text tools" aria-busy={busy} onKeyDown={(event) => event.stopPropagation()}>
    <div role="tablist" aria-label="Text tools views" className="panel-tabs" onKeyDown={event => {
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key) || busy) return
      event.preventDefault()
      const tabs = ['find', 'proofing', 'ai'] as const
      const next = event.key === 'Home' ? 'find' : event.key === 'End' ? 'ai' : tabs[(tabs.indexOf(tab) + (event.key === 'ArrowRight' ? 1 : 2)) % 3]
      setTab(next); event.currentTarget.querySelector<HTMLButtonElement>(`[data-tab="${next}"]`)?.focus()
    }}>
      <button type="button" role="tab" id="text-find-tab" data-tab="find" aria-controls="text-find-view" aria-selected={tab === 'find'} tabIndex={tab === 'find' ? 0 : -1} disabled={busy} onClick={() => setTab('find')}><Search size={18} aria-hidden="true" />Find and replace</button>
      <button type="button" role="tab" id="text-proof-tab" data-tab="proofing" aria-controls="text-proof-view" aria-selected={tab === 'proofing'} tabIndex={tab === 'proofing' ? 0 : -1} disabled={busy} onClick={() => setTab('proofing')}><SpellCheck size={18} aria-hidden="true" />Proofing</button>
      <button type="button" role="tab" id="text-ai-tab" data-tab="ai" aria-controls="text-ai-view" aria-selected={tab === 'ai'} tabIndex={tab === 'ai' ? 0 : -1} disabled={busy} onClick={() => setTab('ai')}><WandSparkles size={18} aria-hidden="true" />Local AI</button>
    </div>
    <div id="text-find-view" role="tabpanel" aria-labelledby="text-find-tab" hidden={tab !== 'find'} style={{ display: tab === 'find' ? 'grid' : 'none', gap: 12 }}>
    <h2>Find and replace</h2>
    <form onSubmit={(event) => { event.preventDefault(); find() }}><fieldset disabled={busy}>
      <label className="field">Find text<input aria-label="Find text" value={search.query} maxLength={4000} required onChange={(event) => setOptions({ query: event.target.value })} /></label>
      <div className="text-tools-options">{([['case_sensitive', 'Match case'], ['whole_word', 'Whole word'], ['include_notes', 'Include notes'], ['include_masters', 'Include masters and layouts']] as const).map(([option, label]) => <label key={option}><input type="checkbox" checked={Boolean(search[option])} onChange={(event) => setOptions({ [option]: event.target.checked })} />{label}</label>)}</div>
      <button type="submit" disabled={busy || !search.query}><Search size={18} aria-hidden="true" />Find</button>
    </fieldset></form>
    <p role="status">{current || !result ? message : 'Document changed. Run Find again.'}</p>
    <ul className="text-tools-results" aria-label="Text matches">{current?.matches.map((match, index) => <li key={`${match.path}:${match.start}`}>
      <input type="checkbox" aria-label={`Select match ${index + 1}`} disabled={busy} checked={selected.includes(index)} onChange={(event) => setSelected((values) => event.target.checked ? [...values, index] : values.filter((value) => value !== index))} />
      {match.path.startsWith('/slides/') || onNavigatePath ? <button type="button" aria-label={`Go to match ${index + 1}`} disabled={busy} onClick={() => navigate(match)}>{match.snippet}<small>{match.path}</small></button> : <span>{match.snippet}<small>{match.path}</small></span>}
    </li>)}</ul>
    <fieldset disabled={busy}>
      <label className="field">Replace with<input aria-label="Replace with" value={replacement} maxLength={4000} onChange={(event) => setReplacement(event.target.value)} /></label>
      <div className="text-tools-actions"><button type="button" disabled={busy || !current || !selected.length} onClick={() => replace(false)}><Replace size={18} aria-hidden="true" />Replace selected</button><button type="button" disabled={busy || !current?.matches.length} onClick={() => replace(true)}><ReplaceAll size={18} aria-hidden="true" />Replace all</button></div>
    </fieldset>
    <form onSubmit={(event) => {
      event.preventDefault()
      if (!fromFont.trim() || !toFont.trim() || fromFont === toFont) return
      void run(async (active) => {
        const document = await session.replaceFont(fromFont.trim(), toFont.trim(), { expectedRevision: session.revision })
        if (!active()) return
        onDocument(document); setResult(null); setSelected([]); setMessage('Font replacement applied.')
      })
    }}><fieldset disabled={busy}><legend>Replace font</legend><div className="text-tools-grid">
      <label className="field">From font<input aria-label="From font" required maxLength={100} value={fromFont} onChange={(event) => setFromFont(event.target.value)} /></label>
      <label className="field">To font<input aria-label="To font" required maxLength={100} value={toFont} onChange={(event) => setToFont(event.target.value)} /></label>
    </div><button type="submit" disabled={busy || !fromFont.trim() || !toFont.trim() || fromFont.trim() === toFont.trim()}><Replace size={18} aria-hidden="true" />Replace font</button></fieldset></form>
    {onSpellCheckChange && <div className="text-tools-options"><label><input type="checkbox" checked={spellCheck} disabled={busy} onChange={(event) => onSpellCheckChange(event.target.checked)} />Browser spellcheck</label></div>}
    {onSpellCheckChange && <small>Browser dictionaries and privacy settings apply independently of local proofing.</small>}
    {(staleParts > 0 || staleBindings > 0) && <p role="status">{staleParts} stale part records; {staleBindings} stale source bindings.</p>}
    {error && <p role="alert">{error}</p>}
    </div>
    {tab === 'proofing' && <div id="text-proof-view" role="tabpanel" aria-labelledby="text-proof-tab"><ProofingTab session={session} onDocument={onDocument} onBusy={onBusy} disabled={disabled} proofingSelection={proofingSelection} /></div>}
    {tab === 'ai' && <div id="text-ai-view" role="tabpanel" aria-labelledby="text-ai-tab"><TextAssistTab session={session} onDocument={onDocument} onBusy={onBusy} disabled={disabled} proofingSelection={proofingSelection} /></div>}
  </section>
}

function ProofingTab({ session, onDocument, onBusy, disabled, proofingSelection }: Pick<TextToolsPanelProps, 'session' | 'onDocument' | 'onBusy' | 'disabled' | 'proofingSelection'>) {
  const document = session.document
  const targets = document.deck.slides.flatMap((slide, slideIndex) => slide.elements.flatMap((element, elementIndex) => element.type === 'text' || element.type === 'shape' ? [{ element, slide, path: `/slides/${slideIndex}/elements/${elementIndex}/text`, key: `${slideIndex}:${elementIndex}` }] : []))
  const [targetKey, setTargetKey] = useState(() => targets.find(target => target.slide.id === proofingSelection?.slideId && target.element.id === proofingSelection.id)?.key ?? targets[0]?.key ?? '')
  const target = targets.find(target => target.key === targetKey)
  const [language, setLanguage] = useState('en-US')
  const [targetLanguage, setTargetLanguage] = useState('ja-JP')
  const [term, setTerm] = useState('')
  const [dictionary, setDictionary] = useState<ProofingDictionary | null>(null)
  const [dictionaryName, setDictionaryName] = useState('Tiny en-US sample (not a complete dictionary)')
  const [result, setResult] = useState<{ session: DocumentSession; revision: number; key: string; data: ProofResult } | null>(null)
  const [working, setWorking] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const pending = useRef<AbortController | null>(null)
  useEffect(() => () => { pending.current?.abort() }, [session])
  const key = JSON.stringify([targetKey, target?.element.id, language, targetLanguage, term, dictionary])
  const current = result?.session === session && result.revision === session.revision && result.key === key ? result.data : null
  const busy = Boolean(disabled || working || session.busy)
  const locked = target?.element.visual?.locked || target?.element.visual?.hidden
  async function run(action: (signal: AbortSignal) => Promise<void>) {
    if (busy || pending.current) return
    const controller = new AbortController(); pending.current = controller
    setWorking(true); onBusy?.(true); setError(''); setMessage('')
    try { await action(controller.signal) }
    catch (reason) { if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { if (!controller.signal.aborted) setWorking(false); pending.current = null; onBusy?.(false) }
  }
  function check(lookup: boolean) {
    if (!target && !lookup) return
    void run(async signal => {
      const revision = session.revision
      const data = await session.proofText({ text: lookup ? '' : target?.element.text ?? '', language, dictionary, ...(lookup && term.trim() ? { term: term.trim(), target_language: targetLanguage } : {}) }, { signal })
      if (signal.aborted || revision !== session.revision) return
      setResult({ session, revision, key, data }); setMessage(lookup ? 'Local lookup complete' : `${data.issues.length} words not in this dictionary`)
    })
  }
  function correct(issue: ProofIssue, replacement: string) {
    if (!target || !current || locked || !result) return
    void run(async signal => {
      const matches = await session.searchText({ query: issue.word, case_sensitive: true, max_matches: 1000 }, { signal })
      const match = matches.find(match => match.path === target.path && match.start === issue.start && match.end === issue.end && match.expected === target.element.text)
      if (!match) throw new Error('Text changed; check spelling again')
      const updated = await session.replaceText({ search: { query: issue.word, case_sensitive: true, max_matches: 1000 }, selected: [match], replacement }, { expectedRevision: result.revision, signal })
      if (signal.aborted) return
      onDocument(updated); setResult(null); setMessage('Correction applied')
    })
  }
  return <div className="text-tools-panel" aria-label="Local proofing">
    <fieldset disabled={busy}>
      <label className="field">Proofing object<select aria-label="Proofing object" value={targetKey} onChange={event => { setTargetKey(event.target.value); setResult(null) }}>{targets.map(target => <option key={target.key} value={target.key}>{target.slide.title}: {target.element.id}</option>)}</select></label>
      <label className="field">Proofing text<textarea aria-label="Proofing text" readOnly spellCheck={false} lang={language} value={target?.element.text ?? ''} rows={4} style={{ width: '100%', boxSizing: 'border-box', resize: 'vertical' }} /></label>
      <div className="text-tools-grid">
        <label className="field">Proofing language<input aria-label="Proofing language" value={language} maxLength={64} onChange={event => { setLanguage(event.target.value); setResult(null) }} /></label>
        <button type="button" className="secondary" disabled={busy || !target?.element.text || locked || !language} onClick={() => void run(async signal => {
          if (!target) return
          const updated = await session.setProofingLanguage(target.slide.id, { id: target.element.id, language }, { expectedRevision: document.revision, signal })
          if (signal.aborted) return
          onDocument(updated); setResult(null); setMessage('Language applied')
        })}><Languages size={18} aria-hidden="true" />Apply proofing language</button>
      </div>
      <label className="field">Dictionary file<input aria-label="Dictionary file" type="file" accept=".txt,.json" onChange={event => {
        const file = event.target.files?.[0]; event.target.value = ''
        if (!file) return
        void run(async signal => {
          if (file.size > 524288) throw new Error('Dictionary file exceeds 512 KiB')
          const content = new TextDecoder('utf-8', { fatal: true }).decode(await file.arrayBuffer())
          if (signal.aborted) return
          const loaded = await session.importProofingDictionary({ language, format: file.name.toLowerCase().endsWith('.json') ? 'json' : 'wordlist', content }, { signal })
          if (signal.aborted) return
          setDictionary(loaded); setDictionaryName(file.name); setResult(null); setMessage(`Dictionary loaded: ${loaded.words.length} words`)
        })
      }} /></label>
      <p style={{ overflowWrap: 'anywhere' }}>{dictionaryName}</p>
      <small>Local UTF-8 word list or JSON, up to 512 KiB. Supply data you are licensed to use. Word-list checking is not grammar checking; the built-in sample is not complete.</small>
      <div className="text-tools-actions"><button type="button" className="primary" disabled={busy || !target} onClick={() => check(false)}><SpellCheck size={18} aria-hidden="true" />Check spelling</button><button type="button" className="secondary" disabled={busy || !dictionary} onClick={() => { setDictionary(null); setDictionaryName('Tiny en-US sample (not a complete dictionary)'); setResult(null); setMessage('') }}><X size={18} aria-hidden="true" />Clear dictionary</button></div>
      <ul aria-label="Spelling results" style={{ paddingInlineStart: 20, maxHeight: 220, overflow: 'auto', overflowWrap: 'anywhere' }}>{current?.issues.map(issue => <li key={issue.start}><strong>{issue.word}</strong><div className="text-tools-actions">{issue.suggestions.map(suggestion => <button type="button" className="secondary" key={suggestion} disabled={busy || locked} aria-label={`Replace ${issue.word} with ${suggestion}`} onClick={() => correct(issue, suggestion)}><Replace size={16} aria-hidden="true" />{suggestion}</button>)}{!issue.suggestions.length && <span>No local suggestion</span>}</div></li>)}</ul>
      <div className="text-tools-grid"><label className="field">Lookup term<input aria-label="Lookup term" value={term} maxLength={128} onChange={event => { setTerm(event.target.value); setResult(null) }} /></label><label className="field">Target language<input aria-label="Target language" value={targetLanguage} maxLength={64} onChange={event => { setTargetLanguage(event.target.value); setResult(null) }} /></label></div>
      <button type="button" className="secondary" disabled={busy || !term.trim()} onClick={() => check(true)}><Search size={18} aria-hidden="true" />Look up term</button>
      <h3>Thesaurus</h3>{current?.synonyms.length ? <ul>{current.synonyms.map(value => <li key={value}>{value}</li>)}</ul> : <p>No local synonym result</p>}
      <h3>Term translation</h3><p>{current?.translation ?? 'No local term translation'}</p>
      <small>Exact bilingual terms from your dictionary only. This dictionary lookup does not send text to a model or external service.</small>
    </fieldset>
    <p role="status">{result && !current ? 'Inputs or document changed; check again.' : message}</p>
    {working && <button type="button" className="secondary" onClick={() => { pending.current?.abort(); setWorking(false); setMessage('Cancelled') }}><X size={18} aria-hidden="true" />Cancel proofing</button>}
    {error && <p role="alert">{error}</p>}
  </div>
}

function TextAssistTab({ session, onDocument, onBusy, disabled, proofingSelection }: Pick<TextToolsPanelProps, 'session' | 'onDocument' | 'onBusy' | 'disabled' | 'proofingSelection'>) {
  const targets = session.document.deck.slides.flatMap(slide => slide.elements.flatMap(element => element.type === 'text' || element.type === 'shape' ? [{ slide, element, key: `${slide.id}:${element.id}` }] : []))
  const [targetKey, setTargetKey] = useState(() => targets.find(target => target.slide.id === proofingSelection?.slideId && target.element.id === proofingSelection.id)?.key ?? targets[0]?.key ?? '')
  const [task, setTask] = useState<TextAssistInput['task']>('proofread')
  const [language, setLanguage] = useState('en')
  const [targetLanguage, setTargetLanguage] = useState('ja')
  const [review, setReview] = useState<{ session: DocumentSession; revision: number; key: string; original: string; result: TextAssistance } | null>(null)
  const [working, setWorking] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const pending = useRef<AbortController | null>(null)
  const mounted = useRef(true)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; pending.current?.abort() } }, [session])
  const target = targets.find(target => target.key === targetKey)
  const key = JSON.stringify([targetKey, task, language, targetLanguage])
  const current = review?.session === session && review.revision === session.revision && review.key === key ? review : null
  const busy = Boolean(disabled || working || session.busy)
  const protectedText = target?.element.visual?.locked || target?.element.visual?.hidden || target?.element.format?.paragraphs?.some(paragraph => paragraph.runs.some(run => run.field))
  async function run(action: (signal: AbortSignal) => Promise<void>) {
    if (busy || pending.current) return
    const controller = new AbortController(); pending.current = controller
    setWorking(true); onBusy?.(true); setError(''); setMessage('')
    try { await action(controller.signal) }
    catch (reason) { if (mounted.current && !controller.signal.aborted) setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { pending.current = null; if (mounted.current) setWorking(false); onBusy?.(false) }
  }
  function preview() {
    if (!target) return
    setReview(null)
    void run(async signal => {
      const revision = session.revision
      const original = target.element.text
      const result = await session.textAssist({ task, text: original, language, target_language: task === 'translate' ? targetLanguage : null }, { signal })
      if (signal.aborted || revision !== session.revision) return
      setReview({ session, revision, key, original, result }); setMessage('Candidate ready for review. Model content is unverified.')
    })
  }
  return <section aria-label="Local AI text editing" className="text-tools-panel" aria-busy={working}>
    <fieldset disabled={busy}>
      <label className="field">AI text object<select aria-label="AI text object" value={targetKey} onChange={event => { setTargetKey(event.target.value); setReview(null) }}>{targets.map(target => <option key={target.key} value={target.key}>{target.slide.title}: {target.element.id}</option>)}</select></label>
      <label className="field">AI task<select aria-label="AI task" value={task} onChange={event => { setTask(event.target.value as TextAssistInput['task']); setReview(null) }}><option value="proofread">Proofread</option><option value="translate">Translate</option></select></label>
      <datalist id="ai-text-languages"><option value="en">English</option><option value="ja">Japanese</option><option value="en-US" /><option value="ja-JP" /></datalist>
      <div className="text-tools-grid"><label className="field">Source language<input aria-label="AI source language" list="ai-text-languages" maxLength={64} value={language} onChange={event => { setLanguage(event.target.value); setReview(null) }} /></label>
        {task === 'translate' && <label className="field">Target language<input aria-label="AI target language" list="ai-text-languages" maxLength={64} value={targetLanguage} onChange={event => { setTargetLanguage(event.target.value); setReview(null) }} /></label>}</div>
      <label className="field">Original text<textarea aria-label="AI original text" readOnly spellCheck={false} value={target?.element.text ?? ''} rows={4} style={{ width: '100%', boxSizing: 'border-box', resize: 'vertical' }} /></label>
      <div className="text-tools-actions"><button type="button" disabled={busy || !target?.element.text || protectedText || !language || (task === 'translate' && !targetLanguage)} onClick={preview}><WandSparkles size={18} aria-hidden="true" />Generate text candidate</button>
        <button type="button" onClick={() => void run(async signal => { const status = await session.textAiStatus({ signal }); if (!signal.aborted) setMessage(status.remote ? 'Remote providers are disabled for local text editing.' : status.message) })}><Settings size={18} aria-hidden="true" />Check local AI setup</button></div>
    </fieldset>
    {protectedText && <p role="status">Locked, hidden or field-bearing text cannot be replaced by AI.</p>}
    {current && <section aria-label="AI text review"><h3>Review changes</h3>
      {current.original.split('\n').map((original, index) => { const proposed = current.result.candidate.text.split('\n')[index]; return <div key={index} style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', marginBlock: 8 }}>{original === proposed ? <p>{original}</p> : <><p><del>{original}</del></p><p><ins>{proposed}</ins></p></>}</div> })}
      <small>{current.result.provenance.model} · {current.result.provenance.elapsed_ms} ms · Local model</small>
      <div className="text-tools-actions"><button type="button" disabled={busy || !target || protectedText} onClick={() => void run(async signal => {
        if (!target) return
        const updated = await session.applyTextAssist(target.slide.id, { id: target.element.id, expected_text: current.original, candidate: current.result.candidate }, { expectedRevision: current.revision, signal })
        if (signal.aborted) return
        onDocument(updated); setReview(null); setMessage('Text applied. One Undo restores the original.')
      })}><Check size={18} aria-hidden="true" />Apply AI text</button><button type="button" disabled={busy} onClick={() => { setReview(null); setMessage('Candidate discarded') }}><X size={18} aria-hidden="true" />Cancel candidate</button></div>
    </section>}
    {review && !current && <p role="status">Document or inputs changed. Generate a new candidate.</p>}
    {working && <button type="button" onClick={() => { pending.current?.abort(); setReview(null); setMessage('Cancelling local AI…') }}><X size={18} aria-hidden="true" />Cancel local AI</button>}
    <p role="status">{message}</p>{error && <p role="alert">{error}</p>}
    <details><summary>Local AI setup</summary><p>Start your existing local model server and launch AISlide with AISLIDE_AI_BASE_URL and AISLIDE_AI_MODEL in its host environment. Only literal loopback endpoints are accepted. A configured status is not a connectivity test.</p><p>No model is downloaded automatically. Review every candidate before applying. Existing object text limits still apply.</p></details>
  </section>
}