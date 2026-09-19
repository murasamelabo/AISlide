import type { AxisOptions, ChartData, ChartDataLabels } from './types'

export function numberFormat(value: unknown, format?: string | null): string {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return String(value ?? '')
  if (!format || format.toLowerCase() === 'general') return String(Number(numeric.toPrecision(12)))
  const section = format.split(';')[numeric < 0 && format.includes(';') ? 1 : 0].replace(/\[[^\]]*\]/g, '')
  const decimal = section.match(/\.([0#]+)/)?.[1]
  const percent = section.includes('%')
  const negativeSection = numeric < 0 && format.includes(';')
  const magnitude = (negativeSection ? Math.abs(numeric) : numeric) * (percent ? 100 : 1)
  const scientific = /[Ee][+-]?0+/.test(section)
  const rendered = scientific ? magnitude.toExponential(Math.min(decimal?.length ?? 2, 12)) : new Intl.NumberFormat('en-US', { useGrouping: section.includes(','), minimumFractionDigits: Math.min(decimal?.replace(/#/g, '').length ?? 0, 12), maximumFractionDigits: Math.min(decimal?.length ?? 0, 12) }).format(magnitude)
  const literals = (value: string) => value.replace(/"([^"]*)"|\\(.)|[_*].|[0#?,.%Ee+]/g, (_token, quoted: string | undefined, escaped: string | undefined) => quoted ?? escaped ?? '')
  const pattern = section.match(/[0#?,]+(?:\.[0#?]+)?(?:[Ee][+-]?0+)?%?/)
  if (!pattern || pattern.index == null) return String(numeric)
  return `${literals(section.slice(0, pattern.index))}${rendered}${percent ? '%' : ''}${literals(section.slice(pattern.index + pattern[0].length))}`
}

export function axisDomain(options: AxisOptions | undefined, values: number[], zero = true, percent = false): [number, number] {
  const valid = values.filter((value) => Number.isFinite(value) && (!options?.log_base || value > 0))
  const minimum = valid.length ? Math.min(...valid) : options?.log_base ? 1 : 0
  const maximum = valid.length ? Math.max(...valid) : options?.log_base ? 10 : 1
  let low = options?.min ?? (options?.log_base || !zero ? minimum : Math.min(0, minimum))
  let high = options?.max ?? (percent ? 1 : options?.log_base || !zero ? maximum : Math.max(0, maximum))
  if (options?.log_base) {
    const base = options.log_base
    low = options.min ?? Math.pow(base, Math.floor(Math.log(Math.max(low, Number.MIN_VALUE)) / Math.log(base)))
    high = options.max ?? Math.pow(base, Math.ceil(Math.log(Math.max(high, Number.MIN_VALUE)) / Math.log(base)))
  }
  if (high <= low) {
    const delta = options?.log_base ? Math.max(low, 1) * (options.log_base - 1) : Math.max(Math.abs(low) * 0.1, 1)
    if (options?.max != null && options.min == null) low = options.log_base ? high / options.log_base : high - delta
    else high = low + delta
  }
  return [low, high]
}

export function axisTicks(options: AxisOptions | undefined, domain: [number, number], minor = false): number[] | undefined {
  const [minimum, maximum] = domain
  if (options?.log_base && !minor) {
    const base = options.log_base
    const first = Math.ceil(Math.log(minimum) / Math.log(base) - 1e-10)
    const count = Math.floor(Math.log(maximum) / Math.log(base) + 1e-10) - first + 1
    return count > 0 && count <= 128 ? Array.from({ length: count }, (_, index) => Math.pow(base, first + index)) : undefined
  }
  const unit = minor ? options?.minor_unit : options?.major_unit
  if (!unit || unit <= 0) return undefined
  const first = Math.ceil(minimum / unit) * unit
  const count = Math.floor((maximum - first) / unit + 1e-10) + 1
  if (!Number.isFinite(count) || count < 1 || count > 128) return undefined
  return Array.from({ length: count }, (_, index) => Number((first + index * unit).toPrecision(14)))
}

export function dataLabel(chart: ChartData, seriesIndex: number, index: number, labels?: ChartDataLabels | null): string {
  if (!labels) return ''
  const series = chart.series[seriesIndex]
  const value = series.values[index]
  const total = ['pie', 'pie3d', 'doughnut'].includes(chart.kind) ? series.values.reduce((sum, entry) => sum + entry, 0) : chart.series.reduce((sum, entry) => sum + entry.values[index], 0)
  return [labels.show_series_name ? series.name : '', labels.show_category_name ? chart.categories[index] : '', labels.show_value ? numberFormat(value, labels.number_format) : '', labels.show_percent ? numberFormat(total ? value / total : 0, labels.number_format?.includes('%') ? labels.number_format : '0%') : ''].filter(Boolean).join(' ')
}

export function waterfallData(chart: ChartData): { category: string; value: number; range: [number, number]; total: boolean }[] {
  if (chart.kind !== 'waterfall' || chart.options?.waterfall_totals == null || chart.series.length !== 1) return []
  const totals = new Set(chart.options.waterfall_totals)
  let running = 0
  return chart.categories.map((category, index) => {
    const value = chart.series[0].values[index]
    const total = totals.has(index)
    const start = total ? 0 : running
    running = total ? value : running + value
    return { category, value, range: [start, running], total }
  })
}

export function chartWarnings(chart: ChartData): string[] {
  const warnings: string[] = []
  if (chart.kind === 'funnel' || chart.kind === 'waterfall') warnings.push('Approximate chartEx preview; Office 2016+ native layout is not visually qualified')
  if (chart.kind === 'waterfall' && chart.options?.waterfall_totals == null) warnings.push('Missing explicit waterfall total markers')
  if (chart.kind.endsWith('3d')) warnings.push('3D preview: 2D projection')
  const limitedFormat = (format: string) => format.toLowerCase() !== 'general' && (!/[0#]/.test(format) || !/^[0#?,.E+%$()\- ;]*$/i.test(format.replace(/"[^"]*"|\\./g, '')))
  for (const [name, options] of [['primary', chart.options?.primary_axis], ['secondary', chart.options?.secondary_axis], ['category', chart.options?.category_axis]] as const) {
    if (options?.minor_unit) warnings.push(`${name} minor units ${chart.kind.startsWith('radar') ? 'are preserved, not previewed on radar' : 'are previewed as gridlines'}`)
    if (options?.number_format && limitedFormat(options.number_format)) warnings.push(`${name} number format uses a numeric preview subset`)
    if (options?.major_unit || options?.minor_unit) warnings.push(`${name} tick generation is capped at 128 per axis`)
  }
  if (chart.options?.data_labels?.position === 'best_fit') warnings.push('Best-fit label placement uses Recharts layout')
  if (chart.options?.data_labels?.number_format && limitedFormat(chart.options.data_labels.number_format)) warnings.push('Data-label number format uses a numeric preview subset')
  return warnings
}