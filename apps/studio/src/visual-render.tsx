import type { CSSProperties } from 'react'
import type { Element, Gradient, Theme, VectorPath, VisualStyle } from './types'
import { cssColor } from './design'

export function visualOf(element: Element): VisualStyle | undefined {
  return 'visual' in element ? element.visual ?? undefined : undefined
}

export function elementTransform(element: Element): string | undefined {
  const visual = visualOf(element)
  const rotation = element.type === 'shape' ? element.rotation : visual?.rotation ?? 0
  return [rotation ? `rotate(${rotation}deg)` : '', visual?.flip_h || visual?.flip_v ? `scale(${visual.flip_h ? -1 : 1}, ${visual.flip_v ? -1 : 1})` : ''].filter(Boolean).join(' ') || undefined
}

export function visualWarnings(element: Element): string[] {
  const visual = visualOf(element)
  const warnings: string[] = []
  if (visual?.gradient) warnings.push('Gradient geometry is a browser approximation')
  if (visual?.shadow || visual?.glow || visual?.soft_edge || visual?.reflection) warnings.push('Effects are browser approximations')
  if (visual?.reflection?.blur) warnings.push('Reflection-only blur is preserved, not previewed')
  if (visual?.text_warp) warnings.push('WordArt uses core-shaped glyph outlines with default-adjustment approximation; not Office parity')
  if (visual?.adjustments?.length) warnings.push('Preset adjustments use a bounded preview mapping')
  if ('format' in element && element.type !== 'table' && element.format?.paragraphs?.some((paragraph) => paragraph.tabs?.some((tab) => tab.alignment !== 'left'))) warnings.push('Centered, right and decimal tabs use a grid approximation')
  if ('format' in element && element.type !== 'table' && element.format?.hyperlink) warnings.push('Hyperlink retained; canvas text does not navigate')
  return warnings
}

function alpha(color: string, opacity: number, theme?: Theme) {
  return `${cssColor(color, theme)}${Math.round(Math.max(0, Math.min(1, opacity)) * 255).toString(16).padStart(2, '0')}`
}

export function effectStyle(visual?: VisualStyle, theme?: Theme): CSSProperties {
  const filters: string[] = []
  if (visual?.soft_edge) filters.push(`blur(${visual.soft_edge / 2}px)`)
  if (visual?.glow) filters.push(`drop-shadow(0 0 ${visual.glow.radius / 2}px ${alpha(visual.glow.color, visual.glow.opacity, theme)})`)
  if (visual?.shadow) {
    const shadow = visual.shadow
    const radians = shadow.angle * Math.PI / 180
    filters.push(`drop-shadow(${shadow.distance * Math.cos(radians)}px ${shadow.distance * Math.sin(radians)}px ${shadow.blur / 2}px ${alpha(shadow.color, shadow.opacity, theme)})`)
  }
  const reflection = visual?.reflection
  return { filter: filters.length ? filters.join(' ') : undefined, ...(reflection ? { WebkitBoxReflect: `below ${reflection.distance}px linear-gradient(to bottom, transparent ${100 * (1 - reflection.end_position)}%, rgba(0,0,0,${reflection.end_opacity}), rgba(0,0,0,${reflection.start_opacity}))` } : {}) } as CSSProperties
}

export function gradientDefinition({ id, gradient, theme }: { id: string; gradient?: Gradient | null; theme?: Theme }) {
  if (!gradient) return null
  const stops = gradient.stops.map((stop, index) => <stop key={index} offset={stop.offset} stopColor={cssColor(stop.color, theme)} stopOpacity={stop.opacity} />)
  if (gradient.kind === 'radial') return <radialGradient id={id} cx={gradient.center[0]} cy={gradient.center[1]} r="0.7071">{stops}</radialGradient>
  const radians = gradient.angle * Math.PI / 180
  const horizontal = Math.cos(radians) / 2
  const vertical = Math.sin(radians) / 2
  return <linearGradient id={id} x1={0.5 - horizontal} y1={0.5 - vertical} x2={0.5 + horizontal} y2={0.5 + vertical}>{stops}</linearGradient>
}

export function pathData(path: VectorPath, width: number, height: number): string {
  const point = ([horizontal, vertical]: [number, number]) => `${horizontal * width} ${vertical * height}`
  return path.commands.map((command) => {
    switch (command.op) {
      case 'move': return `M${point(command.point)}`
      case 'line': return `L${point(command.point)}`
      case 'quadratic': return `Q${point(command.control)} ${point(command.point)}`
      case 'cubic': return `C${point(command.control1)} ${point(command.control2)} ${point(command.point)}`
      case 'close': return 'Z'
    }
  }).join(' ')
}

export function pictureMask(mask: VisualStyle['picture_mask']): string | undefined {
  return mask ? { ellipse: 'ellipse(50% 50% at 50% 50%)', round_rect: 'inset(0 round 16.667%)', diamond: 'polygon(50% 0,100% 50%,50% 100%,0 50%)', hexagon: 'polygon(25% 0,75% 0,100% 50%,75% 100%,25% 100%,0 50%)' }[mask] : undefined
}

export function svgImageSource(svg: string): string {
  const bytes = new TextEncoder().encode(svg)
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return `data:image/svg+xml;base64,${btoa(binary)}`
}