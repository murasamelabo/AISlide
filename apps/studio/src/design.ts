import type { CSSProperties } from 'react'
import type { ChartKind, Element, RunStyle, Theme } from './types'

const defaultColors: Record<string, string> = { dk1: '202525', lt1: 'FFFFFF', dk2: '586563', lt2: 'EDF3F0', accent1: '087F73', accent2: 'CF5847', accent3: '416CA5', accent4: 'C68B19', accent5: '677C54', accent6: '9A506B', hlink: '0066CC', folHlink: '734C8C' }

export function cssColor(value: string, theme?: Theme): string {
  return `#${value.startsWith('@') ? (theme?.colors[value.slice(1)] ?? defaultColors[value.slice(1)] ?? '202525') : value}`
}

export function displayFieldElement(element: Element, pageNumber?: number): Element {
  if (pageNumber == null || (element.type !== 'text' && element.type !== 'shape') || !element.format?.paragraphs?.some((paragraph) => paragraph.runs.some((run) => run.field?.kind === 'slidenum'))) return element
  const paragraphs = element.format.paragraphs.map((paragraph) => ({ ...paragraph, runs: paragraph.runs.map((run) => run.field?.kind === 'slidenum' ? { ...run, text: String(pageNumber) } : run) }))
  return { ...element, text: paragraphs.map((paragraph) => paragraph.runs.map((run) => run.text).join('')).join('\n'), format: { ...element.format, paragraphs } }
}

export function fontFamily(family?: string | null, theme?: Theme): string {
  const latin = family === '@major' ? theme?.fonts.major : family === '@minor' || !family ? theme?.fonts.minor : family
  return [latin ?? 'Aptos', theme?.fonts.east_asian ?? 'Yu Gothic', theme?.fonts.complex_script ?? 'Arial'].map((font) => JSON.stringify(font)).join(', ') + ', sans-serif'
}

export function textStyle(element: Extract<Element, { type: 'text' | 'shape' }>, theme?: Theme): CSSProperties {
  return { fontSize: element.font_size, fontFamily: fontFamily(element.format?.font_family, theme), color: cssColor(element.color, theme), fontWeight: element.bold ? 700 : 400, fontStyle: element.format?.italic ? 'italic' : 'normal', textDecoration: element.format?.underline ? 'underline' : 'none', textAlign: element.format?.alignment ?? 'left' }
}

export function runStyle(style: RunStyle | null | undefined, theme?: Theme, size = 24): CSSProperties {
  if (!style) return {}
  return Object.fromEntries(Object.entries({
    fontWeight: style.bold == null ? undefined : style.bold ? 700 : 400,
    fontStyle: style.italic == null ? undefined : style.italic ? 'italic' : 'normal',
    textDecoration: style.underline == null ? undefined : style.underline ? 'underline' : 'none',
    fontSize: style.font_size ?? undefined,
    color: style.color ? cssColor(style.color, theme) : undefined,
    fontFamily: style.font_family ? fontFamily(style.font_family, theme) : undefined,
    backgroundColor: style.highlight ? cssColor(style.highlight, theme) : undefined,
    verticalAlign: style.baseline == null ? undefined : `${style.baseline / 100000 * (style.font_size ?? size)}px`,
  }).filter(([, value]) => value !== undefined)) as CSSProperties
}

export const chartNames: Record<ChartKind, string> = { column: 'Column', bar: 'Bar', line: 'Line', pie: 'Pie', doughnut: 'Doughnut', area: 'Area', scatter: 'Scatter', stacked_column: 'Stacked column', stacked_bar: 'Stacked bar', percent_stacked_column: '100% stacked column', percent_stacked_bar: '100% stacked bar', combo: 'Combination', bubble: 'Bubble', radar: 'Radar', radar_filled: 'Filled radar', column3d: '3D column', bar3d: '3D bar', pie3d: '3D pie', funnel: 'Funnel', waterfall: 'Waterfall', histogram: 'Histogram', box_whisker: 'Box and whisker', treemap: 'Treemap', sunburst: 'Sunburst' }