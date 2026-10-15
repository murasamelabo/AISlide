import { Area, Bar, CartesianGrid, Cell, ComposedChart, Funnel, FunnelChart, LabelList, Legend, Line, Pie, PieChart, PolarAngleAxis, PolarGrid, PolarRadiusAxis, Radar, RadarChart, ReferenceLine, Scatter, ScatterChart, SunburstChart, Treemap, XAxis, YAxis, ZAxis } from 'recharts'
import type { ComponentProps } from 'react'
import type { AxisOptions, Element, Theme } from './types'
import { cssColor, fontFamily } from './design'
import { axisDomain, axisTicks, chartWarnings, dataLabel, numberFormat, waterfallData } from './chart-render'
import './document-render.css'
import { CoreElementPreview } from './RichTextSurface'

type ChartElement = Extract<Element, { type: 'chart' }>

function TypedChartSurface({ element, theme }: { element: ChartElement; theme?: Theme }) {
  const histogram = element.options?.histogram
  const box = element.options?.box_whisker
  const hierarchy = element.options?.hierarchy
  const shade = cssColor(element.series[0].color, theme)
  const ink = cssColor('@dk1', theme)
  const warning = 'Approximate Studio preview; not Office visual parity. Raw data is preserved.'
  let rendering: React.ReactNode
  if (histogram) {
    const low = histogram.underflow ?? Math.min(...histogram.samples)
    const high = histogram.overflow ?? Math.max(...histogram.samples)
    const count = histogram.binning.rule === 'count' ? histogram.binning.count : Math.max(1, Math.ceil((high - low) / histogram.binning.width))
    const width = histogram.binning.rule === 'width' ? histogram.binning.width : (high - low) / count || 1
    const bins = Array.from({ length: Math.max(1, Math.min(128, count)) }, (_, index) => ({ name: `${numberFormat(low + index * width)}..${numberFormat(low + (index + 1) * width)}`, count: 0 }))
    let underflow = 0
    let overflow = 0
    for (const sample of histogram.samples) {
      if (histogram.underflow != null && sample <= histogram.underflow) { underflow++; continue }
      if (histogram.overflow != null && sample > histogram.overflow) { overflow++; continue }
      const position = (sample - low) / width
      const index = histogram.interval_closed === 'left' ? Math.floor(position) : Math.ceil(position) - 1
      bins[Math.max(0, Math.min(bins.length - 1, index))].count++
    }
    if (histogram.underflow != null) bins.unshift({ name: `<=${histogram.underflow}`, count: underflow })
    if (histogram.overflow != null) bins.push({ name: `>${histogram.overflow}`, count: overflow })
    rendering = <ComposedChart width={element.width} height={element.height} data={bins} margin={{ top: 16, right: 16, bottom: 12, left: 0 }} accessibilityLayer={false}>
      <XAxis dataKey="name" tick={{ fontSize: 10 }} /><YAxis allowDecimals={false} width={32} tick={{ fontSize: 10 }} />
      <Bar dataKey="count" name={element.series[0].name} fill={shade} isAnimationActive={false} />
    </ComposedChart>
  } else if (box) {
    const domain = axisDomain(undefined, box.samples.flat(), false)
    const scale = (value: number) => 20 + (domain[1] - value) / (domain[1] - domain[0]) * (element.height - 60)
    const statistics = box.samples.map((samples) => {
      const sorted = [...samples].sort((left, right) => left - right)
      const quantile = (fraction: number) => {
        const position = box.quartile_method === 'inclusive' ? (sorted.length - 1) * fraction : (sorted.length + 1) * fraction - 1
        const lower = Math.max(0, Math.min(sorted.length - 1, Math.floor(position)))
        const upper = Math.min(sorted.length - 1, lower + 1)
        return sorted[lower] + (sorted[upper] - sorted[lower]) * Math.max(0, position - lower)
      }
      const first = quantile(0.25)
      const third = quantile(0.75)
      const fence = (third - first) * 1.5
      const inside = sorted.filter((value) => value >= first - fence && value <= third + fence)
      return { first, third, median: quantile(0.5), mean: samples.reduce((sum, value) => sum + value, 0) / samples.length, low: inside[0], high: inside.at(-1)!, inside, outside: sorted.filter((value) => value < first - fence || value > third + fence) }
    })
    const step = (element.width - 40) / statistics.length
    const center = (index: number) => 20 + step * (index + 0.5)
    rendering = <svg width={element.width} height={element.height} viewBox={`0 0 ${element.width} ${element.height}`}>
      <line x1={20} x2={element.width - 20} y1={element.height - 40} y2={element.height - 40} stroke={ink} />
      {box.mean_line && <polyline points={statistics.map((group, index) => `${center(index)},${scale(group.mean)}`).join(' ')} fill="none" stroke={shade} strokeDasharray="4 3" />}
      {statistics.map((group, index) => <g key={index} data-box-group={index}>
        <title>{`${element.categories[index]}: median ${group.median}; Q1 ${group.first}; Q3 ${group.third}`}</title>
        <line x1={center(index)} x2={center(index)} y1={scale(group.low)} y2={scale(group.high)} stroke={ink} />
        {[group.low, group.high].map((value, position) => <line key={position} x1={center(index) - step * 0.15} x2={center(index) + step * 0.15} y1={scale(value)} y2={scale(value)} stroke={ink} />)}
        <rect x={center(index) - step * 0.25} y={scale(group.third)} width={step * 0.5} height={Math.max(1, scale(group.first) - scale(group.third))} fill={shade} fillOpacity={0.45} stroke={ink} />
        <line x1={center(index) - step * 0.25} x2={center(index) + step * 0.25} y1={scale(group.median)} y2={scale(group.median)} stroke={ink} strokeWidth={2} />
        {box.mean_marker && <path d={`M${center(index) - 4},${scale(group.mean) - 4}l8,8m0,-8l-8,8`} stroke={ink} />}
        {[...(box.nonoutliers ? group.inside : []), ...(box.outliers ? group.outside : [])].map((value, position) => <circle key={position} cx={center(index)} cy={scale(value)} r={2} fill={ink} />)}
        <text x={center(index)} y={element.height - 20} textAnchor="middle" fill={ink} fontSize={10}>{element.categories[index].length > Math.floor(step / 6) ? `${element.categories[index].slice(0, Math.max(1, Math.floor(step / 6) - 2))}..` : element.categories[index]}</text>
      </g>)}
    </svg>
  } else if (hierarchy) {
    type Node = { name: string; value: number; fill: string; children?: Node[] }
    const root: Node = { name: element.series[0].name, value: 0, fill: shade, children: [] }
    hierarchy.paths.forEach((path, index) => {
      let parent = root
      const value = element.series[0].values[index]
      parent.value += value
      path.forEach((name, depth) => {
        parent.children ??= []
        let node = parent.children.find((child) => child.name === name)
        if (!node) { node = { name, value: 0, fill: depth === 0 ? cssColor(`@accent${parent.children.length % 6 + 1}`, theme) : parent.fill }; parent.children.push(node) }
        node.value += value
        parent = node
      })
    })
    rendering = element.kind === 'treemap' ? <Treemap width={element.width} height={element.height} data={root.children} dataKey="value" nameKey="name" stroke={cssColor('@lt1', theme)} isAnimationActive={false} nodeInset={4} nodeGap={2} /> : <SunburstChart width={element.width} height={element.height} data={root} innerRadius={16} outerRadius={Math.min(element.width, element.height) / 2 - 10} textOptions={{ fontSize: 10, fill: ink }} />
  }
  return <div className="document-chart" data-chart-layout={element.kind} role="img" aria-label={`${element.kind}: ${element.series[0].name}`} title={warning} data-preview-warnings={warning} style={{ width: element.width, height: element.height, background: cssColor('@lt1', theme), fontFamily: fontFamily(null, theme), overflow: 'hidden' }}>{rendering}</div>
}

