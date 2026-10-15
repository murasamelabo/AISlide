import { lazy, memo, Suspense, useEffect, useId, useRef, useState } from 'react'
import type { PointerEvent as ReactPointerEvent } from 'react'
import type { Design, Element, Slide, Theme, SelectionOperation } from './types'
import { InlineEditor } from './InlineEditor'
import { ShapeSurface } from './ShapeSurface'
import { cssColor, displayFieldElement, textStyle } from './design'
import type { MenuPosition } from './ContextMenu'
import { RichTextSurface } from './RichTextSurface'
import { TableSurface } from './TableSurface'
import { effectStyle, elementTransform, gradientDefinition, pathData, pictureMask, svgImageSource, visualOf, visualWarnings } from './visual-render'
import './document-render.css'

const ChartSurface = lazy(() => import('./ChartSurface').then((module) => ({ default: module.ChartSurface })))

type Props = { slide: Slide; pageNumber?: number; design?: Design | null; width?: number; height?: number; disabled?: boolean; onBusy?: (busy: boolean) => void; selected?: string | null; selectedIds?: string[]; onSelect?: (id: string, toggle?: boolean) => void; onSelectMany?: (ids: string[]) => void; onSelectionTransform?: (operation: Extract<SelectionOperation, { op: 'translate' | 'resize' }>) => void | Promise<void>; onMove?: (id: string, x: number, y: number) => void | Promise<void>; onResize?: (id: string, width: number, height: number) => void | Promise<void>; onEdit?: (element: Element) => Promise<void>; editRequest?: { id: string; slideId: string; sequence: number }; onContextMenu?: (position: MenuPosition, id?: string) => void; onDraftChange?: (dirty: boolean) => void }

