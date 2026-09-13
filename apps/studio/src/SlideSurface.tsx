import { useEffect, useRef, useState } from 'react'
import type { Element, Slide } from './types'

type Props = { slide: Slide; selected?: string | null; onSelect?: (id: string) => void; onMove?: (id: string, x: number, y: number) => void }

function Content({ element }: { element: Element }) {
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