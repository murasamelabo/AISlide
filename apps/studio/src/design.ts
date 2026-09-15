import type { CSSProperties } from 'react'
import type { Element, Theme } from './types'

const defaultColors: Record<string, string> = { dk1: '202525', lt1: 'FFFFFF', dk2: '586563', lt2: 'EDF3F0', accent1: '087F73', accent2: 'CF5847', accent3: '416CA5', accent4: 'C68B19', accent5: '677C54', accent6: '9A506B', hlink: '0066CC', folHlink: '734C8C' }

export function cssColor(value: string, theme?: Theme): string {
  return `#${value.startsWith('@') ? (theme?.colors[value.slice(1)] ?? defaultColors[value.slice(1)] ?? '202525') : value}`
}

export function fontFamily(family?: string | null, theme?: Theme): string {
  const latin = family === '@major' ? theme?.fonts.major : family === '@minor' || !family ? theme?.fonts.minor : family
  return [latin ?? 'Aptos', theme?.fonts.east_asian ?? 'Yu Gothic', theme?.fonts.complex_script ?? 'Arial'].map((font) => JSON.stringify(font)).join(', ') + ', sans-serif'
}

export function textStyle(element: Extract<Element, { type: 'text' | 'shape' }>, theme?: Theme): CSSProperties {
  return { fontSize: element.font_size, fontFamily: fontFamily(element.format?.font_family, theme), color: cssColor(element.color, theme), fontWeight: element.bold ? 700 : 400, fontStyle: element.format?.italic ? 'italic' : 'normal', textDecoration: element.format?.underline ? 'underline' : 'none', textAlign: element.format?.alignment ?? 'left' }
}

export const chartNames = { column: 'Column', bar: 'Bar', line: 'Line', pie: 'Pie', doughnut: 'Doughnut', area: 'Area', scatter: 'Scatter', stacked_column: 'Stacked column', stacked_bar: 'Stacked bar', percent_stacked_column: '100% stacked column' }