export const Content = memo(function ElementContent({ element: source, theme, pageNumber }: { element: Element; theme?: Theme; pageNumber?: number }) {
  const element = displayFieldElement(source, pageNumber)
  const arrowId = useId()
  const gradientId = useId()
  const visual = visualOf(element)
  if (visual?.hidden) return null
  const definitions = <defs>{gradientDefinition({ id: gradientId, gradient: visual?.gradient, theme })}</defs>
  const fill = (color: string) => visual?.gradient ? `url(#${gradientId})` : color === 'none' ? 'none' : cssColor(color, theme)
  const content = (() => {
    if (element.type === 'group') return <div style={{ position: 'absolute', width: element.view_width, height: element.view_height, transformOrigin: 'top left', transform: `scale(${element.width / element.view_width}, ${element.height / element.view_height})` }}>
      {element.children.map((child) => <div key={child.id} style={{ position: 'absolute', left: child.x, top: child.y, width: child.width, height: child.height, transform: child.type === 'shape' ? `rotate(${child.rotation}deg)` : undefined }}><Content element={child} theme={theme} pageNumber={pageNumber} /></div>)}
    </div>
    if (element.type === 'picture') {
    const crop = element.crop ?? { left: 0, top: 0, right: 0, bottom: 0 }
    const visibleWidth = 1 - crop.left - crop.right
    const visibleHeight = 1 - crop.top - crop.bottom
    const fallback = `data:${element.mime_type};base64,${element.base64}`
    return <div style={{ width: '100%', height: '100%', overflow: 'hidden', position: 'relative', clipPath: pictureMask(visual?.picture_mask), opacity: visual?.opacity ?? undefined }}><img src={element.svg ? svgImageSource(element.svg) : fallback} onError={(event) => { if (event.currentTarget.getAttribute('src') !== fallback) event.currentTarget.src = fallback }} alt={element.alt} draggable={false} style={{ position: 'absolute', maxWidth: 'none', width: `${100 / visibleWidth}%`, height: `${100 / visibleHeight}%`, left: `${-100 * crop.left / visibleWidth}%`, top: `${-100 * crop.top / visibleHeight}%` }} /></div>
  }
  if (element.type === 'connector') return <svg className="connector-line" width={Math.max(1, element.width)} height={Math.max(1, element.height)} style={{ overflow: 'visible' }} aria-hidden="true">
    <defs><marker id={arrowId} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill={cssColor(element.color, theme)} /></marker></defs>
    {element.routing ? <polyline points={element.routing.points.map(([horizontal, vertical]) => `${horizontal * element.width},${vertical * element.height}`).join(' ')} fill="none" stroke={cssColor(element.color, theme)} strokeWidth={element.stroke_width} strokeDasharray={element.routing.dashed ? '8 5' : undefined} markerStart={element.routing.start_arrow ? `url(#${arrowId})` : undefined} markerEnd={element.arrow ? `url(#${arrowId})` : undefined} /> : <line x1="0" y1={element.flip_v ? element.height : 0} x2={element.width} y2={element.flip_v ? 0 : element.height} stroke={cssColor(element.color, theme)} strokeWidth={element.stroke_width} markerEnd={element.arrow ? `url(#${arrowId})` : undefined} />}
  </svg>
  if (element.type === 'chart') return <Suspense fallback={<div role="status" aria-label="Chart loading" style={{ width: element.width, height: element.height }} />}><ChartSurface element={element} theme={theme} /></Suspense>
  if (element.type === 'polygon') return <svg width={element.width} height={element.height} viewBox={`0 0 ${element.width} ${element.height}`} aria-hidden="true">{definitions}{visual?.path ? <path d={pathData(visual.path, element.width, element.height)} fill={fill(element.fill)} fillOpacity={visual.opacity ?? undefined} stroke={cssColor(element.stroke, theme)} strokeWidth={element.stroke_width} /> : <polygon points={element.points.map(([horizontal, vertical]) => `${horizontal * element.width},${vertical * element.height}`).join(' ')} fill={fill(element.fill)} fillOpacity={visual?.opacity ?? undefined} stroke={cssColor(element.stroke, theme)} strokeWidth={element.stroke_width} />}</svg>
  if (element.type === 'rect') return <svg width="100%" height="100%" aria-hidden="true">{definitions}<rect width="100%" height="100%" fill={fill(element.fill)} fillOpacity={visual?.opacity ?? undefined} /></svg>
  if (element.type === 'table') return <TableSurface element={element} theme={theme} />
  return <>
    {element.type === 'shape' && <ShapeSurface preset={element.preset} fill={fill(element.fill)} stroke={cssColor(element.stroke, theme)} strokeWidth={element.stroke_width} fillOpacity={visual?.opacity ?? undefined} adjustments={visual?.adjustments} width={element.width} height={element.height} nativeGeometry={Boolean(visual?.connection_sites?.length)}>{definitions}</ShapeSurface>}
    <div className="slide-text" style={{ ...textStyle(element, theme), textDecoration: 'none', justifyContent: element.format?.vertical === 'middle' ? 'center' : element.format?.vertical === 'bottom' ? 'flex-end' : 'flex-start' }}><RichTextSurface text={element.text} format={element.format} size={element.font_size} theme={theme} warp={visual?.text_warp} /></div>
  </>
  })()
  const transform = element.type === 'shape' ? elementTransform({ ...element, rotation: 0 }) : elementTransform(element)
  return <div className="document-ink" data-preview-warnings={visualWarnings(element).join('; ') || undefined} title={visualWarnings(element).join('; ') || undefined} style={{ ...effectStyle(visual, theme), transform }}>{content}</div>
})

type TransformPreview = { id: string; slideId: string; mode: 'move' | 'resize'; x: number; y: number; width: number; height: number; pending: boolean }
type Gesture = { element: Element; preview: TransformPreview; pointerId: number; startX: number; startY: number; scale: number; control: HTMLButtonElement }

