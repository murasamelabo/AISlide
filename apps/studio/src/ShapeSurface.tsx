import type { ReactNode } from 'react'
import type { ShapeAdjustment } from './types'

function polygon(sides: number, inner?: number): string {
  const count = inner ? sides * 2 : sides
  return Array.from({ length: count }, (_, index) => {
    const angle = -Math.PI / 2 + index * Math.PI * 2 / count
    const radius = inner && index % 2 ? 50 * inner : 50
    return `${index ? 'L' : 'M'}${50 + radius * Math.cos(angle)} ${50 + radius * Math.sin(angle)}`
  }).join(' ') + ' Z'
}

export function ShapeSurface({ preset, fill, stroke, strokeWidth = 2, fillOpacity, adjustments, children, width = 100, height = 100, nativeGeometry = false }: { preset: string; fill: string; stroke: string; strokeWidth?: number; fillOpacity?: number; adjustments?: ShapeAdjustment[]; children?: ReactNode; width?: number; height?: number; nativeGeometry?: boolean }) {
  const rectangle = 'M0 0H100V100H0Z'
  const ellipse = 'M0 50A50 50 0 1 0 100 50A50 50 0 1 0 0 50Z'
  const paths: Record<string, string> = {
    rect: rectangle, roundRect: 'M17 0H83Q100 0 100 17V83Q100 100 83 100H17Q0 100 0 83V17Q0 0 17 0Z',
    ellipse, triangle: 'M50 0L100 100H0Z', rtTriangle: 'M0 0L100 100H0Z', diamond: 'M50 0L100 50 50 100 0 50Z',
    can: 'M0 15A50 15 0 0 1 100 15V85A50 15 0 0 1 0 85Z',
    cloud: 'M20 78C1 78 0 48 18 42C9 20 36 11 46 25C54 5 86 12 84 35C101 31 108 63 92 72C99 92 72 102 61 86C45 101 17 97 20 78Z',
    parallelogram: 'M25 0H100L75 100H0Z', trapezoid: 'M25 0H75L100 100H0Z',
    pentagon: polygon(5), hexagon: polygon(6), heptagon: polygon(7), octagon: polygon(8), decagon: polygon(10),
    plus: 'M33 0H67V33H100V67H67V100H33V67H0V33H33Z',
    heart: 'M50 100C40 85 0 60 0 28C0 -5 36 -10 50 18C64 -10 100 -5 100 28C100 60 60 85 50 100Z',
    chevron: 'M0 0H65L100 50 65 100H0L35 50Z', rightArrow: 'M0 25H60V0L100 50 60 100V75H0Z',
    leftArrow: 'M100 25H40V0L0 50 40 100V75H100Z', upArrow: 'M25 100V40H0L50 0 100 40H75V100Z',
    downArrow: 'M25 0V60H0L50 100 100 60H75V0Z', leftRightArrow: 'M0 50L25 0V25H75V0L100 50 75 100V75H25V100Z',
    upDownArrow: 'M50 0L100 25H75V75H100L50 100 0 75H25V25H0Z', homePlate: 'M0 0H65L100 50 65 100H0Z',
    flowChartProcess: rectangle, flowChartDecision: 'M50 0L100 50 50 100 0 50Z',
    flowChartTerminator: 'M25 0H75A25 50 0 0 1 75 100H25A25 50 0 0 1 25 0Z',
    flowChartInputOutput: 'M20 0H100L80 100H0Z', flowChartDocument: 'M0 0H100V85C60 65 40 110 0 90Z',
    flowChartPredefinedProcess: rectangle, flowChartInternalStorage: rectangle, flowChartConnector: ellipse,
    flowChartOfflineStorage: 'M0 0H100L50 100Z',
    wedgeRectCallout: 'M0 0H100V80H40L10 100 23 80H0Z',
    wedgeRoundRectCallout: 'M15 0H85Q100 0 100 15V65Q100 80 85 80H40L10 100 23 80H15Q0 80 0 65V15Q0 0 15 0Z',
    wedgeEllipseCallout: 'M20 76C-20 52 0 0 50 0C115 0 120 76 50 80L10 100Z',
  }
  const star = /^star(\d+)$/.exec(preset)
  const adjustment = adjustments?.find((entry) => entry.name === 'adj')?.value
  if (adjustment != null) {
    if (preset === 'roundRect') {
      const radius = Math.max(0, Math.min(0.5, adjustment / 100000)) * Math.min(width, height)
      const horizontal = radius / width * 100
      const vertical = radius / height * 100
      paths.roundRect = `M${horizontal} 0H${100 - horizontal}Q100 0 100 ${vertical}V${100 - vertical}Q100 100 ${100 - horizontal} 100H${horizontal}Q0 100 0 ${100 - vertical}V${vertical}Q0 0 ${horizontal} 0Z`
    }
    if (preset === 'triangle') paths.triangle = `M${Math.max(0, Math.min(100, adjustment / 1000))} 0L100 100H0Z`
    if (preset === 'chevron') { const inset = Math.max(0, Math.min(100, adjustment / 100000 * Math.min(width, height) / width * 100)); paths.chevron = `M0 0H${100 - inset}L100 50 ${100 - inset} 100H0L${inset} 50Z` }
  }
  if (nativeGeometry && preset === 'roundRect') {
    const radius = Math.max(0, Math.min(50000, adjustment ?? 16667)) / 100000 * Math.min(width, height)
    const horizontal = radius / width * 100, vertical = radius / height * 100
    paths.roundRect = radius === 0 ? rectangle : `M${horizontal} 0H${100 - horizontal}A${horizontal} ${vertical} 0 0 1 100 ${vertical}V${100 - vertical}A${horizontal} ${vertical} 0 0 1 ${100 - horizontal} 100H${horizontal}A${horizontal} ${vertical} 0 0 1 0 ${100 - vertical}V${vertical}A${horizontal} ${vertical} 0 0 1 ${horizontal} 0Z`
  }
  const path = star ? polygon(Number(star[1]), Number(star[1]) === 5 ? 0.382 : 0.5) : paths[preset] ?? rectangle
  return <svg className="preset-shape" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
    {children}
    <path d={path} fill={fill} fillOpacity={fillOpacity} stroke={stroke} strokeWidth={strokeWidth} vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
    {preset === 'flowChartPredefinedProcess' && <path d="M13 0V100M87 0V100" stroke={stroke} strokeWidth={strokeWidth} vectorEffect="non-scaling-stroke" />}
    {preset === 'flowChartInternalStorage' && <path d="M15 0V100M0 15H100" stroke={stroke} strokeWidth={strokeWidth} vectorEffect="non-scaling-stroke" />}
    {preset === 'can' && <path d="M0 15A50 15 0 0 0 100 15" fill="none" stroke={stroke} strokeWidth={strokeWidth} vectorEffect="non-scaling-stroke" />}
  </svg>
}