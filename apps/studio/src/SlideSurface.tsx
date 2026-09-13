import { lazy, Suspense, useEffect, useId, useRef, useState } from 'react'
import type { Element, Slide } from './types'

const ChartSurface = lazy(() => import('./ChartSurface').then((module) => ({ default: module.ChartSurface })))

type Props = { slide: Slide; selected?: string | null; onSelect?: (id: string) => void; onMove?: (id: string, x: number, y: number) => void }

function Content({ element }: { element: Element }) {
  const arrowId = useId()
  if (element.type === 'group') return <div style={{ position: 'absolute', width: element.view_width, height: element.view_height, transformOrigin: 'top left', transform: `scale(${element.width / element.view_width}, ${element.height / element.view_height})` }}>
    {element.children.map((child) => <div key={child.id} style={{ position: 'absolute', left: child.x, top: child.y, width: child.width, height: child.height }}><Content element={child} /></div>)}
  </div>
  if (element.type === 'picture') {
    const crop = element.crop ?? { left: 0, top: 0, right: 0, bottom: 0 }
    const visibleWidth = 1 - crop.left - crop.right
    const visibleHeight = 1 - crop.top - crop.bottom
    return <div style={{ width: '100%', height: '100%', overflow: 'hidden', position: 'relative' }}><img src={`data:${element.mime_type};base64,${element.base64}`} alt={element.alt} draggable={false} style={{ position: 'absolute', maxWidth: 'none', width: `${100 / visibleWidth}%`, height: `${100 / visibleHeight}%`, left: `${-100 * crop.left / visibleWidth}%`, top: `${-100 * crop.top / visibleHeight}%` }} /></div>
  }
  if (element.type === 'connector') return <svg className="connector-line" width={element.width} height={element.height} style={{ overflow: 'visible' }} aria-hidden="true">
    <defs><marker id={arrowId} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill={`#${element.color}`} /></marker></defs>
    <line x1="0" y1={element.flip_v ? element.height : 0} x2={element.width} y2={element.flip_v ? 0 : element.height} stroke={`#${element.color}`} strokeWidth={element.stroke_width} markerEnd={element.arrow ? `url(#${arrowId})` : undefined} />
  </svg>
  if (element.type === 'chart') return <Suspense fallback={<div role="status" aria-label="Chart loading" style={{ width: element.width, height: element.height }} />}><ChartSurface element={element} /></Suspense>
  if (element.type === 'rect') return <div style={{ width: '100%', height: '100%', background: `#${element.fill}` }} />
  if (element.type === 'table') return <table className="slide-table" style={{ fontSize: element.font_size }}><tbody>
    {element.rows.map((row, rowIndex) => <tr key={rowIndex} style={{ height: `${100 / element.rows.length}%` }}>{row.map((cell, cellIndex) => rowIndex === 0 ? <th key={cellIndex}>{cell}</th> : <td key={cellIndex}>{cell}</td>)}</tr>)}
  </tbody></table>
  return <div className="slide-text" style={{ fontSize: element.font_size, color: `#${element.color}`, fontWeight: element.bold ? 700 : 400 }}>{element.text}</div>
}

export function SlideSurface({ slide, selected, onSelect, onMove }: Props) {
  const host = useRef<HTMLDivElement>(null)
  const [scale, setScale] = useState(0.5)
  const [drag, setDrag] = useState<{ id: string; startX: number; startY: number; x: number; y: number; nextX: number; nextY: number } | null>(null)
  useEffect(() => {
    const node = host.current
    if (!node) return
    const observer = new ResizeObserver(([entry]) => setScale(entry.contentRect.width / 1280))
    observer.observe(node)
    return () => observer.disconnect()
  }, [])
  return <div className="slide-surface" ref={host}>
    <div className="slide-page" style={{ transform: `scale(${scale})`, background: `#${slide.background}` }}>
      {slide.elements.map((element) => <div key={element.id} className={`slide-element ${onSelect && selected === element.id ? 'selected' : ''}`} style={{ left: drag?.id === element.id ? drag.nextX : element.x, top: drag?.id === element.id ? drag.nextY : element.y, width: element.width, height: element.height }}>
        <Content element={element} />
        {onSelect && <button type="button" className="element-hitbox" aria-label={`Edit ${element.id}`} title={element.id}
          onClick={() => onSelect(element.id)}
          onPointerDown={(event) => {
            if (event.button !== 0) return
            onSelect(element.id)
            event.currentTarget.setPointerCapture(event.pointerId)
            setDrag({ id: element.id, startX: event.clientX, startY: event.clientY, x: element.x, y: element.y, nextX: element.x, nextY: element.y })
          }}
          onPointerMove={(event) => {
            if (drag?.id !== element.id) return
            setDrag({ ...drag, nextX: Math.max(0, Math.min(1280 - element.width, Math.round(drag.x + (event.clientX - drag.startX) / scale))), nextY: Math.max(0, Math.min(720 - element.height, Math.round(drag.y + (event.clientY - drag.startY) / scale))) })
          }}
          onPointerUp={() => {
            if (drag?.id === element.id && (drag.nextX !== drag.x || drag.nextY !== drag.y)) onMove?.(element.id, drag.nextX, drag.nextY)
            setDrag(null)
          }}
          onPointerCancel={() => setDrag(null)}
          onKeyDown={(event) => {
            const increment = event.shiftKey ? 10 : 1
            const delta = { ArrowLeft: [-increment, 0], ArrowRight: [increment, 0], ArrowUp: [0, -increment], ArrowDown: [0, increment] }[event.key]
            if (!delta) return
            event.preventDefault()
            onMove?.(element.id, Math.max(0, Math.min(1280 - element.width, element.x + delta[0])), Math.max(0, Math.min(720 - element.height, element.y + delta[1])))
          }} />}
      </div>)}
    </div>
  </div>
}