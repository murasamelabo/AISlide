import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react'
import { ReactFlow, Background, Controls, Handle, Position, ConnectionMode, NodeResizer, BaseEdge, EdgeText, MarkerType, applyNodeChanges, applyEdgeChanges, getSmoothStepPath, getStraightPath } from '@xyflow/react'
import type { Node, Edge, NodeProps, EdgeProps, NodeChange, EdgeChange, Connection } from '@xyflow/react'
import { Square, RectangleHorizontal, Circle, Diamond, Database, Cloud, Plus, Trash2, Undo2, Redo2, AlignLeft, AlignCenterHorizontal, AlignRight, AlignStartVertical, AlignCenterVertical, AlignEndVertical, Grid2X2, Code2, Eye, MousePointer2, Copy, Check, Sticker, ArrowLeft } from 'lucide-react'
import { AislideClient } from '../../../packages/client/index.mjs'
import { core } from './api'
import { Content } from './SlideSurface'
import { ShapeSurface } from './ShapeSurface'
import { ColorField } from './TextControls'
import { Tool } from './Tool'
import { ContextMenu } from './ContextMenu'
import type { MenuCommand, MenuPosition } from './ContextMenu'
import { cssColor, fontFamily } from './design'
import type { AssetInput, GraphCatalog, GraphSpec, GraphNode, GraphEdge, GraphGroup, GraphNodeKind, GraphOperation, GraphIcon, Element, Theme } from './types'
import '@xyflow/react/dist/style.css'
import './graph-editor.css'

const client = new AislideClient(core)
const AssetPanel = lazy(() => import('./AssetPanel').then((module) => ({ default: module.AssetPanel })))
let previewQueue: Promise<void> = Promise.resolve()
const kinds: Record<GraphNodeKind, { preset: string; name: string; icon: typeof Square }> = {
  rectangle: { preset: 'rect', name: 'Rectangle', icon: Square },
  rounded_rectangle: { preset: 'roundRect', name: 'Rounded rectangle', icon: RectangleHorizontal },
  ellipse: { preset: 'ellipse', name: 'Ellipse', icon: Circle },
  diamond: { preset: 'diamond', name: 'Decision', icon: Diamond },
  cylinder: { preset: 'can', name: 'Database', icon: Database },
  cloud: { preset: 'cloud', name: 'Cloud', icon: Cloud },
}
const portPositions = { top: Position.Top, left: Position.Left, bottom: Position.Bottom, right: Position.Right }
type Bounds = { x: number; y: number; width: number; height: number }
type GraphFlowNode = Node<{ item: GraphNode | GraphGroup; boundary: boolean; theme?: Theme; label?: Element; icon?: Element; resize: (id: string, bounds: Bounds) => void }>
type GraphFlowEdge = Edge<{ points?: [number, number][]; elbow: boolean; label?: Element; theme?: Theme }>
type View = 'edit' | 'preview' | 'json'
type IconTarget = { kind: 'new' } | { kind: 'node'; node: GraphNode } | { kind: 'group'; group: GraphGroup }

function groupAncestors(groups: GraphGroup[], parent?: string | null) {
  const ancestors: string[] = []
  while (parent && !ancestors.includes(parent) && ancestors.length < groups.length) {
    ancestors.push(parent)
    parent = groups.find((group) => group.id === parent)?.parent
  }
  return ancestors
}

function serviceFrame(graph: GraphSpec, parent?: GraphGroup): Bounds {
  const width = 160, height = 140
  const left = parent ? parent.x + 8 : 24
  const top = parent ? parent.y + 40 : 104
  const right = parent ? parent.x + parent.width - 8 : 1128
  const bottom = parent ? parent.y + parent.height - 8 : 504
  const occupied = [...graph.nodes.filter((node) => (node.group ?? null) === (parent?.id ?? null)), ...(graph.groups ?? []).filter((group) => (group.parent ?? null) === (parent?.id ?? null))]
  for (let y = top; y + height <= bottom; y += 8) {
    for (let x = left; x + width <= right; x += 8) {
      if (occupied.every((item) => x + width + 8 <= item.x || x >= item.x + (item.width ?? 176) + 8 || y + height + 8 <= item.y || y >= item.y + (item.height ?? 80) + 8)) return { x, y, width, height }
    }
  }
  throw new Error('No room for a 160 x 140 service icon. Enlarge the boundary or move existing items.')
}

function appendHistory(entries: GraphSpec[], entry: GraphSpec) {
  const next = [...entries, structuredClone(entry)].slice(-30)
  const encoder = new TextEncoder()
  let bytes = 0
  let start = next.length
  while (start > 0) {
    bytes += encoder.encode(JSON.stringify(next[start - 1])).byteLength
    if (bytes > 4 * 1024 * 1024) break
    start -= 1
  }
  return next.slice(start)
}

function GraphNodeView({ id, data, selected }: NodeProps<GraphFlowNode>) {
  const item = data.item
  const node = item as GraphNode
  return <div className={`graph-node-content ${data.boundary ? 'graph-boundary' : ''}`}>
    {(data.boundary || node.presentation !== 'icon') && <ShapeSurface preset={data.boundary ? 'rect' : kinds[node.kind ?? 'rectangle'].preset} fill={cssColor(item.fill ?? (data.boundary ? '@lt2' : '@lt1'), data.theme)} stroke={cssColor(item.stroke ?? '@accent1', data.theme)} strokeWidth={1.5} />}
    {data.icon && <div className="graph-node-icon" style={{ left: data.icon.x - item.x, top: data.icon.y - item.y, width: data.icon.width, height: data.icon.height }}><Content element={data.icon} theme={data.theme} /></div>}
    {data.label ? <div className="graph-fitted-label" style={{ left: data.label.x - item.x, top: data.label.y - item.y, width: data.label.width, height: data.label.height }}><Content element={data.label} theme={data.theme} /></div> : <div className={`graph-node-label ${node.kind ?? 'rectangle'}`} style={{ color: cssColor(node.color ?? '@dk1', data.theme), fontFamily: fontFamily('@minor', data.theme), fontSize: data.boundary ? 18 : node.font_size ?? 18 }}>{item.label}</div>}
    <NodeResizer isVisible={selected} minWidth={64} minHeight={40} maxWidth={1152} maxHeight={424} onResizeEnd={(_event, bounds) => data.resize(id, bounds)} />
    {!data.boundary && Object.entries(portPositions).map(([port, position]) => <Handle key={port} id={port} type="source" position={position} style={node.kind === 'cloud' ? { top: port === 'top' ? `${1235 / 216}%` : port === 'bottom' ? `${21577 / 216}%` : '50%', bottom: 'auto', left: port === 'left' ? `${67 / 216}%` : port === 'right' ? `${21582 / 216}%` : '50%', right: 'auto', transform: 'translate(-50%, -50%)' } : undefined} title={`${item.label} ${port} connection`} aria-hidden="true" />)}
  </div>
}