export function SlideSurface({ slide, pageNumber, design, width = 1280, height = 720, disabled, onBusy, selected, selectedIds, onSelect, onSelectMany, onSelectionTransform, onMove, onResize, onEdit, editRequest, onContextMenu, onDraftChange }: Props) {
  function selectionBounds(elements: Element[]) {
    if (!elements.length) return null
    const x = Math.min(...elements.map((element) => element.x))
    const y = Math.min(...elements.map((element) => element.y))
    return { x, y, width: Math.max(...elements.map((element) => element.x + element.width)) - x, height: Math.max(...elements.map((element) => element.y + element.height)) - y }
  }
  const host = useRef<HTMLDivElement>(null)
  const [scale, setScale] = useState(0.5)
  const [editing, setEditing] = useState<{ slide: string; id: string } | null>(null)
  const [cancelledRequest, setCancelledRequest] = useState(0)
  const [preview, setPreview] = useState<TransformPreview | null>(null)
  const [transformSource, setTransformSource] = useState<{ elements: Element[]; base: Element } | null>(null)
  const [gestureError, setGestureError] = useState('')
  const gesture = useRef<(Gesture & { originals: Element[] }) | null>(null)
  const ids = selectedIds ?? (selected ? [selected] : [])
  const selectedElements = slide.elements.filter((element) => ids.includes(element.id) && !visualOf(element)?.hidden)
  const bounds = selectionBounds(selectedElements)
  const selectionLocked = selectedElements.some((element) => visualOf(element)?.locked)
  const marqueeGesture = useRef<{ pointerId: number; x: number; y: number; base: string[] } | null>(null)
  const [marquee, setMarquee] = useState<{ x: number; y: number; width: number; height: number } | null>(null)
  const frame = useRef<number | null>(null)
  if (preview && preview.slideId !== slide.id) { setPreview(null); setGestureError('') }
  const activePreview = preview?.slideId === slide.id ? preview : null
  const layout = design?.layouts.find((entry) => entry.id === (slide.layout_id ?? design.layouts[0]?.id))
  const master = design?.masters.find((entry) => entry.id === layout?.master_id)
  const theme = master?.theme ?? design?.theme
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
    if (disabled || event.button !== 0 || !event.isPrimary || gesture.current) return
    event.preventDefault(); event.stopPropagation()
    event.currentTarget.focus({ preventScroll: true })
    if (event.shiftKey || event.ctrlKey || event.metaKey) { onSelect?.(element.id, true); return }
    if (!ids.includes(element.id)) onSelect?.(element.id)
    if (visualOf(element)?.locked || visualOf(element)?.hidden || ids.includes(element.id) && selectionLocked || mode === 'move' && !onMove && !onSelectionTransform) return
    const originals = ids.includes(element.id) && onSelectionTransform ? selectedElements : [element]
    const frameBounds = selectionBounds(originals)!
    const frameElement = originals.length > 1 ? { ...element, ...frameBounds } : element
    const next: TransformPreview = { id: element.id, slideId: slide.id, mode, x: frameElement.x, y: frameElement.y, width: frameElement.width, height: frameElement.height, pending: false }
    gesture.current = { element: frameElement, originals, preview: next, pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, scale, control: event.currentTarget }
    event.currentTarget.setPointerCapture(event.pointerId)
    setTransformSource({ elements: originals, base: frameElement })
    setGestureError(''); setPreview(next)
  }
  function calculatePreview(current: Gesture, clientX: number, clientY: number): TransformPreview {
    const element = current.element
    if (Math.hypot(clientX - current.startX, clientY - current.startY) < 3) return { ...current.preview, x: element.x, y: element.y, width: element.width, height: element.height }
    const horizontal = (clientX - current.startX) / current.scale
    const vertical = (clientY - current.startY) / current.scale
    if (current.preview.mode === 'move') return { ...current.preview, x: Math.max(0, Math.min(width - element.width, Math.round(element.x + horizontal))), y: Math.max(0, Math.min(height - element.height, Math.round(element.y + vertical))) }
    const angle = gesture.current?.originals.length === 1 ? (element.type === 'shape' ? element.rotation : visualOf(element)?.rotation ?? 0) * Math.PI / 180 : 0
    return { ...current.preview, width: Math.max(1, Math.min(width - element.x, Math.round(element.width + horizontal * Math.cos(angle) + vertical * Math.sin(angle)))), height: Math.max(1, Math.min(height - element.y, Math.round(element.height - horizontal * Math.sin(angle) + vertical * Math.cos(angle)))) }
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
      if (next.mode === 'move' && (next.x !== current.element.x || next.y !== current.element.y)) {
        if (onSelectionTransform && current.originals.length > 1) await onSelectionTransform({ op: 'translate', ids: current.originals.map((element) => element.id), dx: next.x - current.element.x, dy: next.y - current.element.y })
        else await onMove?.(next.id, next.x, next.y)
      }
      if (next.mode === 'resize' && (next.width !== current.element.width || next.height !== current.element.height)) {
        if (onSelectionTransform && current.originals.length > 1) await onSelectionTransform({ op: 'resize', ids: current.originals.map((element) => element.id), x: next.x, y: next.y, width: next.width, height: next.height })
        else await onResize?.(next.id, next.width, next.height)
      }
    } catch (reason) { setGestureError(reason instanceof Error ? reason.message : String(reason)) }
    finally { if (gesture.current === current) { gesture.current = null; setPreview(null) } }
  }
  useEffect(() => () => {
    const current = gesture.current
    gesture.current = null
    marqueeGesture.current = null
    if (frame.current !== null) { cancelAnimationFrame(frame.current); frame.current = null }
    if (current?.control.hasPointerCapture(current.pointerId)) current.control.releasePointerCapture(current.pointerId)
  }, [slide.id])
  useEffect(() => {
    const current = gesture.current
    if (!current || current.preview.pending || onSelect && current.originals.every((element) => slide.elements.includes(element))) return
    gesture.current = null
    if (frame.current !== null) { cancelAnimationFrame(frame.current); frame.current = null }
    if (current.control.hasPointerCapture(current.pointerId)) current.control.releasePointerCapture(current.pointerId)
    setPreview(null)
  }, [onSelect, slide.elements])
  useEffect(() => {
    const node = host.current
    if (!node) return
    const observer = new ResizeObserver(([entry]) => setScale(entry.contentRect.width / width))
    observer.observe(node)
    return () => observer.disconnect()
  }, [width])
  function point(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect()
    return { x: Math.max(0, Math.min(width, (event.clientX - rect.left) * width / rect.width)), y: Math.max(0, Math.min(height, (event.clientY - rect.top) * width / rect.width)) }
  }
  function marqueeBounds(event: ReactPointerEvent<HTMLDivElement>) {
    const start = marqueeGesture.current!
    const current = point(event)
    return { x: Math.min(start.x, current.x), y: Math.min(start.y, current.y), width: Math.abs(start.x - current.x), height: Math.abs(start.y - current.y) }
  }
  function cancelMarquee() { marqueeGesture.current = null; setMarquee(null) }
  const frameBounds = activePreview && transformSource?.elements.length !== 1 ? activePreview : bounds
  function withoutOuterTransform(element: Element): Element {
    if (!elementTransform(element)) return element
    if (element.type === 'table' || element.type === 'chart') return element
    if (element.type === 'shape') return element
    return { ...element, visual: { ...element.visual, rotation: 0, flip_h: false, flip_v: false } }
  }
  function outerTransform(element: Element) {
    return elementTransform(element.type === 'shape' ? { ...element, visual: { ...element.visual, flip_h: false, flip_v: false } } : element)
  }
  return <div className="slide-surface" ref={host} style={{ aspectRatio: `${width} / ${height}` }} aria-busy={activePreview?.pending || undefined}
    onPointerDown={(event) => {
      if (disabled || !onSelectMany || !onSelect || activeEditing || event.button !== 0 || !event.isPrimary || gesture.current || (event.target as HTMLElement).closest('button,input,textarea,.inline-editor')) return
      event.preventDefault(); event.currentTarget.closest<HTMLElement>('.canvas-workspace')?.focus({ preventScroll: true })
      marqueeGesture.current = { pointerId: event.pointerId, ...point(event), base: event.shiftKey || event.ctrlKey || event.metaKey ? ids : [] }
      event.currentTarget.setPointerCapture(event.pointerId); setMarquee({ ...point(event), width: 0, height: 0 })
    }}
    onPointerMove={(event) => { if (marqueeGesture.current?.pointerId === event.pointerId) setMarquee(marqueeBounds(event)) }}
    onPointerUp={(event) => {
      const start = marqueeGesture.current
      if (!start || start.pointerId !== event.pointerId) return
      const area = marqueeBounds(event)
      const matches = area.width * scale < 3 && area.height * scale < 3 ? [] : slide.elements.filter((element) => !visualOf(element)?.hidden && !visualOf(element)?.locked && element.x < area.x + area.width && element.x + element.width > area.x && element.y < area.y + area.height && element.y + element.height > area.y).map((element) => element.id)
      onSelectMany?.([...new Set([...start.base, ...matches])]); cancelMarquee()
      if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
    }} onPointerCancel={cancelMarquee} onLostPointerCapture={cancelMarquee} onKeyDown={(event) => { if (event.key === 'Escape' && marqueeGesture.current) { event.preventDefault(); cancelMarquee() } }}>
    <div className="slide-page" style={{ width, height, transform: `scale(${scale})`, background: cssColor(background, theme) }}>
      <div className="master-graphics">{common.map((element, index) => <div key={index} className="slide-element" style={{ left: element.x, top: element.y, width: element.width, height: element.height, transform: outerTransform(element) }}><Content element={withoutOuterTransform(element)} theme={theme} pageNumber={pageNumber} /></div>)}</div>
      {slide.elements.map((element) => {
        const original = activePreview && transformSource?.elements.find((item) => item.id === element.id)
        const base = transformSource?.base
        const temporary = original && activePreview && base ? { ...activePreview, x: activePreview.x + (original.x - base.x) * (activePreview.mode === 'resize' ? activePreview.width / Math.max(1, base.width) : 1), y: activePreview.y + (original.y - base.y) * (activePreview.mode === 'resize' ? activePreview.height / Math.max(1, base.height) : 1), width: original.width * (activePreview.mode === 'resize' ? activePreview.width / Math.max(1, base.width) : 1), height: original.height * (activePreview.mode === 'resize' ? activePreview.height / Math.max(1, base.height) : 1) } : null
        const locked = Boolean(visualOf(element)?.locked)
        const hidden = Boolean(visualOf(element)?.hidden)
        const transform = [temporary?.mode === 'move' ? `translate3d(${temporary.x - element.x}px, ${temporary.y - element.y}px, 0)` : '', outerTransform(element)].filter(Boolean).join(' ')
        return <div key={element.id} data-element-id={element.id} data-locked={locked || undefined} aria-hidden={hidden || undefined} className={`slide-element ${temporary?.mode === 'move' ? 'moving' : ''} ${(onSelect || activePreview?.pending) && ids.includes(element.id) && !hidden ? 'selected' : ''} ${!hidden && activeEditing?.slide === slide.id && activeEditing.id === element.id ? 'editing' : ''}`} style={{ pointerEvents: hidden ? 'none' : undefined, left: temporary?.mode === 'resize' ? temporary.x : element.x, top: temporary?.mode === 'resize' ? temporary.y : element.y, width: temporary?.mode === 'resize' ? temporary.width : element.width, height: temporary?.mode === 'resize' ? temporary.height : element.height, transform: transform || undefined }}>
        <Content element={withoutOuterTransform(temporary?.mode === 'resize' ? { ...element, width: temporary.width, height: temporary.height } : element)} theme={theme} pageNumber={pageNumber} />
        {activeEditing?.slide === slide.id && activeEditing.id === element.id && onEdit && !locked && !hidden ? <InlineEditor key={element.id} element={element} theme={theme} onCommit={onEdit} onCancel={finishEditing} onDraftChange={onDraftChange} onBusy={onBusy} disabled={disabled} /> : onSelect && !hidden && <button type="button" className="element-hitbox" aria-label={`Edit ${element.id}`} title={element.id}
          onContextMenu={(event) => { event.preventDefault(); event.stopPropagation(); onContextMenu?.({ x: event.clientX, y: event.clientY, anchor: event.currentTarget }, element.id) }}
          onClick={(event) => { if (event.detail === 0) onSelect(element.id, event.shiftKey || event.ctrlKey || event.metaKey) }}
          onDoubleClick={() => { if (!disabled && !locked && onEdit && ['text', 'shape', 'table', 'group'].includes(element.type)) { cancelGesture(); onSelect(element.id); setEditing({ slide: slide.id, id: element.id }) } }}
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
            if (locked || disabled || selectionLocked) return
            if (onEdit && ['text', 'shape', 'table', 'group'].includes(element.type) && (event.key === 'Enter' || event.key === 'F2')) {
              event.preventDefault(); setEditing({ slide: slide.id, id: element.id }); return
            }
            const increment = event.shiftKey ? 10 : 1
            const delta = { ArrowLeft: [-increment, 0], ArrowRight: [increment, 0], ArrowUp: [0, -increment], ArrowDown: [0, increment] }[event.key]
            if (!delta) return
            event.preventDefault()
            if (ids.length > 1 && bounds && onSelectionTransform) { void onSelectionTransform({ op: 'translate', ids, dx: Math.max(-bounds.x, Math.min(width - bounds.x - bounds.width, delta[0])), dy: Math.max(-bounds.y, Math.min(height - bounds.y - bounds.height, delta[1])) }); return }
            onMove?.(element.id, Math.max(0, Math.min(width - element.width, element.x + delta[0])), Math.max(0, Math.min(height - element.height, element.y + delta[1])))
          }} />}
        {onSelect && onResize && ids.length === 1 && selected === element.id && !activeEditing && !locked && !hidden && <button type="button" className="resize-handle" aria-label={`Resize ${element.id}`} title={`Resize ${element.id}`} style={{ width: 24 / scale, height: 24 / scale, right: -8 / scale, bottom: -8 / scale }}
          onPointerDown={(event) => beginGesture(event, element, 'resize')}
          onPointerMove={moveGesture}
          onPointerUp={finishGesture}
          onPointerCancel={cancelGesture}
          onLostPointerCapture={cancelGesture}
          onKeyDown={(event) => { if (event.key === 'Escape' && gesture.current) { event.preventDefault(); event.stopPropagation(); cancelGesture(); return }; if (gesture.current) return; const amount = event.shiftKey ? 10 : 1; const delta = { ArrowLeft: [-amount, 0], ArrowRight: [amount, 0], ArrowUp: [0, -amount], ArrowDown: [0, amount] }[event.key]; if (!delta) return; event.preventDefault(); void onResize(element.id, Math.max(1, Math.min(width - element.x, element.width + delta[0])), Math.max(1, Math.min(height - element.y, element.height + delta[1]))) }}><span /></button>}
      </div>})}
      {ids.length > 1 && frameBounds && !selectionLocked && !activeEditing && <div className="selection-frame" style={{ left: frameBounds.x, top: frameBounds.y, width: frameBounds.width, height: frameBounds.height }}>{onSelect && onSelectionTransform && <button type="button" className="resize-handle" aria-label="Resize selection" style={{ width: 24 / scale, height: 24 / scale, right: -8 / scale, bottom: -8 / scale }} onPointerDown={(event) => beginGesture(event, selectedElements[0], 'resize')} onPointerMove={moveGesture} onPointerUp={finishGesture} onPointerCancel={cancelGesture} onLostPointerCapture={cancelGesture} onKeyDown={(event) => { if (event.key === 'Escape') { event.preventDefault(); cancelGesture(); return }; if (disabled || gesture.current) return; const amount = event.shiftKey ? 10 : 1; const delta = { ArrowLeft: [-amount, 0], ArrowRight: [amount, 0], ArrowUp: [0, -amount], ArrowDown: [0, amount] }[event.key]; if (!delta) return; event.preventDefault(); void onSelectionTransform({ op: 'resize', ids, x: frameBounds.x, y: frameBounds.y, width: Math.max(1, Math.min(width - frameBounds.x, frameBounds.width + delta[0])), height: Math.max(1, Math.min(height - frameBounds.y, frameBounds.height + delta[1])) }) }}><span /></button>}</div>}
      {marquee && <div className="selection-marquee" style={{ left: marquee.x, top: marquee.y, width: marquee.width, height: marquee.height }} />}
    </div>
    {gestureError && <div className="canvas-interaction-error" role="alert">{gestureError}</div>}
  </div>
}