function ExtendedChartSurface({ element, theme }: { element: ChartElement; theme?: Theme }) {
  const compact = element.width < 360
  const warnings = chartWarnings(element)
  const data = waterfallData(element)
  const domain = axisDomain(undefined, data.flatMap((entry) => entry.range))
  const dimensions = { width: element.width, height: element.height, margin: { top: 24, right: 20, bottom: 16, left: 0 }, accessibilityLayer: false }
  const shade = cssColor(element.series[0].color, theme)
  const tick = { fontSize: compact ? 10 : 14, fill: cssColor('@dk2', theme) }
  const legendPosition = element.options?.legend ?? 'bottom'
  const sideLegend = legendPosition === 'left' || legendPosition === 'right'
  const legend = legendPosition !== 'hidden' && <Legend iconType="square" layout={sideLegend ? 'vertical' : 'horizontal'} align={legendPosition === 'left' ? 'left' : legendPosition === 'right' || legendPosition === 'top_right' ? 'right' : 'center'} verticalAlign={sideLegend ? 'middle' : legendPosition === 'top' || legendPosition === 'top_right' ? 'top' : 'bottom'} width={sideLegend ? element.width * 0.25 : undefined} wrapperStyle={{ fontSize: compact ? 10 : 14, overflowWrap: 'anywhere', maxWidth: '100%' }} />
  const rendering = element.kind === 'funnel' ? <FunnelChart {...dimensions}>
    <Funnel data={element.categories.map((category, index) => ({ name: category, value: element.series[0].values[index], fill: shade }))} dataKey="value" nameKey="name" isAnimationActive={false} />
    {legend}
  </FunnelChart> : <ComposedChart {...dimensions} data={data}>
    <CartesianGrid vertical={false} stroke={cssColor('@lt2', theme)} />
    <XAxis dataKey="category" tick={tick} axisLine={false} tickLine={false} />
    <YAxis type="number" domain={domain} tick={tick} width={compact ? 38 : 70} axisLine={false} tickLine={false} />
    <ReferenceLine y={0} stroke={cssColor('@dk2', theme)} />
    <Bar dataKey="range" name={element.series[0].name} fill={shade} isAnimationActive={false} />
    {legend}
  </ComposedChart>
  return <div className="document-chart" data-chart-layout={element.kind} role="img" aria-label={`${element.kind} chart: ${element.categories.map((category, index) => `${category} ${element.series[0].values[index]}${data[index]?.total ? ' (total)' : ''}`).join(', ')}`} title={warnings.join('; ')} data-preview-warnings={warnings.join('; ')} style={{ width: element.width, height: element.height, background: cssColor('@lt1', theme), fontFamily: fontFamily(null, theme) }}>{rendering}</div>
}