function GraphEdgeView({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data, markerEnd, markerStart, style, label, selected }: EdgeProps<GraphFlowEdge>) {
  const points = data?.points
  const stable = points && Math.abs(points[0][0] - sourceX) < 2 && Math.abs(points[0][1] - sourceY) < 2 && Math.abs(points.at(-1)![0] - targetX) < 2 && Math.abs(points.at(-1)![1] - targetY) < 2
  const fallback = data?.elbow ? getSmoothStepPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, borderRadius: 0 }) : getStraightPath({ sourceX, sourceY, targetX, targetY })
  const path = stable ? points.map(([horizontal, vertical], index) => `${index ? 'L' : 'M'}${horizontal},${vertical}`).join(' ') : fallback[0]
  const nativeLabel = data?.label
  const horizontal = Math.abs(targetX - sourceX) >= Math.abs(targetY - sourceY)
  const shiftX = points && !stable ? (sourceX + targetX - points[0][0] - points.at(-1)![0]) / 2 : 0
  const shiftY = points && !stable ? (sourceY + targetY - points[0][1] - points.at(-1)![1]) / 2 : 0
  return <><BaseEdge path={path} markerEnd={markerEnd} markerStart={markerStart} style={{ ...style, strokeWidth: selected ? 3 : 2 }} />{label && (nativeLabel ? <foreignObject className="graph-edge-label" x={nativeLabel.x + shiftX} y={nativeLabel.y + shiftY} width={nativeLabel.width} height={nativeLabel.height} style={{ pointerEvents: 'none', overflow: 'visible' }}><Content element={nativeLabel} theme={data?.theme} /></foreignObject> : <EdgeText x={fallback[1] + (horizontal ? 0 : String(label).length * 4.5 + 12)} y={fallback[2] - (horizontal ? 22 : 0)} label={label} labelShowBg={false} labelStyle={{ fontSize: 16, fill: 'var(--text)' }} />)}</>
}
const nodeTypes = { graphNode: GraphNodeView }
const edgeTypes = { graphEdge: GraphEdgeView }

function GraphTool({ label, Icon, onClick, disabled }: { label: string; Icon: typeof Square; onClick: () => void; disabled: boolean }) {
  return <Tool label={label} onClick={onClick} disabled={disabled}><Icon size={20} /></Tool>
}

