import { lazy, memo, Suspense, useEffect, useId, useRef, useState } from 'react'
import type { PointerEvent as ReactPointerEvent } from 'react'
import type { Design, Element, Slide, Theme } from './types'
import { InlineEditor } from './InlineEditor'
import { ShapeSurface } from './ShapeSurface'
import { cssColor, fontFamily, textStyle } from './design'
import type { MenuPosition } from './ContextMenu'

const ChartSurface = lazy(() => import('./ChartSurface').then((module) => ({ default: module.ChartSurface })))

type Props = { slide: Slide; design?: Design | null; selected?: string | null; onSelect?: (id: string) => void; onMove?: (id: string, x: number, y: number) => void | Promise<void>; onResize?: (id: string, width: number, height: number) => void | Promise<void>; onEdit?: (element: Element) => Promise<void>; editRequest?: { id: string; slideId: string; sequence: number }; onContextMenu?: (position: MenuPosition, id?: string) => void; onDraftChange?: (dirty: boolean) => void }

export const Content = memo(function ElementContent({ element, theme }: { element: Element; theme?: Theme }) {
  const arrowId = useId()
  if (element.type === 'group') return <div style={{ position: 'absolute', width: element.view_width, height: element.view_height, transformOrigin: 'top left', transform: `scale(${element.width / element.view_width}, ${element.height / element.view_height})` }}>
    {element.children.map((child) => <div key={child.id} style={{ position: 'absolute', left: child.x, top: child.y, width: child.width, height: child.height, transform: child.type === 'shape' ? `rotate(${child.rotation}deg)` : undefined }}><Content element={child} theme={theme} /></div>)}
  </div>
  if (element.type === 'picture') {
    const crop = element.crop ?? { left: 0, top: 0, right: 0, bottom: 0 }
    const visibleWidth = 1 - crop.left - crop.right
    const visibleHeight = 1 - crop.top - crop.bottom
    return <div style={{ width: '100%', height: '100%', overflow: 'hidden', position: 'relative' }}><img src={`data:${element.mime_type};base64,${element.base64}`} alt={element.alt} draggable={false} style={{ position: 'absolute', maxWidth: 'none', width: `${100 / visibleWidth}%`, height: `${100 / visibleHeight}%`, left: `${-100 * crop.left / visibleWidth}%`, top: `${-100 * crop.top / visibleHeight}%` }} /></div>
  }
  if (element.type === 'connector') return <svg className="connector-line" width={Math.max(1, element.width)} height={Math.max(1, element.height)} style={{ overflow: 'visible' }} aria-hidden="true">
    <defs><marker id={arrowId} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill={cssColor(element.color, theme)} /></marker></defs>
    {element.routing ? <polyline points={element.routing.points.map(([horizontal, vertical]) => `${horizontal * element.width},${vertical * element.height}`).join(' ')} fill="none" stroke={cssColor(element.color, theme)} strokeWidth={element.stroke_width} strokeDasharray={element.routing.dashed ? '8 5' : undefined} markerStart={element.routing.start_arrow ? `url(#${arrowId})` : undefined} markerEnd={element.arrow ? `url(#${arrowId})` : undefined} /> : <line x1="0" y1={element.flip_v ? element.height : 0} x2={element.width} y2={element.flip_v ? 0 : element.height} stroke={cssColor(element.color, theme)} strokeWidth={element.stroke_width} markerEnd={element.arrow ? `url(#${arrowId})` : undefined} />}
  </svg>
  if (element.type === 'chart') return <Suspense fallback={<div role="status" aria-label="Chart loading" style={{ width: element.width, height: element.height }} />}><ChartSurface element={element} theme={theme} /></Suspense>
  if (element.type === 'polygon') return <svg width={element.width} height={element.height} viewBox={`0 0 ${element.width} ${element.height}`} aria-hidden="true"><polygon points={element.points.map(([horizontal, vertical]) => `${horizontal * element.width},${vertical * element.height}`).join(' ')} fill={element.fill === 'none' ? 'none' : cssColor(element.fill, theme)} stroke={cssColor(element.stroke, theme)} strokeWidth={element.stroke_width} /></svg>
  if (element.type === 'rect') return <div style={{ width: '100%', height: '100%', background: cssColor(element.fill, theme) }} />
  if (element.type === 'table') return <table className="slide-table" style={{ fontSize: element.font_size, fontFamily: fontFamily(null, theme) }}><tbody>
    {element.rows.map((row, rowIndex) => <tr key={rowIndex} style={{ height: `${100 / element.rows.length}%` }}>{row.map((cell, cellIndex) => rowIndex === 0 ? <th key={cellIndex} style={{ background: cssColor('@accent1', theme), color: cssColor('@lt1', theme) }}>{cell}</th> : <td key={cellIndex} style={{ background: cssColor(rowIndex % 2 === 0 ? '@lt2' : '@lt1', theme), color: cssColor('@dk1', theme) }}>{cell}</td>)}</tr>)}
  </tbody></table>
  const content = element.format?.bullet === 'bullet' ? <ul>{element.text.split('\n').map((line, index) => <li key={index}>{line}</li>)}</ul>
    : element.format?.bullet === 'numbered' ? <ol>{element.text.split('\n').map((line, index) => <li key={index}>{line}</li>)}</ol> : <div>{element.text}</div>
  return <>
    {element.type === 'shape' && <ShapeSurface preset={element.preset} fill={element.fill === 'none' ? 'none' : cssColor(element.fill, theme)} stroke={cssColor(element.stroke, theme)} strokeWidth={element.stroke_width} />}
    <div className="slide-text" style={{ ...textStyle(element, theme), justifyContent: element.format?.vertical === 'middle' ? 'center' : element.format?.vertical === 'bottom' ? 'flex-end' : 'flex-start' }}>{content}</div>
  </>
})

