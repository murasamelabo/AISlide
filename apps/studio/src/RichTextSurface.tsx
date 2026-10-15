import { Fragment, useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import type { Element, ElementPreview, RichParagraph, RichRun, RichSpacing, RunStyle, TextFormat, Theme, TextWarp } from './types'
import { runStyle } from './design'
import { core } from './api'

let previewQueue = Promise.resolve()
const previewCache = new Map<string, ElementPreview>()

export function CoreElementPreview({ element, theme, label }: { element: Extract<Element, { type: 'chart' | 'text' | 'shape' }>; theme?: Theme; label: string }) {
  const key = JSON.stringify({ element, theme })
  const [state, setState] = useState<{ key: string; result?: ElementPreview; error?: string }>()
  useEffect(() => {
    let disposed = false
    const task = async () => {
      if (disposed) return
      try {
        let result = previewCache.get(key)
        if (!result) {
          result = await core<ElementPreview>({ op: 'render_element_preview', ...JSON.parse(key) })
          if (result.svg.length <= 262144) {
            if (previewCache.size >= 24) previewCache.delete(previewCache.keys().next().value!)
            previewCache.set(key, result)
          }
        }
        if (!disposed) setState({ key, result })
      } catch (error) {
        if (!disposed) setState({ key, error: error instanceof Error ? error.message : 'Preview failed' })
      }
    }
    previewQueue = previewQueue.then(task, task)
    return () => { disposed = true }
  }, [key])
  const current = state?.key === key ? state : undefined
  const warnings = current?.result?.warnings.map((warning) => warning.message).join('; ')
  return <div data-core-preview={current?.result ? 'ready' : current?.error ? 'error' : 'pending'} data-preview-warnings={warnings} title={warnings} style={{ width: element.width, height: element.height, overflow: 'hidden' }}>
    {current?.result ? <img alt={label} draggable={false} src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(current.result.svg)}`} style={{ width: '100%', height: '100%', display: 'block' }} /> : current?.error ? <span role="alert">{current.error}</span> : <span role="status" aria-label="Rendering preview" />}
  </div>
}

function WarpedText({ text, format, size, theme, warp, baseStyle }: { text: string; format?: TextFormat | null; size: number; theme?: Theme; warp: TextWarp; baseStyle?: RunStyle | null }) {
  const host = useRef<HTMLDivElement>(null)
  const [frame, setFrame] = useState<{ width: number; height: number; color: string; bold: boolean }>()
  useEffect(() => {
    const parent = host.current?.parentElement
    if (!parent) return
    const update = () => {
      const style = getComputedStyle(parent)
      const channels = style.color.match(/[\d.]+/g)?.slice(0, 3).map(Number) ?? [0, 0, 0]
      const next = { width: parent.clientWidth, height: parent.clientHeight, color: channels.map((channel) => Math.round(channel).toString(16).padStart(2, '0')).join('').toUpperCase(), bold: Number(style.fontWeight) >= 600 }
      setFrame((previous) => JSON.stringify(previous) === JSON.stringify(next) ? previous : next)
    }
    update()
    const observer = new ResizeObserver(update)
    observer.observe(parent)
    return () => observer.disconnect()
  }, [text, format, size, theme, warp, baseStyle])
  const element: Extract<Element, { type: 'text' }> | undefined = frame && frame.width > 0 && frame.height > 0 ? { type: 'text', id: 'wordart-preview', x: 0, y: 0, width: frame.width, height: frame.height, text, font_size: size, color: frame.color, bold: frame.bold, format: format ? { ...format, placeholder: null, inherit_layout: false } : undefined, visual: { text_warp: warp } } : undefined
  return <div ref={host} style={{ position: 'absolute', inset: 0 }}>{element && <CoreElementPreview element={element} theme={theme} label={text} />}</div>
}

function spacing(value: RichSpacing | null | undefined, size: number, line = false): number | string | undefined {
  if (!value) return undefined
  return value.kind === 'points' ? `${value.value / 75}px` : line ? value.value / 100000 : size * value.value / 100000
}

function ordinal(value: number, kind: RichParagraph['numbering']): string {
  let label = String(value)
  if (kind?.startsWith('alpha')) {
    label = ''
    for (let remaining = value; remaining > 0; remaining = Math.floor((remaining - 1) / 26)) label = String.fromCharCode(65 + (remaining - 1) % 26) + label
    if (kind.includes('Lc')) label = label.toLowerCase()
  } else if (kind?.startsWith('roman')) {
    label = ''
    let remaining = Math.min(value, 3999)
    for (const [amount, letters] of [[1000, 'M'], [900, 'CM'], [500, 'D'], [400, 'CD'], [100, 'C'], [90, 'XC'], [50, 'L'], [40, 'XL'], [10, 'X'], [9, 'IX'], [5, 'V'], [4, 'IV'], [1, 'I']] as const) {
      while (remaining >= amount) { label += letters; remaining -= amount }
    }
    if (kind.includes('Lc')) label = label.toLowerCase()
  }
  return kind === 'arabicPlain' ? label : kind === 'arabicParenBoth' ? `(${label})` : kind?.endsWith('ParenR') ? `${label})` : `${label}.`
}

function runsContent(runs: RichRun[], size: number, theme?: Theme): ReactNode[] {
  return runs.map((run, index) => {
    const field = 'field' in run && run.field && typeof run.field === 'object' && 'kind' in run.field ? String(run.field.kind) : undefined
    return <span key={index} lang={run.style?.language ?? undefined} data-field-kind={field} style={runStyle(run.style, theme, size)}>{run.text}</span>
  })
}

function paragraphContent(paragraph: RichParagraph, size: number, theme?: Theme) {
  if (!paragraph.runs.some((run) => run.text.includes('\t'))) return runsContent(paragraph.runs, size, theme)
  const groups: RichRun[][] = [[]]
  for (const run of paragraph.runs) {
    run.text.split('\t').forEach((part, index) => { if (index) groups.push([]); groups[groups.length - 1].push({ ...run, text: part }) })
  }
  const stops = paragraph.tabs ?? []
  const widths = groups.slice(0, -1).map((_, index) => Math.max(0, (stops[index]?.position ?? (index + 1) * 914400) - (stops[index - 1]?.position ?? index * 914400)) / 9525)
  return <span className="document-tabs" style={{ gridTemplateColumns: [...widths.map((width) => `${width}px`), 'minmax(0, 1fr)'].join(' ') }}>
    {groups.map((runs, index) => { const alignment = index ? stops[index - 1]?.alignment : 'left'; return <span key={index} style={{ textAlign: alignment === 'decimal' ? 'right' : alignment }}>{runsContent(runs, size, theme)}</span> })}
  </span>
}

export function RichTextSurface({ text, format, size, theme, warp, baseStyle }: { text: string; format?: TextFormat | null; size: number; theme?: Theme; warp?: TextWarp | null; baseStyle?: RunStyle | null }) {
  if (warp && (text.includes('\t') || format?.paragraphs?.some((paragraph) => paragraph.tabs?.length))) return <span data-preview-warnings="WordArt with tab stops is unsupported; unwarped rich text displayed" title="WordArt with tab stops is unsupported; unwarped rich text displayed"><RichTextSurface text={text} format={format} size={size} theme={theme} baseStyle={baseStyle} /></span>
  if (warp) return <WarpedText text={text} format={format} size={size} theme={theme} warp={warp} baseStyle={baseStyle} />
  if (!text && !format?.paragraphs?.length && (!format?.bullet || format.bullet === 'none')) return null
  const paragraphs: RichParagraph[] = format?.paragraphs?.length ? format.paragraphs : text.split('\n').map((line) => ({ runs: [{ text: line }] }))
  const counters = new Map<number, number>()
  return <>{paragraphs.map((sourceParagraph, index) => {
    const paragraph = { ...sourceParagraph, runs: sourceParagraph.runs.map((run) => ({ ...run, style: { underline: format?.underline, ...baseStyle, ...Object.fromEntries(Object.entries(run.style ?? {}).filter(([, value]) => value != null)) } })) }
    const level = paragraph.level ?? 0
    const bullet = paragraph.bullet ?? format?.bullet ?? 'none'
    const value = paragraph.number_start ?? (counters.get(level) ?? 0) + 1
    if (bullet === 'numbered') counters.set(level, value)
    else counters.delete(level)
    for (const key of counters.keys()) if (key > level) counters.delete(key)
    const marker = bullet === 'numbered' ? ordinal(value, paragraph.numbering) : bullet === 'bullet' ? paragraph.bullet_character ?? '\u2022' : null
    return <Fragment key={index}>{index > 0 && '\n'}<div className="document-paragraph" data-level={level} style={{ textAlign: paragraph.alignment ?? format?.alignment, marginLeft: paragraph.margin_left == null ? level * size * 0.75 : paragraph.margin_left / 9525, textIndent: paragraph.indent == null ? undefined : paragraph.indent / 9525, lineHeight: spacing(paragraph.line_spacing, size, true), marginTop: spacing(paragraph.space_before, size), marginBottom: spacing(paragraph.space_after, size), paddingLeft: marker && paragraph.margin_left == null ? '1.3em' : undefined }}>
      {marker && <span className="document-list-marker" style={{ marginLeft: paragraph.indent == null ? '-1.3em' : undefined }}>{marker}</span>}
      {paragraphContent(paragraph, size, theme)}
      {paragraph.runs.every((run) => !run.text) && <br />}
    </div></Fragment>
  })}</>
}