function GeometryFields({ item, onChange }: { item: { x: number; y: number; width?: number; height?: number }; onChange: (key: keyof Bounds, value: number) => void }) {
  return <div className="graph-geometry">{(['x', 'y', 'width', 'height'] as const).map((field) => <label className="field" key={field}>{field}<input aria-label={`Graph ${field}`} required type="number" step={1} value={Number.isFinite(item[field]) ? item[field] : item[field] === undefined ? field === 'width' ? 176 : 80 : ''} onChange={(event) => onChange(field, event.currentTarget.valueAsNumber)} /></label>)}</div>
}
function NodeProperties({ node, groups, theme, onApply, onChooseIcon }: { node: GraphNode; groups: GraphGroup[]; theme?: Theme; onApply: (node: GraphNode) => void; onChooseIcon: (node: GraphNode) => void }) {
  const [draft, setDraft] = useState(node)
  return <form onSubmit={(event) => { event.preventDefault(); onApply(draft) }} className="graph-properties-form">
    <label className="field">Label<textarea aria-label="Node label" required rows={3} maxLength={160} value={draft.label} onChange={(event) => setDraft({ ...draft, label: event.target.value })} /></label>
    <div className="graph-node-assets">{draft.icon && <img className="graph-icon-thumbnail" src={`data:${draft.icon.mime_type};base64,${draft.icon.base64}`} alt={draft.icon.alt || 'Node icon'} />}<button type="button" className="secondary" aria-label={draft.icon ? 'Change node icon' : 'Choose node icon'} onClick={() => onChooseIcon(draft)}><Sticker size={18} />{draft.icon ? 'Change icon' : 'Choose icon'}</button>{draft.icon && <Tool label="Remove node icon" onClick={() => onApply({ ...draft, icon: null, ...(draft.presentation === 'icon' ? { presentation: 'card' as const } : {}) })}><Trash2 size={20} /></Tool>}</div>
    <label className="field">Presentation<select aria-label="Node presentation" value={draft.presentation ?? 'card'} onChange={(event) => setDraft({ ...draft, presentation: event.target.value as GraphNode['presentation'] })}><option value="card">Card</option><option value="icon" disabled={!draft.icon}>Icon</option></select></label>
    <label className="field">Shape<select aria-label="Node shape" value={draft.kind ?? 'rectangle'} onChange={(event) => setDraft({ ...draft, kind: event.target.value as GraphNodeKind })}>{Object.entries(kinds).map(([value, entry]) => <option key={value} value={value}>{entry.name}</option>)}</select></label>
    <GeometryFields item={draft} onChange={(key, value) => setDraft({ ...draft, [key]: value })} />
    <label className="field">Group<select aria-label="Node group" value={draft.group ?? ''} onChange={(event) => setDraft({ ...draft, group: event.target.value || null })}><option value="">None</option>{groups.map((group) => <option key={group.id} value={group.id}>{group.label}</option>)}</select></label>
    {draft.presentation !== 'icon' && <><ColorField label="Node fill" value={draft.fill ?? '@lt1'} theme={theme} onChange={(fill) => setDraft({ ...draft, fill })} /><ColorField label="Node outline" value={draft.stroke ?? '@accent1'} theme={theme} onChange={(stroke) => setDraft({ ...draft, stroke })} /></>}
    <ColorField label="Node text" value={draft.color ?? '@dk1'} theme={theme} onChange={(color) => setDraft({ ...draft, color })} />
    <label className="field">Font size<input aria-label="Node font size" type="number" required min={12} max={40} value={draft.font_size ?? 18} onChange={(event) => setDraft({ ...draft, font_size: event.currentTarget.valueAsNumber })} /></label>
    <button className="secondary" type="submit"><Check size={16} />Apply node</button>
  </form>
}
function EdgeProperties({ edge, nodes, theme, onApply }: { edge: GraphEdge; nodes: GraphNode[]; theme?: Theme; onApply: (edge: GraphEdge) => void }) {
  const [draft, setDraft] = useState(edge)
  return <form className="graph-properties-form" onSubmit={(event) => { event.preventDefault(); onApply(draft) }}>
    <label className="field">Label<input aria-label="Edge label" maxLength={64} value={draft.label ?? ''} onChange={(event) => setDraft({ ...draft, label: event.target.value })} /></label>
    {(['source', 'target'] as const).map((key) => <div className="graph-connection-fields" key={key}><label className="field">{key}<select aria-label={`Edge ${key}`} value={draft[key]} onChange={(event) => setDraft({ ...draft, [key]: event.target.value })}>{nodes.map((node) => <option key={node.id} value={node.id}>{node.label}</option>)}</select></label><label className="field">Port<select aria-label={`Edge ${key} port`} value={draft[`${key}_port`] ?? 'auto'} onChange={(event) => setDraft({ ...draft, [`${key}_port`]: event.target.value })}>{['auto', 'top', 'left', 'bottom', 'right'].map((port) => <option key={port}>{port}</option>)}</select></label></div>)}
    <label className="field">Route<select aria-label="Edge route" value={draft.route ?? 'straight'} onChange={(event) => setDraft({ ...draft, route: event.target.value as GraphEdge['route'] })}><option value="straight">Straight</option><option value="elbow">Right angle</option></select></label>
    <label className="field">Arrowheads<select aria-label="Edge arrowheads" value={`${draft.start_arrow ? '1' : '0'}${draft.arrow !== false ? '1' : '0'}`} onChange={(event) => setDraft({ ...draft, start_arrow: event.target.value[0] === '1', arrow: event.target.value[1] === '1' })}><option value="00">None</option><option value="01">End</option><option value="10">Start</option><option value="11">Both</option></select></label>
    <label className="checkbox"><input type="checkbox" checked={Boolean(draft.dashed)} onChange={(event) => setDraft({ ...draft, dashed: event.target.checked })} />Dashed line</label>
    <ColorField label="Edge color" value={draft.color ?? '@dk2'} theme={theme} onChange={(color) => setDraft({ ...draft, color })} />
    <button className="secondary" type="submit"><Check size={16} />Apply edge</button>
  </form>
}
function GroupProperties({ group, groups, theme, onApply, onChooseIcon }: { group: GraphGroup; groups: GraphGroup[]; theme?: Theme; onApply: (group: GraphGroup) => void; onChooseIcon: (group: GraphGroup) => void }) {
  const [draft, setDraft] = useState(group)
  const parents = groups.filter((entry) => entry.id !== group.id && !groupAncestors(groups, entry.parent).includes(group.id))
  return <form className="graph-properties-form" onSubmit={(event) => { event.preventDefault(); onApply(draft) }}>
    <label className="field">Label<input aria-label="Group label" required maxLength={64} value={draft.label} onChange={(event) => setDraft({ ...draft, label: event.target.value })} /></label>
    <div className="graph-node-assets">{draft.icon && <img className="graph-icon-thumbnail" src={`data:${draft.icon.mime_type};base64,${draft.icon.base64}`} alt={draft.icon.alt || 'Boundary icon'} />}<button type="button" className="secondary" aria-label={draft.icon ? 'Change group icon' : 'Choose group icon'} onClick={() => onChooseIcon(draft)}><Sticker size={18} />{draft.icon ? 'Change icon' : 'Choose icon'}</button>{draft.icon && <Tool label="Remove group icon" onClick={() => onApply({ ...draft, icon: null })}><Trash2 size={20} /></Tool>}</div>
    <label className="field">Parent<select aria-label="Group parent" value={draft.parent ?? ''} onChange={(event) => setDraft({ ...draft, parent: event.target.value || null })}><option value="">None</option>{parents.map((entry) => <option key={entry.id} value={entry.id}>{entry.label}</option>)}</select></label>
    <GeometryFields item={draft} onChange={(key, value) => setDraft({ ...draft, [key]: value })} />
    <ColorField label="Group fill" value={draft.fill ?? '@lt2'} theme={theme} onChange={(fill) => setDraft({ ...draft, fill })} />
    <ColorField label="Group outline" value={draft.stroke ?? '@dk2'} theme={theme} onChange={(stroke) => setDraft({ ...draft, stroke })} />
    <button className="secondary" type="submit"><Check size={16} />Apply group</button>
  </form>
}

function NativePreview({ element, theme }: { element: Element; theme?: Theme }) {
  const container = useRef<HTMLDivElement>(null)
  const [scale, setScale] = useState(1)
  useEffect(() => {
    const observer = new ResizeObserver(([entry]) => setScale(entry.contentRect.width / 1152))
    if (container.current) observer.observe(container.current)
    return () => observer.disconnect()
  }, [])
  return <div ref={container} className="graph-native-preview"><div className="graph-native-frame" style={{ height: 512 * scale }}><div style={{ transform: `scale(${scale})`, transformOrigin: 'top left', width: 1152, height: 512 }}><Content element={element} theme={theme} /></div></div></div>
}

