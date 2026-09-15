import { Area, AreaChart, Bar, BarChart, CartesianGrid, Cell, Legend, Line, LineChart, Pie, PieChart, ReferenceLine, Scatter, ScatterChart, XAxis, YAxis } from 'recharts'
import type { Element, Theme } from './types'
import { cssColor } from './design'

export function ChartSurface({ element, theme }: { element: Extract<Element, { type: 'chart' }>; theme?: Theme }) {
  const percent = element.kind === 'percent_stacked_column'
  const data = element.categories.map((category, index) => Object.fromEntries([
    ['category', category], ...element.series.map((series, seriesIndex) => [`series${seriesIndex}`, percent ? series.values[index] / element.series.reduce((total, entry) => total + entry.values[index], 0) * 100 : series.values[index]]),
  ]))
  const horizontal = element.kind === 'bar' || element.kind === 'stacked_bar'
  const stacked = element.kind === 'stacked_bar' || element.kind === 'stacked_column' || percent
  const numericDomain: [number, number] | ['auto', 'auto'] | [(minimum: number) => number, (maximum: number) => number] = percent ? [0, 100] : element.kind === 'line'
    ? ['auto', 'auto'] : [(minimum) => Math.min(0, minimum), (maximum) => Math.max(0, maximum)]
  const axes = <>
    <CartesianGrid stroke="#DDE4E1" vertical={horizontal} horizontal={!horizontal} />
    <XAxis type={horizontal ? 'number' : 'category'} dataKey={horizontal ? undefined : 'category'} domain={horizontal ? numericDomain : undefined} tick={{ fontSize: 14, fill: '#586563' }} axisLine={false} tickLine={false} />
    <YAxis type={horizontal ? 'category' : 'number'} dataKey={horizontal ? 'category' : undefined} domain={horizontal ? undefined : numericDomain} tickFormatter={percent ? (value: number) => `${value}%` : undefined} width={horizontal ? 120 : 70} tick={{ fontSize: 14, fill: '#586563' }} axisLine={false} tickLine={false} />
    <ReferenceLine {...(horizontal ? { x: 0 } : { y: 0 })} stroke="#586563" />
    <Legend iconType="square" wrapperStyle={{ fontSize: 14 }} />
  </>
  return <div role="img" aria-label={`${element.kind} chart: ${element.series.map((series) => series.name).join(', ')}`} style={{ width: element.width, height: element.height, background: cssColor('@lt1', theme) }}>
    {element.kind === 'pie' || element.kind === 'doughnut' ? <PieChart width={element.width} height={element.height} accessibilityLayer={false}>
      <Pie data={element.categories.map((name, index) => ({ name, value: element.series[0].values[index] }))} dataKey="value" nameKey="name" innerRadius={element.kind === 'doughnut' ? '40%' : 0} outerRadius="80%" startAngle={90} endAngle={-270} isAnimationActive={false}>
        {element.categories.map((category, index) => <Cell key={`${index}-${category}`} fill={cssColor(`@accent${index % 6 + 1}`, theme)} />)}
      </Pie><Legend iconType="square" wrapperStyle={{ fontSize: 14 }} />
    </PieChart> : element.kind === 'scatter' ? <ScatterChart width={element.width} height={element.height} margin={{ top: 16, right: 24, bottom: 8, left: 0 }} accessibilityLayer={false}>
      <CartesianGrid stroke="#DDE4E1" /><XAxis type="number" dataKey="x" domain={['auto', 'auto']} /><YAxis type="number" dataKey="y" domain={['auto', 'auto']} /><Legend />
      {element.series.map((series, index) => <Scatter key={index} name={series.name} data={series.values.map((value, position) => ({ x: Number(element.categories[position]), y: value }))} fill={cssColor(series.color, theme)} isAnimationActive={false} />)}
    </ScatterChart> : element.kind === 'area' ? <AreaChart width={element.width} height={element.height} data={data} margin={{ top: 16, right: 24, bottom: 8, left: 0 }} accessibilityLayer={false}>
      {axes}{element.series.map((series, index) => <Area key={index} type="linear" dataKey={`series${index}`} name={series.name} fill={cssColor(series.color, theme)} stroke={cssColor(series.color, theme)} fillOpacity={0.65} isAnimationActive={false} />)}
    </AreaChart> :
    element.kind === 'line' ? <LineChart width={element.width} height={element.height} data={data} margin={{ top: 16, right: 24, bottom: 8, left: 0 }} accessibilityLayer={false}>
      {axes}{element.series.map((series, index) => <Line key={index} type="linear" dataKey={`series${index}`} name={series.name} stroke={cssColor(series.color, theme)} strokeWidth={3} dot={{ r: 4 }} isAnimationActive={false} />)}
    </LineChart> : <BarChart width={element.width} height={element.height} data={data} layout={horizontal ? 'vertical' : 'horizontal'} margin={{ top: 16, right: 24, bottom: 8, left: 0 }} accessibilityLayer={false}>
      {axes}{element.series.map((series, index) => <Bar key={index} dataKey={`series${index}`} name={series.name} fill={cssColor(series.color, theme)} stackId={stacked ? 'stack' : undefined} isAnimationActive={false} />)}
    </BarChart>}
  </div>
}