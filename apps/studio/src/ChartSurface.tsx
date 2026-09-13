import { Bar, BarChart, CartesianGrid, Legend, Line, LineChart, ReferenceLine, XAxis, YAxis } from 'recharts'
import type { Element } from './types'

export function ChartSurface({ element }: { element: Extract<Element, { type: 'chart' }> }) {
  const data = element.categories.map((category, index) => Object.fromEntries([
    ['category', category], ...element.series.map((series, seriesIndex) => [`series${seriesIndex}`, series.values[index]]),
  ]))
  const horizontal = element.kind === 'bar'
  const numericDomain: ['auto', 'auto'] | [(minimum: number) => number, (maximum: number) => number] = element.kind === 'line'
    ? ['auto', 'auto'] : [(minimum) => Math.min(0, minimum), (maximum) => Math.max(0, maximum)]
  const axes = <>
    <CartesianGrid stroke="#DDE4E1" vertical={horizontal} horizontal={!horizontal} />
    <XAxis type={horizontal ? 'number' : 'category'} dataKey={horizontal ? undefined : 'category'} domain={horizontal ? numericDomain : undefined} tick={{ fontSize: 14, fill: '#586563' }} axisLine={false} tickLine={false} />
    <YAxis type={horizontal ? 'category' : 'number'} dataKey={horizontal ? 'category' : undefined} domain={horizontal ? undefined : numericDomain} width={horizontal ? 120 : 70} tick={{ fontSize: 14, fill: '#586563' }} axisLine={false} tickLine={false} />
    <ReferenceLine {...(horizontal ? { x: 0 } : { y: 0 })} stroke="#586563" />
    <Legend iconType="square" wrapperStyle={{ fontSize: 14 }} />
  </>
  return <div role="img" aria-label={`${element.kind} chart: ${element.series.map((series) => series.name).join(', ')}`} style={{ width: element.width, height: element.height, background: '#FFFFFF' }}>
    {element.kind === 'line' ? <LineChart width={element.width} height={element.height} data={data} margin={{ top: 16, right: 24, bottom: 8, left: 0 }} accessibilityLayer={false}>
      {axes}{element.series.map((series, index) => <Line key={index} type="linear" dataKey={`series${index}`} name={series.name} stroke={`#${series.color}`} strokeWidth={3} dot={{ r: 4 }} isAnimationActive={false} />)}
    </LineChart> : <BarChart width={element.width} height={element.height} data={data} layout={horizontal ? 'vertical' : 'horizontal'} margin={{ top: 16, right: 24, bottom: 8, left: 0 }} accessibilityLayer={false}>
      {axes}{element.series.map((series, index) => <Bar key={index} dataKey={`series${index}`} name={series.name} fill={`#${series.color}`} isAnimationActive={false} />)}
    </BarChart>}
  </div>
}