export function ChartSurface({ element, theme }: { element: ChartElement; theme?: Theme }) {
  const advanced = element.series.some((series) => series.trendline || series.error_bars)
    || [element.options?.primary_axis, element.options?.secondary_axis, element.options?.category_axis].some((axis) => axis && Object.values(axis).some((value) => value != null && value !== false))
    || element.options?.data_labels?.number_format
  if (advanced && !['pie', 'pie3d', 'doughnut', 'radar', 'radar_filled', 'funnel', 'waterfall', 'histogram', 'box_whisker', 'treemap', 'sunburst'].includes(element.kind)) return <CoreElementPreview element={{ ...element, x: 0, y: 0 }} theme={theme} label={`${element.kind} chart: ${element.series.map((series) => series.name).join(', ')}`} />
  if (['histogram', 'box_whisker', 'treemap', 'sunburst'].includes(element.kind)) return <TypedChartSurface element={element} theme={theme} />
  if (element.kind === 'funnel' || element.kind === 'waterfall') return <ExtendedChartSurface element={element} theme={theme} />
  const options = element.options
  const percent = element.kind === 'percent_stacked_column' || element.kind === 'percent_stacked_bar'
  const horizontal = ['bar', 'bar3d', 'stacked_bar', 'percent_stacked_bar'].includes(element.kind)
  const stacked = percent || element.kind === 'stacked_bar' || element.kind === 'stacked_column'
  const xy = element.kind === 'scatter' || element.kind === 'bubble'
  const polar = ['pie', 'pie3d', 'doughnut'].includes(element.kind)
  const radar = element.kind === 'radar' || element.kind === 'radar_filled'
  const secondary = element.series.some((series) => series.axis === 'secondary')
  const compact = element.width < 360
  const tick = { fontSize: compact ? 10 : 14, fill: cssColor('@dk2', theme) }
  const warnings = chartWarnings(element)
  const data = element.categories.map((category, index) => {
    const total = element.series.reduce((sum, series) => sum + series.values[index], 0)
    return Object.fromEntries([['category', category], ...element.series.flatMap((series, seriesIndex) => [
      [`series${seriesIndex}`, percent ? total > 0 ? series.values[index] / total : 0 : series.values[index]],
      [`label${seriesIndex}`, dataLabel(element, seriesIndex, index, options?.data_labels)],
    ])])
  })
  const valuesFor = (axis: 'primary' | 'secondary') => {
    const series = element.series.filter((series) => (series.axis ?? 'primary') === axis)
    if (percent) return [0, 1]
    if (stacked) return element.categories.flatMap((_, index) => [series.reduce((sum, entry) => sum + Math.min(0, entry.values[index]), 0), series.reduce((sum, entry) => sum + Math.max(0, entry.values[index]), 0)])
    return series.flatMap((entry) => entry.values)
  }
  const primaryDomain = axisDomain(options?.primary_axis, valuesFor('primary'), !xy && element.kind !== 'line', percent)
  const secondaryDomain = axisDomain(options?.secondary_axis, valuesFor('secondary'), false)
  const categoryValues = element.categories.map(Number)
  const categoryDomain = axisDomain(options?.category_axis, categoryValues, false)
  const axisProps = (axis: AxisOptions | undefined, domain: [number, number], percentAxis = false) => ({ domain, allowDataOverflow: axis?.min != null || axis?.max != null, reversed: axis?.reverse ?? false, scale: axis?.log_base ? 'log' as const : 'linear' as const, ticks: axisTicks(axis, domain), tickFormatter: (value: unknown) => numberFormat(value, axis?.number_format ?? (percentAxis ? '0%' : undefined)), tick, axisLine: false, tickLine: false })
  const categoryFormatter = (value: unknown) => options?.category_axis?.number_format ? numberFormat(value, options.category_axis.number_format) : String(value)
  const position = options?.data_labels?.position
  const labelPosition: ComponentProps<typeof LabelList>['position'] = position === 'center' ? 'center' : position === 'inside_end' ? horizontal ? 'insideRight' : 'insideTop' : position === 'outside_end' ? horizontal ? 'right' : 'top' : 'top'
  const labels = (index: number, radial = false) => options?.data_labels && <LabelList dataKey={`label${index}`} position={radial ? position === 'outside_end' ? 'outside' : 'center' : labelPosition} fontSize={compact ? 10 : 14} fill={cssColor('@dk1', theme)} />
  const legendPosition = options?.legend ?? 'bottom'
  const sideLegend = legendPosition === 'left' || legendPosition === 'right'
  const legend = legendPosition !== 'hidden' && <Legend iconType="square" iconSize={compact ? 8 : 12} layout={sideLegend ? 'vertical' : 'horizontal'} align={legendPosition === 'left' ? 'left' : legendPosition === 'right' || legendPosition === 'top_right' ? 'right' : 'center'} verticalAlign={sideLegend ? 'middle' : legendPosition === 'top' || legendPosition === 'top_right' ? 'top' : 'bottom'} width={sideLegend ? Math.min(element.width * 0.25, 140) : undefined} wrapperStyle={{ fontSize: compact ? 10 : 14, maxWidth: '100%', overflowWrap: 'anywhere', lineHeight: 1.2 }} />
  const axes = <>
    <CartesianGrid stroke={cssColor('@lt2', theme)} vertical={horizontal || xy} horizontal={!horizontal || xy} />
    {horizontal ? <>
      <XAxis type="number" xAxisId="primary" {...axisProps(options?.primary_axis, primaryDomain, percent)} />
      <YAxis type="category" dataKey="category" reversed={!(options?.category_axis?.reverse ?? false)} tickFormatter={categoryFormatter} width={compact ? 32 : 100} tick={tick} axisLine={false} tickLine={false} />
    </> : <>
      <XAxis type={xy ? 'number' : 'category'} dataKey={xy ? 'x' : 'category'} {...(xy ? axisProps(options?.category_axis, categoryDomain) : { reversed: options?.category_axis?.reverse ?? false, tickFormatter: categoryFormatter, tick, axisLine: false, tickLine: false })} />
      <YAxis type="number" yAxisId="primary" dataKey={xy ? 'y' : undefined} width={compact ? 38 : 70} {...axisProps(options?.primary_axis, primaryDomain, percent)} />
    </>}
    {secondary && (horizontal ? <XAxis type="number" xAxisId="secondary" orientation="top" {...axisProps(options?.secondary_axis, secondaryDomain)} /> : <YAxis type="number" yAxisId="secondary" orientation="right" width={compact ? 38 : 70} {...axisProps(options?.secondary_axis, secondaryDomain)} />)}
    {!options?.primary_axis?.log_base && primaryDomain[0] <= 0 && primaryDomain[1] >= 0 && <ReferenceLine {...(horizontal ? { x: 0, xAxisId: 'primary' } : { y: 0, yAxisId: 'primary' })} stroke={cssColor('@dk2', theme)} />}
    {(['primary', 'secondary'] as const).flatMap((axis) => {
      if (axis === 'secondary' && !secondary) return []
      const configuration = axis === 'primary' ? options?.primary_axis : options?.secondary_axis
      const domain = axis === 'primary' ? primaryDomain : secondaryDomain
      return (axisTicks(configuration, domain, true) ?? []).map((value) => <ReferenceLine key={`${axis}-${value}`} {...(horizontal ? { x: value, xAxisId: axis } : { y: value, yAxisId: axis })} stroke={cssColor('@lt2', theme)} strokeDasharray="2 3" />)
    })}
    {xy && (axisTicks(options?.category_axis, categoryDomain, true) ?? []).map((value) => <ReferenceLine key={`category-${value}`} x={value} yAxisId="primary" stroke={cssColor('@lt2', theme)} strokeDasharray="2 3" />)}
    {legend}
  </>
  const dimensions = { width: element.width, height: element.height, margin: { top: compact ? 12 : 24, right: compact ? 12 : 28, bottom: 8, left: 0 }, accessibilityLayer: false }
  const rendering = polar ? <PieChart {...dimensions}>
    <Pie data={element.categories.map((name, index) => ({ name, value: element.series[0].values[index], label0: dataLabel(element, 0, index, options?.data_labels) }))} dataKey="value" nameKey="name" innerRadius={element.kind === 'doughnut' ? '40%' : 0} outerRadius={position === 'outside_end' ? '65%' : '80%'} startAngle={90} endAngle={-270} isAnimationActive={false}>
      {element.categories.map((category, index) => <Cell key={`${index}-${category}`} fill={cssColor(`@accent${index % 6 + 1}`, theme)} />)}
      {labels(0, true)}
    </Pie>{legend}
  </PieChart> : radar ? <RadarChart {...dimensions} data={options?.category_axis?.reverse ? [data[0], ...data.slice(1).reverse()] : data} outerRadius="65%" startAngle={90} endAngle={-270}>
    <PolarGrid stroke={cssColor('@lt2', theme)} />
    <PolarAngleAxis dataKey="category" tick={tick} tickFormatter={categoryFormatter} />
    <PolarRadiusAxis {...axisProps(options?.primary_axis, primaryDomain)} />
    {element.series.map((series, index) => <Radar key={index} dataKey={`series${index}`} name={series.name} stroke={cssColor(series.color, theme)} fill={cssColor(series.color, theme)} fillOpacity={element.kind === 'radar_filled' ? 0.35 : 0} isAnimationActive={false}>{labels(index)}</Radar>)}
    {legend}
  </RadarChart> : xy ? <ScatterChart {...dimensions}>
    {axes}
    {element.kind === 'bubble' && <ZAxis dataKey="z" domain={[0, Math.max(1, ...element.series.flatMap((series) => series.bubble_sizes ?? []))]} range={[0, compact ? 300 : 1400]} />}
    {element.series.map((series, index) => <Scatter key={index} yAxisId={series.axis ?? 'primary'} name={series.name} data={series.values.map((value, position) => ({ x: Number(element.categories[position]), y: value, z: series.bubble_sizes?.[position] ?? 1, [`label${index}`]: dataLabel(element, index, position, options?.data_labels) }))} fill={cssColor(series.color, theme)} isAnimationActive={false}>{labels(index)}</Scatter>)}
  </ScatterChart> : <ComposedChart {...dimensions} data={data} layout={horizontal ? 'vertical' : 'horizontal'}>
    {axes}
    {element.series.map((series, index) => {
      const kind = element.kind === 'combo' ? series.kind ?? 'column' : element.kind
      const axis = horizontal ? { xAxisId: series.axis ?? 'primary' } : { yAxisId: series.axis ?? 'primary' }
      const common = { ...axis, dataKey: `series${index}`, name: series.name, isAnimationActive: false }
      if (kind === 'line') return <Line key={index} {...common} type="linear" stroke={cssColor(series.color, theme)} strokeWidth={3} dot={{ r: compact ? 2 : 4 }}>{labels(index)}</Line>
      if (kind === 'area') return <Area key={index} {...common} type="linear" fill={cssColor(series.color, theme)} stroke={cssColor(series.color, theme)} fillOpacity={0.65}>{labels(index)}</Area>
      return <Bar key={index} {...common} fill={cssColor(series.color, theme)} stackId={stacked ? 'stack' : undefined}>{labels(index)}</Bar>
    })}
  </ComposedChart>
  return <div className="document-chart" role="img" aria-label={`${element.kind} chart: ${element.series.map((series) => series.name).join(', ')}`} title={warnings.join('; ') || undefined} data-preview-warnings={warnings.join('; ') || undefined} style={{ width: element.width, height: element.height, background: cssColor('@lt1', theme), fontFamily: fontFamily(null, theme) }}>{rendering}</div>
}