export function GraphEditor({ catalog, initial, theme, editing, onApply, onBusy }: { catalog: GraphCatalog; initial?: GraphSpec; theme?: Theme; editing: boolean; onApply: (spec: GraphSpec) => Promise<void>; onBusy: (busy: boolean) => void }) {
  const [spec, setSpec] = useState<GraphSpec>(() => structuredClone(initial ?? catalog.examples[0].spec))
  const current = useRef(spec)
  const [nodes, setNodes] = useState<GraphFlowNode[]>([])
  const [edges, setEdges] = useState<GraphFlowEdge[]>([])
  const [selected, setSelected] = useState<string[]>([])
  const selection = useRef(selected)
  const [view, setView] = useState<View>('edit')
  const [raw, setRaw] = useState('')
  const [preview, setPreview] = useState<Element | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [snap, setSnap] = useState(true)
  const [from, setFrom] = useState(spec.nodes[0]?.id ?? '')
  const [to, setTo] = useState(spec.nodes[1]?.id ?? '')
  const gate = useRef(false)
  const [history, setHistory] = useState<{ past: GraphSpec[]; future: GraphSpec[] }>({ past: [], future: [] })
  const [menu, setMenu] = useState<MenuPosition | null>(null)
  const [iconTarget, setIconTarget] = useState<IconTarget | null>(null)
  const iconTrigger = useRef<HTMLElement | null>(null)
  const editor = useRef<HTMLDivElement>(null)
  const closeMenu = useCallback(() => setMenu(null), [])
  const metadataBefore = useRef<GraphSpec | null>(null)
  const action = useRef<(operations: GraphOperation[]) => void>(() => {})
  const resize = useCallback((id: string, bounds: Bounds) => {
    const graph = current.current
    const group = graph.groups?.find((group) => group.id === id)
    if (group) {
      const parent = graph.groups?.find((entry) => entry.id === group.parent)
      action.current([{ op: 'put_group', group: { ...group, ...bounds, x: bounds.x + (parent?.x ?? 0), y: bounds.y + (parent?.y ?? 0) } }]); return
    }
    const node = graph.nodes.find((node) => node.id === id)
    if (!node) return
    const parent = graph.groups?.find((group) => group.id === node.group)
    action.current([{ op: 'put_node', node: { ...node, ...bounds, x: bounds.x + (parent?.x ?? 0), y: bounds.y + (parent?.y ?? 0) } }])
  }, [])
  const install = useCallback((next: GraphSpec, rendered?: Element) => {
    current.current = next; setSpec(next)
    selection.current = selection.current.filter((id) => [...next.nodes, ...next.edges ?? [], ...next.groups ?? []].some((entry) => entry.id === id))
    setSelected(selection.current)
    const nativeChildren = rendered?.type === 'group' ? rendered.children : []
    const nativePrefix = nativeChildren[0]?.id.replace(/-title$/, '')
    const groups = next.groups ?? []
    const ordered = [...groups].sort((left, right) => groupAncestors(groups, left.parent).length - groupAncestors(groups, right.parent).length)
    const members: GraphFlowNode[] = ordered.map((item) => {
      const parent = groups.find((group) => group.id === item.parent)
      return { id: item.id, type: 'graphNode', position: { x: item.x - (parent?.x ?? 0), y: item.y - (parent?.y ?? 0) }, parentId: parent?.id, extent: parent ? [[8, 40], [parent.width - 8, parent.height - 8]] : undefined, width: item.width, height: item.height, style: { width: item.width, height: item.height }, data: { item, boundary: true, theme, resize, label: nativeChildren.find((entry) => entry.id.endsWith(`-gt-${item.id}`)), icon: nativeChildren.find((entry) => entry.type === 'picture' && entry.id.endsWith(`-gi-${item.id}`)) }, selected: selection.current.includes(item.id), ariaLabel: `Group ${item.label}`, zIndex: -1 }
    })
    for (const item of next.nodes) {
      const parent = next.groups?.find((group) => group.id === item.group)
      members.push({ id: item.id, type: 'graphNode', position: { x: item.x - (parent?.x ?? 0), y: item.y - (parent?.y ?? 0) }, parentId: parent?.id, extent: parent ? [[8, 40], [parent.width - 8, parent.height - 8]] : undefined, width: item.width ?? 176, height: item.height ?? 80, style: { width: item.width ?? 176, height: item.height ?? 80 }, data: { item, boundary: false, theme, resize, label: nativeChildren.find((entry) => entry.id.endsWith(`-nt-${item.id}`)), icon: nativeChildren.find((entry) => entry.type === 'picture' && entry.id.endsWith(`-ni-${item.id}`)) }, selected: selection.current.includes(item.id), ariaLabel: `Node ${item.label}` })
    }
    setNodes((previous) => {
      const measuredById = new Map(previous.map((entry) => [entry.id, entry.measured]))
      return members.map((entry) => {
        const measured = measuredById.get(entry.id)
        return measured?.width === entry.width && measured?.height === entry.height ? { ...entry, measured } : entry
      })
    })
    setEdges((next.edges ?? []).map((item) => {
      const source = next.nodes.find((node) => node.id === item.source)!
      const target = next.nodes.find((node) => node.id === item.target)!
      const automatic = (node: GraphNode, other: GraphNode) => {
        const horizontal = other.x + (other.width ?? 176) / 2 - node.x - (node.width ?? 176) / 2
        const vertical = other.y + (other.height ?? 80) / 2 - node.y - (node.height ?? 80) / 2
        return Math.abs(horizontal) >= Math.abs(vertical) ? horizontal >= 0 ? 'right' : 'left' : vertical >= 0 ? 'bottom' : 'top'
      }
      const native = nativeChildren.find((entry) => entry.type === 'connector' && entry.id === `${nativePrefix}-e-${item.id}`)
      const points = native?.type === 'connector' ? native.routing?.points.map(([horizontal, vertical]) => [native.x + horizontal * native.width, native.y + vertical * native.height] as [number, number]) : undefined
      const label = nativeChildren.find((entry) => entry.type === 'text' && entry.id === `${nativePrefix}-et-${item.id}`)
      return { id: item.id, type: 'graphEdge', source: item.source, target: item.target, sourceHandle: item.source_port && item.source_port !== 'auto' ? item.source_port : automatic(source, target), targetHandle: item.target_port && item.target_port !== 'auto' ? item.target_port : automatic(target, source), label: item.label, data: { points, elbow: item.route === 'elbow', label, theme }, markerEnd: item.arrow === false ? undefined : { type: MarkerType.ArrowClosed, color: cssColor(item.color ?? '@dk2', theme) }, markerStart: item.start_arrow ? { type: MarkerType.ArrowClosed, color: cssColor(item.color ?? '@dk2', theme) } : undefined, style: { stroke: cssColor(item.color ?? '@dk2', theme), strokeDasharray: item.dashed ? '8 5' : undefined }, selected: selection.current.includes(item.id), ariaLabel: `Connection ${item.label || `${source.label} to ${target.label}`}` }
    }))
    setPreview(rendered ?? null)
  }, [resize, theme])
  useEffect(() => {
    const starting = current.current
    install(starting)
    let obsolete = false
    previewQueue = previewQueue.then(async () => {
      if (obsolete) return
      try {
        const rendered = await client.createGraph({ id: 'graph-preview', spec: starting, theme })
        if (!obsolete && current.current === starting) install(starting, rendered)
      } catch (reason) { if (!obsolete) setError(reason instanceof Error ? reason.message : String(reason)) }
    })
    return () => { obsolete = true }
  }, [install, theme])

  function remember(previous: GraphSpec) {
    setHistory((state) => ({ past: appendHistory(state.past, previous), future: [] }))
  }
  async function change(next: GraphSpec, operations?: GraphOperation[], record = true, propagateError = false) {
    if (gate.current) return false
    gate.current = true; setBusy(true); onBusy(true); setError('')
    try {
      await previewQueue
      const candidate = operations ? await client.transformGraph(next, operations) : next
      const rendered = await client.createGraph({ id: 'graph-preview', spec: candidate, theme })
      if (record && JSON.stringify(current.current) !== JSON.stringify(candidate)) remember(current.current)
      install(candidate, rendered)
      return true
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); install(current.current, preview ?? undefined); if (propagateError) throw reason; return false }
    finally { gate.current = false; setBusy(false); onBusy(false) }
  }
  function perform(operations: GraphOperation[]) { void change(current.current, operations) }
  function chooseIcon(target: IconTarget) {
    iconTrigger.current = menu?.anchor ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null)
    setMenu(null)
    void previewQueue.then(() => setIconTarget(structuredClone(target)))
  }
  function closeIconPicker() {
    const kind = iconTarget?.kind
    setIconTarget(null)
    requestAnimationFrame(() => {
      const fallback = kind === 'new' ? '.graph-add-service' : '.graph-node-assets button'
      const target = iconTrigger.current?.isConnected ? iconTrigger.current : editor.current?.querySelector<HTMLElement>(fallback)
      target?.focus()
    })
  }
  async function insertIcon(assets: AssetInput[], preparedIcon?: GraphIcon) {
    if (!iconTarget || assets.length !== 1) throw new Error('Choose one icon')
    await previewQueue
    const { base64, mime_type, alt } = assets[0]
    const icon = preparedIcon ?? await client.createGraphIcon({ base64, mime_type, alt })
    let operation: GraphOperation
    if (iconTarget.kind === 'group') operation = { op: 'put_group', group: { ...iconTarget.group, icon } }
    else if (iconTarget.kind === 'node') operation = { op: 'put_node', node: { ...iconTarget.node, icon } }
    else {
      const parent = selection.current.length === 1 ? current.current.groups?.find((group) => group.id === selection.current[0]) : undefined
      const label = (alt?.replace(/ \(Lucide\)$/, '').trim() || 'Service')
      if ([...label].length > 160) throw new Error('The service name exceeds 160 characters. Add an icon to a node with a shorter label.')
      operation = { op: 'put_node', node: { id: `node-${crypto.randomUUID().slice(0, 8)}`, label, presentation: 'icon', font_size: 16, ...serviceFrame(current.current, parent), ...(parent ? { group: parent.id } : {}), icon } }
    }
    if (await change(current.current, [operation], true, true)) {
      if (iconTarget.kind === 'new' && operation.op === 'put_node') choose(operation.node.id)
      closeIconPicker()
    }
  }
  async function addGroup() {
    const parent = selection.current.length === 1 ? current.current.groups?.find((group) => group.id === selection.current[0]) : undefined
    if (parent && groupAncestors(current.current.groups ?? [], parent.id).length >= 4) { setError('Boundaries support at most four levels.'); return }
    if (parent && (parent.width < 96 || parent.height < 104)) { setError('Enlarge the selected boundary before adding a child boundary.'); return }
    const group: GraphGroup = { id: `group-${crypto.randomUUID().slice(0, 8)}`, label: 'Boundary', ...(parent ? { parent: parent.id, x: parent.x + 16, y: parent.y + 48, width: parent.width - 32, height: parent.height - 64 } : { x: 24, y: 104, width: 520, height: 360 }) }
    if (await change(current.current, [{ op: 'put_group', group }])) choose(group.id)
  }
  useEffect(() => { action.current = perform })
  function choose(id: string) {
    selection.current = [id]; setSelected([id])
    setNodes((nodes) => nodes.map((node) => ({ ...node, selected: node.id === id })))
    setEdges((edges) => edges.map((edge) => ({ ...edge, selected: edge.id === id })))
  }
  const nodesChange = useCallback((changes: NodeChange<GraphFlowNode>[]) => setNodes((nodes) => applyNodeChanges(changes, nodes)), [])
  const edgesChange = useCallback((changes: EdgeChange<GraphFlowEdge>[]) => setEdges((edges) => applyEdgeChanges(changes, edges)), [])
  const selectionChange = useCallback(({ nodes, edges }: { nodes: GraphFlowNode[]; edges: GraphFlowEdge[] }) => { const ids = [...nodes, ...edges].map((entry) => entry.id); selection.current = ids; setSelected(ids) }, [])
  const dragStop = useCallback((_event: unknown, moved: GraphFlowNode, movedNodes: GraphFlowNode[]) => {
    const graph = current.current
    const node = graph.nodes.find((node) => node.id === moved.id)
    const item = node ?? graph.groups?.find((group) => group.id === moved.id)
    if (!item) return
    const groups = graph.groups ?? []
    const parentId = node ? node.group : (item as GraphGroup).parent
    const absolute = { ...moved.position }
    for (const id of groupAncestors(groups, parentId)) {
      const parent = groups.find((group) => group.id === id)!
      const ancestor = groups.find((group) => group.id === parent.parent)
      const position = movedNodes.find((entry) => entry.id === id)?.position ?? { x: parent.x - (ancestor?.x ?? 0), y: parent.y - (ancestor?.y ?? 0) }
      absolute.x += position.x; absolute.y += position.y
    }
    const dx = absolute.x - item.x
    const dy = absolute.y - item.y
    if (dx || dy) action.current([{ op: 'move', ids: selection.current.includes(moved.id) ? selection.current.filter((id) => graph.nodes.some((node) => node.id === id) || graph.groups?.some((group) => group.id === id)) : [moved.id], dx, dy }])
  }, [])
  const connect = useCallback((connection: Connection) => {
    if (connection.source && connection.target) action.current([{ op: 'put_edge', edge: { id: `edge-${crypto.randomUUID().slice(0, 8)}`, source: connection.source, target: connection.target, source_port: connection.sourceHandle as GraphEdge['source_port'] ?? 'auto', target_port: connection.targetHandle as GraphEdge['target_port'] ?? 'auto', route: 'elbow' } }])
  }, [])
  const reconnect = useCallback((old: GraphFlowEdge, connection: Connection) => {
    const edge = current.current.edges?.find((edge) => edge.id === old.id)
    if (edge && connection.source && connection.target) action.current([{ op: 'put_edge', edge: { ...edge, source: connection.source, target: connection.target, source_port: connection.sourceHandle as GraphEdge['source_port'] ?? 'auto', target_port: connection.targetHandle as GraphEdge['target_port'] ?? 'auto' } }])
  }, [])
  async function save() {
    if (gate.current) return
    gate.current = true; setBusy(true); onBusy(true); setError('')
    try {
      await previewQueue
      const candidate = view === 'json' ? JSON.parse(raw) : current.current
      await client.createGraph({ id: 'graph-preview', spec: candidate, theme })
      await onApply(candidate)
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { gate.current = false; setBusy(false); onBusy(false) }
  }
  async function restore(redo: boolean) {
    if (gate.current) return
    const source = redo ? history.future : history.past
    const next = source.at(-1)
    if (!next) return
    const previous = current.current
    if (await change(next, undefined, false)) setHistory((state) => redo
      ? { past: appendHistory(state.past, previous), future: state.future.slice(0, -1) }
      : { past: state.past.slice(0, -1), future: appendHistory(state.future, previous) })
  }
  async function switchView(mode: View) {
    if (view === mode || gate.current) return
    if (view === 'json') {
      try { if (!await change(JSON.parse(raw))) return }
      catch (reason) { setError(String(reason)); return }
    }
    if (mode === 'preview' && !await change(current.current, undefined, false)) return
    if (mode === 'json') setRaw(JSON.stringify(current.current, null, 2))
    setView(mode)
  }
  function finishMetadata() {
    const before = metadataBefore.current
    if (before && (before.title !== current.current.title || before.subtitle !== current.current.subtitle)) { remember(before); setSpec({ ...current.current }) }
    metadataBefore.current = null
  }
  const node = selected.length === 1 ? spec.nodes.find((node) => node.id === selected[0]) : undefined
  const edge = selected.length === 1 ? spec.edges?.find((edge) => edge.id === selected[0]) : undefined
  const group = selected.length === 1 ? spec.groups?.find((group) => group.id === selected[0]) : undefined
  const source = spec.nodes.some((node) => node.id === from) ? from : spec.nodes[0]?.id ?? ''
  const target = spec.nodes.some((node) => node.id === to) ? to : spec.nodes[1]?.id ?? spec.nodes[0]?.id ?? ''
  const locked = busy || view !== 'edit' || Boolean(iconTarget)
  const nested = Boolean(spec.groups?.some((group) => group.parent))
  const menuCommands: MenuCommand[] = [
    { label: 'Undo diagram edit', icon: Undo2, action: () => void restore(false), disabled: locked || !history.past.length },
    { label: 'Redo diagram edit', icon: Redo2, action: () => void restore(true), disabled: locked || !history.future.length },
    { label: 'Properties', icon: Eye, action: () => menu?.anchor?.closest('dialog')?.querySelector<HTMLElement>('.graph-inspector textarea, .graph-inspector input')?.focus(), disabled: !node && !edge && !group, separator: true },
    { label: node?.icon ? 'Change node icon' : 'Choose node icon', icon: Sticker, action: () => { if (node) chooseIcon({ kind: 'node', node }) }, disabled: locked || !node },
    { label: 'Remove node icon', icon: Trash2, action: () => { if (node) perform([{ op: 'put_node', node: { ...node, icon: null, ...(node.presentation === 'icon' ? { presentation: 'card' as const } : {}) } }]) }, disabled: locked || !node?.icon },
    { label: 'Duplicate node', icon: Copy, action: () => { if (node) perform([{ op: 'put_node', node: { ...node, id: `node-${crypto.randomUUID().slice(0, 8)}`, x: node.x + 24, y: node.y + 24 } }]) }, disabled: locked || !node },
    { label: 'Delete selection', icon: Trash2, action: () => perform([{ op: 'remove', ids: selected }]), disabled: locked || !selected.length, danger: true },
    ...(['left', 'center', 'right', 'top', 'middle', 'bottom'] as const).map((alignment, index) => ({ label: `Align ${alignment}`, icon: AlignLeft, action: () => perform([{ op: 'align', ids: selected, alignment }]), disabled: locked || selected.length < 2 || selected.some((id) => !spec.nodes.some((node) => node.id === id)), separator: index === 0 })),
    { label: 'Add rectangle node', icon: Square, action: () => perform([{ op: 'put_node', node: { id: `node-${crypto.randomUUID().slice(0, 8)}`, label: 'Rectangle', x: 64, y: 120 } }]), disabled: locked, separator: true },
    { label: 'Grid layout', icon: Grid2X2, action: () => perform([{ op: 'layout', columns: Math.min(4, Math.ceil(Math.sqrt(spec.nodes.length))) }]), disabled: locked || nested },
  ]
  function contextAt(target: HTMLElement, position: MenuPosition) {
    if (locked) return
    const entry = target.closest<HTMLElement>('.react-flow__node, .react-flow__edge')
    const id = entry?.dataset.id
    if (id) { if (!selected.includes(id)) choose(id) }
    else { selection.current = []; setSelected([]) }
    setMenu({ ...position, anchor: entry ?? position.anchor })
  }
  return <div ref={editor} className="graph-editor" aria-busy={busy} onKeyDownCapture={(event) => {
    if (iconTarget) { if (event.key === 'Escape' && !busy) { event.preventDefault(); event.stopPropagation(); closeIconPicker() } return }
    const element = event.target as HTMLElement
    if (locked || element.closest('input, textarea, select, [contenteditable=true]')) return
    if (event.key === 'ContextMenu' || event.shiftKey && event.key === 'F10') { event.preventDefault(); event.stopPropagation(); const bounds = element.getBoundingClientRect(); contextAt(element, { x: bounds.left + 12, y: bounds.top + 12, anchor: element }); return }
    if (element.closest('[role=menu]')) return
    if (!element.closest('.graph-canvas, .graph-entities')) return
    const movement = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[event.key]
    if (movement && element.closest('.react-flow__node')) {
      const ids = selection.current.filter((id) => spec.nodes.some((node) => node.id === id) || spec.groups?.some((group) => group.id === id))
      if (ids.length) { event.preventDefault(); event.stopPropagation(); const step = (snap ? 8 : 1) * (event.shiftKey ? 10 : 1); perform([{ op: 'move', ids, dx: movement[0] * step, dy: movement[1] * step }]) }
    } else if (event.key === 'Delete' && selected.length) { event.preventDefault(); event.stopPropagation(); perform([{ op: 'remove', ids: selected }]) }
  }}>
    <div className="graph-main" hidden={Boolean(iconTarget)}>
    <div className="graph-topbar">{(['title', 'subtitle'] as const).map((field) => <label key={field} className="field">{field === 'title' ? 'Title' : 'Subtitle'}<input aria-label={`Graph ${field}`} maxLength={field === 'title' ? 80 : 120} value={spec[field] ?? ''} disabled={locked} onFocus={() => { metadataBefore.current = current.current }} onBlur={finishMetadata} onChange={(event) => { const next = { ...current.current, [field]: event.target.value }; current.current = next; setSpec(next); setPreview(null) }} /></label>)}</div>
    <div className="graph-toolbar" aria-label="Diagram tools">
      <GraphTool label="Undo diagram edit" Icon={Undo2} onClick={() => void restore(false)} disabled={locked || !history.past.length} /><GraphTool label="Redo diagram edit" Icon={Redo2} onClick={() => void restore(true)} disabled={locked || !history.future.length} />
      <GraphTool label="Delete graph selection" Icon={Trash2} onClick={() => perform([{ op: 'remove', ids: selected }])} disabled={locked || !selected.length} />
      <GraphTool label="Duplicate node" Icon={Copy} onClick={() => { if (node) perform([{ op: 'put_node', node: { ...node, id: `node-${crypto.randomUUID().slice(0, 8)}`, x: node.x + 24, y: node.y + 24 } }]) }} disabled={locked || !node} />
      {([['left', AlignLeft], ['center', AlignCenterHorizontal], ['right', AlignRight], ['top', AlignStartVertical], ['middle', AlignCenterVertical], ['bottom', AlignEndVertical]] as const).map(([alignment, Icon]) => <GraphTool key={alignment} label={`Align ${alignment}`} Icon={Icon} onClick={() => perform([{ op: 'align', ids: selected, alignment }])} disabled={locked || selected.length < 2 || selected.some((id) => !spec.nodes.some((node) => node.id === id))} />)}
      <span className="graph-layout-control" title={nested ? 'Grid layout is unavailable for nested boundaries.' : undefined}><GraphTool label="Grid layout" Icon={Grid2X2} onClick={() => perform([{ op: 'layout', columns: Math.min(4, Math.ceil(Math.sqrt(spec.nodes.length))) }])} disabled={locked || nested} />{nested && <span className="visually-hidden">Grid layout is unavailable for nested boundaries.</span>}</span>
      <label className="checkbox"><input type="checkbox" checked={snap} disabled={locked} onChange={(event) => setSnap(event.target.checked)} />Snap</label>
      <div className="graph-views" role="tablist" aria-label="Diagram views">{([['edit', MousePointer2, 'Canvas'], ['preview', Eye, 'Preview'], ['json', Code2, 'JSON']] as const).map(([mode, Icon, label], index, modes) => <button type="button" key={mode} id={`graph-tab-${mode}`} role="tab" aria-selected={view === mode} aria-controls={`graph-panel-${mode}`} tabIndex={view === mode ? 0 : -1} disabled={busy} onClick={() => void switchView(mode)} onKeyDown={(event) => { const offset = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0; if (offset) { event.preventDefault(); const next = modes[(index + offset + modes.length) % modes.length][0]; void switchView(next).then(() => document.getElementById(`graph-tab-${next}`)?.focus()) } }}><Icon size={16} />{label}</button>)}</div>
    </div>
    <div className="graph-workspace">
      <fieldset className="graph-library" disabled={locked}><legend className="visually-hidden">Graph library</legend><label className="field">Example<select aria-label="Graph example" defaultValue="" onChange={(event) => { const example = catalog.examples.find((entry) => entry.id === event.target.value); if (example) void change(structuredClone(example.spec)) }}><option value="" disabled>Choose example</option>{catalog.examples.map((entry) => <option key={entry.id} value={entry.id}>{entry.name}</option>)}</select></label>
        <button type="button" className="secondary graph-add-service" aria-label="Add service icon" onClick={() => chooseIcon({ kind: 'new' })}><Sticker size={20} /><span>Add service icon</span></button>
        <div className="graph-shapes">{Object.entries(kinds).map(([kind, entry]) => <button type="button" key={kind} aria-label={`Add ${entry.name.toLowerCase()} node`} title={`Add ${entry.name}`} onClick={() => perform([{ op: 'put_node', node: { id: `node-${crypto.randomUUID().slice(0, 8)}`, label: entry.name, kind: kind as GraphNodeKind, x: 64 + spec.nodes.length % 4 * 240, y: 120 + Math.floor(spec.nodes.length / 4) % 3 * 112 } }])}><entry.icon size={22} /><span>{entry.name}</span></button>)}</div>
        <button type="button" className="secondary" onClick={() => void addGroup()}><Plus size={16} />Boundary</button>
        <div className="graph-entities">{spec.groups?.map((entry) => <button key={entry.id} type="button" className={selected.includes(entry.id) ? 'active' : ''} aria-label={`Select group ${entry.id}`} onClick={() => choose(entry.id)}>{entry.label}</button>)}{spec.nodes.map((entry) => <button type="button" key={entry.id} className={selected.includes(entry.id) ? 'active' : ''} aria-label={`Select node ${entry.id}`} onClick={() => choose(entry.id)}>{entry.label}</button>)}{spec.edges?.map((entry) => <button type="button" key={entry.id} className={selected.includes(entry.id) ? 'active' : ''} aria-label={`Select edge ${entry.id}`} onClick={() => choose(entry.id)}>{entry.label || `${entry.source} to ${entry.target}`}</button>)}</div>
      </fieldset>
      <section className="graph-canvas" role="tabpanel" id={`graph-panel-${view}`} aria-labelledby={`graph-tab-${view}`} onContextMenu={(event) => { if (locked) return; event.preventDefault(); event.stopPropagation(); contextAt(event.target as HTMLElement, { x: event.clientX, y: event.clientY, anchor: event.currentTarget }) }}>
        {view === 'edit' && <ReactFlow<GraphFlowNode, GraphFlowEdge> nodes={nodes} edges={edges} nodeTypes={nodeTypes} edgeTypes={edgeTypes} onNodesChange={nodesChange} onEdgesChange={edgesChange} onSelectionChange={selectionChange} onNodeDragStop={dragStop} onConnect={connect} onReconnect={reconnect} fitView fitViewOptions={{ padding: 0.2 }} minZoom={0.25} maxZoom={2.5} nodeExtent={[[0, 88], [1152, 512]]} connectionMode={ConnectionMode.Loose} snapToGrid={snap} snapGrid={[8, 8]} deleteKeyCode={null} nodesDraggable={!busy} nodesConnectable={!busy} edgesReconnectable={!busy} elementsSelectable={!busy} colorMode="light"><Background gap={24} size={1} /><Controls showInteractive={false} /></ReactFlow>}
        {view === 'preview' && (preview ? <NativePreview element={preview} theme={theme} /> : <p role="status">Rendering diagram</p>)}
        {view === 'json' && <div className="graph-json"><label className="field">Graph JSON<textarea aria-label="Graph JSON" className="code-input" maxLength={2 * 1024 * 1024} rows={20} disabled={busy} value={raw} onChange={(event) => setRaw(event.target.value)} /></label><button type="button" className="secondary" disabled={busy} onClick={() => { try { void change(JSON.parse(raw)) } catch (reason) { setError(String(reason)) } }}><Check size={16} />Validate JSON</button></div>}
      </section>
      <fieldset className="graph-inspector" disabled={locked}><legend className="visually-hidden">Graph properties</legend>{node ? <NodeProperties key={JSON.stringify(node)} node={node} groups={spec.groups ?? []} theme={theme} onApply={(node) => perform([{ op: 'put_node', node }])} onChooseIcon={(node) => chooseIcon({ kind: 'node', node })} /> : edge ? <EdgeProperties key={JSON.stringify(edge)} edge={edge} nodes={spec.nodes} theme={theme} onApply={(edge) => perform([{ op: 'put_edge', edge }])} /> : group ? <GroupProperties key={JSON.stringify(group)} group={group} groups={spec.groups ?? []} theme={theme} onApply={(group) => perform([{ op: 'put_group', group }])} onChooseIcon={(group) => chooseIcon({ kind: 'group', group })} /> : <div className="graph-empty">{selected.length ? `${selected.length} selected` : 'No selection'}</div>}
        <div className="graph-new-edge"><label className="field">From<select aria-label="Connect from" value={source} onChange={(event) => setFrom(event.target.value)}>{spec.nodes.map((node) => <option value={node.id} key={node.id}>{node.label}</option>)}</select></label><label className="field">To<select aria-label="Connect to" value={target} onChange={(event) => setTo(event.target.value)}>{spec.nodes.map((node) => <option value={node.id} key={node.id}>{node.label}</option>)}</select></label><button type="button" className="secondary" disabled={source === target} onClick={() => perform([{ op: 'put_edge', edge: { id: `edge-${crypto.randomUUID().slice(0, 8)}`, source, target, route: 'elbow' } }])}><Plus size={16} />Connect nodes</button></div>
      </fieldset>
    </div>
    {error && <p className="error graph-error" role="alert">{error}</p>}
    <footer className="graph-actions"><span role="status">{busy ? 'Validating diagram' : `${spec.nodes.length} nodes / ${spec.edges?.length ?? 0} connections`}</span><button type="button" className="primary" disabled={busy} onClick={() => void save()}><Check size={18} />{editing ? 'Update graph' : 'Insert graph'}</button></footer>
    {menu && <ContextMenu position={menu} label="Diagram actions" commands={menuCommands} onClose={closeMenu} />}
    </div>
    {iconTarget && <section className="graph-icon-picker" aria-label={iconTarget.kind === 'new' ? 'New service icon' : iconTarget.kind === 'group' ? 'Boundary icon' : 'Node icon'}><header><Tool label="Cancel icon selection" disabled={busy} onClick={closeIconPicker}><ArrowLeft size={20} /></Tool><h3>{iconTarget.kind === 'new' ? 'New service icon' : iconTarget.kind === 'group' ? iconTarget.group.label : iconTarget.node.label}</h3></header><Suspense fallback={<p role="status">Loading icons</p>}><AssetPanel maxFiles={1} showSize={false} onBusy={(value) => { setBusy(value); onBusy(value) }} onInsert={insertIcon} /></Suspense></section>}
  </div>
}