type TransformPreview = { id: string; slideId: string; mode: 'move' | 'resize'; x: number; y: number; width: number; height: number; pending: boolean }
type Gesture = { element: Element; preview: TransformPreview; pointerId: number; startX: number; startY: number; scale: number; control: HTMLButtonElement }

export function SlideSurface({ slide, design, selected, onSelect, onMove, onResize, onEdit, editRequest, onContextMenu, onDraftChange }: Props) {
  const host = useRef<HTMLDivElement>(null)
  const [scale, setScale] = useState(0.5)
  const [editing, setEditing] = useState<{ slide: string; id: string } | null>(null)
  const [cancelledRequest, setCancelledRequest] = useState(0)
  const [preview, setPreview] = useState<TransformPreview | null>(null)
  const [gestureError, setGestureError] = useState('')
  const gesture = useRef<Gesture | null>(null)
  const frame = useRef<number | null>(null)
  if (preview && preview.slideId !== slide.id) { setPreview(null); setGestureError('') }
  const activePreview = preview?.slideId === slide.id ? preview : null
  const layout = design?.layouts.find((entry) => entry.id === (slide.layout_id ?? design.layouts[0]?.id))
  const master = design?.masters.find((entry) => entry.id === layout?.master_id)
  const background = slide.inherit_background ? layout?.background ?? master?.background ?? slide.background : slide.background
  const common = slide.hide_master_graphics ? [] : [...(master?.elements ?? []), ...(layout?.elements ?? [])].filter((element) => element.type !== 'text' || !element.format?.placeholder)
  const activeEditing = editing ?? (editRequest?.slideId === slide.id && editRequest.sequence !== cancelledRequest && slide.elements.some((element) => element.id === editRequest.id) ? { slide: slide.id, id: editRequest.id } : null)
  function finishEditing() { setEditing(null); setCancelledRequest(editRequest?.sequence ?? 0) }
  function cancelFrame() { if (frame.current !== null) { cancelAnimationFrame(frame.current); frame.current = null } }
  function releasePointer(current: Gesture) { if (current.control.hasPointerCapture(current.pointerId)) current.control.releasePointerCapture(current.pointerId) }
  function cancelGesture(event?: ReactPointerEvent<HTMLButtonElement>) {
    const current = gesture.current
    if (!current || current.preview.pending || event && event.pointerId !== current.pointerId) return
    gesture.current = null; cancelFrame(); releasePointer(current); setPreview(null)
  }
  function beginGesture(event: ReactPointerEvent<HTMLButtonElement>, element: Element, mode: TransformPreview['mode']) {
    if (event.button !== 0 || !event.isPrimary || gesture.current || mode === 'move' && !onMove) return
    event.preventDefault(); event.stopPropagation()
    event.currentTarget.focus({ preventScroll: true })
    onSelect?.(element.id)
    const next: TransformPreview = { id: element.id, slideId: slide.id, mode, x: element.x, y: element.y, width: element.width, height: element.height, pending: false }
    gesture.current = { element, preview: next, pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, scale, control: event.currentTarget }
    event.currentTarget.setPointerCapture(event.pointerId)
    setGestureError(''); setPreview(next)
  }
  function calculatePreview(current: Gesture, clientX: number, clientY: number): TransformPreview {
    const element = current.element
    if (Math.hypot(clientX - current.startX, clientY - current.startY) < 3) return { ...current.preview, x: element.x, y: element.y, width: element.width, height: element.height }
    const horizontal = (clientX - current.startX) / current.scale
    const vertical = (clientY - current.startY) / current.scale
    if (current.preview.mode === 'move') return { ...current.preview, x: Math.max(0, Math.min(1280 - element.width, Math.round(element.x + horizontal))), y: Math.max(0, Math.min(720 - element.height, Math.round(element.y + vertical))) }
    const angle = element.type === 'shape' ? element.rotation * Math.PI / 180 : 0
    return { ...current.preview, width: Math.max(1, Math.min(1280 - element.x, Math.round(element.width + horizontal * Math.cos(angle) + vertical * Math.sin(angle)))), height: Math.max(1, Math.min(720 - element.y, Math.round(element.height - horizontal * Math.sin(angle) + vertical * Math.cos(angle)))) }
  }
  function moveGesture(event: ReactPointerEvent<HTMLButtonElement>) {
    const current = gesture.current
    if (!current || current.preview.pending || current.pointerId !== event.pointerId || current.control !== event.currentTarget) return
    current.preview = calculatePreview(current, event.clientX, event.clientY)
    if (frame.current === null) frame.current = requestAnimationFrame(() => { frame.current = null; setPreview(gesture.current?.preview ?? null) })
  }
  async function finishGesture(event: ReactPointerEvent<HTMLButtonElement>) {
    const current = gesture.current
    if (!current || current.preview.pending || current.pointerId !== event.pointerId || current.control !== event.currentTarget) return
    cancelFrame()
    current.preview = { ...calculatePreview(current, event.clientX, event.clientY), pending: true }
    setPreview(current.preview); releasePointer(current)
    const next = current.preview
    try {
      if (next.mode === 'move' && (next.x !== current.element.x || next.y !== current.element.y)) await onMove?.(next.id, next.x, next.y)
      if (next.mode === 'resize' && (next.width !== current.element.width || next.height !== current.element.height)) await onResize?.(next.id, next.width, next.height)
    } catch (reason) { setGestureError(reason instanceof Error ? reason.message : String(reason)) }
    finally { if (gesture.current === current) { gesture.current = null; setPreview(null) } }
  }
  useEffect(() => () => {
    const current = gesture.current
    gesture.current = null
    if (frame.current !== null) { cancelAnimationFrame(frame.current); frame.current = null }
    if (current?.control.hasPointerCapture(current.pointerId)) current.control.releasePointerCapture(current.pointerId)
  }, [slide.id])
  useEffect(() => {
    const current = gesture.current
    if (!current || current.preview.pending || onSelect && slide.elements.includes(current.element)) return
    gesture.current = null
    if (frame.current !== null) { cancelAnimationFrame(frame.current); frame.current = null }
    if (current.control.hasPointerCapture(current.pointerId)) current.control.releasePointerCapture(current.pointerId)
    setPreview(null)
  }, [onSelect, slide.elements])
  useEffect(() => {
    const node = host.current
    if (!node) return
    const observer = new ResizeObserver(([entry]) => setScale(entry.contentRect.width / 1280))
    observer.observe(node)
    return () => observer.disconnect()
  }, [])
  return <div className="slide-surface" ref={host} aria-busy={activePreview?.pending || undefined}>
    <div className="slide-page" style={{ transform: `scale(${scale})`, background: cssColor(background, design?.theme) }}>
      <div className="master-graphics">{common.map((element, index) => <div key={index} className="slide-element" style={{ left: element.x, top: element.y, width: element.width, height: element.height, transform: element.type === 'shape' ? `rotate(${element.rotation}deg)` : undefined }}><Content element={element} theme={design?.theme} /></div>)}</div>
      {slide.elements.map((element) => {
        const temporary = activePreview?.id === element.id ? activePreview : null
        const transform = [temporary?.mode === 'move' ? `translate3d(${temporary.x - element.x}px, ${temporary.y - element.y}px, 0)` : '', element.type === 'shape' ? `rotate(${element.rotation}deg)` : ''].filter(Boolean).join(' ')
        return <div key={element.id} data-element-id={element.id} className={`slide-element ${temporary?.mode === 'move' ? 'moving' : ''} ${onSelect && selected === element.id ? 'selected' : ''} ${activeEditing?.slide === slide.id && activeEditing.id === element.id ? 'editing' : ''}`} style={{ left: element.x, top: element.y, width: temporary?.mode === 'resize' ? temporary.width : element.width, height: temporary?.mode === 'resize' ? temporary.height : element.height, transform: transform || undefined }}>
        <Content element={temporary?.mode === 'resize' ? { ...element, width: temporary.width, height: temporary.height } : element} theme={design?.theme} />
        {activeEditing?.slide === slide.id && activeEditing.id === element.id && onEdit ? <InlineEditor key={element.id} element={element} theme={design?.theme} onCommit={onEdit} onCancel={finishEditing} onDraftChange={onDraftChange} /> : onSelect && <button type="button" className="element-hitbox" aria-label={`Edit ${element.id}`} title={element.id}
          onContextMenu={(event) => { event.preventDefault(); event.stopPropagation(); onContextMenu?.({ x: event.clientX, y: event.clientY, anchor: event.currentTarget }, element.id) }}
          onClick={() => onSelect(element.id)}
          onDoubleClick={() => { if (onEdit && ['text', 'shape', 'table', 'group'].includes(element.type)) { cancelGesture(); setEditing({ slide: slide.id, id: element.id }) } }}
          onDragStart={(event) => event.preventDefault()}
          onPointerDown={(event) => beginGesture(event, element, 'move')}
          onPointerMove={moveGesture}
          onPointerUp={finishGesture}
          onPointerCancel={cancelGesture}
          onLostPointerCapture={cancelGesture}
          onKeyDown={(event) => {
            if (event.key === 'Escape' && gesture.current) { event.preventDefault(); event.stopPropagation(); cancelGesture(); return }
            if (gesture.current) return
            if (event.key === 'ContextMenu' || event.shiftKey && event.key === 'F10') { event.preventDefault(); event.stopPropagation(); const bounds = event.currentTarget.getBoundingClientRect(); onContextMenu?.({ x: bounds.left + 12, y: bounds.top + 12, anchor: event.currentTarget }, element.id); return }
            if (onEdit && ['text', 'shape', 'table', 'group'].includes(element.type) && (event.key === 'Enter' || event.key === 'F2')) {
              event.preventDefault(); setEditing({ slide: slide.id, id: element.id }); return
            }
            const increment = event.shiftKey ? 10 : 1
            const delta = { ArrowLeft: [-increment, 0], ArrowRight: [increment, 0], ArrowUp: [0, -increment], ArrowDown: [0, increment] }[event.key]
            if (!delta) return
            event.preventDefault()
            onMove?.(element.id, Math.max(0, Math.min(1280 - element.width, element.x + delta[0])), Math.max(0, Math.min(720 - element.height, element.y + delta[1])))
          }} />}
        {onSelect && onResize && selected === element.id && !activeEditing && <button type="button" className="resize-handle" aria-label={`Resize ${element.id}`} title={`Resize ${element.id}`} style={{ width: 24 / scale, height: 24 / scale, right: -8 / scale, bottom: -8 / scale }}
          onPointerDown={(event) => beginGesture(event, element, 'resize')}
          onPointerMove={moveGesture}
          onPointerUp={finishGesture}
          onPointerCancel={cancelGesture}
          onLostPointerCapture={cancelGesture}
          onKeyDown={(event) => { if (event.key === 'Escape' && gesture.current) { event.preventDefault(); event.stopPropagation(); cancelGesture(); return }; if (gesture.current) return; const amount = event.shiftKey ? 10 : 1; const delta = { ArrowLeft: [-amount, 0], ArrowRight: [amount, 0], ArrowUp: [0, -amount], ArrowDown: [0, amount] }[event.key]; if (!delta) return; event.preventDefault(); void onResize(element.id, Math.max(1, Math.min(1280 - element.x, element.width + delta[0])), Math.max(1, Math.min(720 - element.y, element.height + delta[1]))) }}><span /></button>}
      </div>})}
    </div>
    {gestureError && <div className="canvas-interaction-error" role="alert">{gestureError}</div>}
  </div>
}