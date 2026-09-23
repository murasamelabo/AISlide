import { useRef, useState } from 'react'
import { AlignStartVertical, AlignCenterVertical, AlignEndVertical, AlignStartHorizontal, AlignCenterHorizontal, AlignEndHorizontal, AlignHorizontalDistributeCenter, AlignVerticalDistributeCenter, Group, Ungroup, Copy, Scissors, ClipboardPaste, Combine, Layers2, Minus, Diff, Paintbrush, ClipboardCopy } from 'lucide-react'
import type { Element, SelectionOperation, CombineShapesInput } from './types'
import { Tool } from './Tool'
import { ContextMenu } from './ContextMenu'
import type { MenuPosition } from './ContextMenu'
import { visualOf } from './visual-render'
import './selection.css'

function selectionBounds(elements: Element[]) {
  if (!elements.length) return null
  const x = Math.min(...elements.map((element) => element.x))
  const y = Math.min(...elements.map((element) => element.y))
  return { x, y, width: Math.max(...elements.map((element) => element.x + element.width)) - x, height: Math.max(...elements.map((element) => element.y + element.height)) - y }
}

export function SelectionTools({ elements, disabled, canPaste, onCommand, onCombine, canApplyFormat = false, onCopyFormat, onApplyFormat }: { elements: Element[]; disabled: boolean; canPaste: boolean; onCommand: (operation: SelectionOperation) => void; onCombine?: (input: CombineShapesInput) => void; canApplyFormat?: boolean; onCopyFormat?: (paragraphIndex: number, runIndex: number) => void; onApplyFormat?: () => void }) {
  const [formatSource, setFormatSource] = useState({ id: '', paragraph: 0, run: 0 })
  const formatElement = elements.length === 1 && !['group', 'connector'].includes(elements[0].type) ? elements[0] : null
  const formatParagraphs = formatElement?.type === 'text' || formatElement?.type === 'shape' ? formatElement.format?.paragraphs ?? [] : []
  const formatParagraph = formatSource.id === formatElement?.id && formatSource.paragraph < Math.max(1, formatParagraphs.length) ? formatSource.paragraph : 0
  const formatRun = formatSource.id === formatElement?.id && formatSource.run < Math.max(1, formatParagraphs[formatParagraph]?.runs.length ?? 1) ? formatSource.run : 0
  const [relative, setRelative] = useState<'selection' | 'page'>('selection')
  const combinationTool = useRef<HTMLDivElement>(null)
  const [combinationMenu, setCombinationMenu] = useState<MenuPosition | null>(null)
  const ids = elements.map((element) => element.id)
  const locked = elements.some((element) => visualOf(element)?.locked || visualOf(element)?.hidden)
  const blocked = disabled || locked || !ids.length
  const combinationBlocked = blocked || !onCombine || ids.length < 2 || ids.length > 32 || elements.some((element) => element.type !== 'rect' && element.type !== 'polygon' && !(element.type === 'shape' && element.preset === 'rect' && !element.rotation && !element.text))
  if (combinationBlocked && combinationMenu) setCombinationMenu(null)
  function closeCombinationMenu() {
    setCombinationMenu(null)
    requestAnimationFrame(() => combinationTool.current?.querySelector('button')?.focus())
  }
  const bounds = selectionBounds(elements)
  return <div className="selection-tools" role="group" aria-label="Selection tools" onFocusCapture={(event) => { if (event.currentTarget.contains(event.target)) event.target.scrollIntoView({ block: 'nearest', inline: 'nearest' }) }}>
    <div className="selection-tool-group" role="group" aria-label="Clipboard">
      <Tool label="Copy selection" disabled={disabled || !ids.length} onClick={() => onCommand({ op: 'copy', ids, format: 'keep_source_formatting' })}><Copy /></Tool>
      <Tool label="Cut selection" disabled={blocked} onClick={() => onCommand({ op: 'cut', ids, format: 'keep_source_formatting' })}><Scissors /></Tool>
      <Tool label="Paste selection" disabled={disabled || !canPaste} onClick={() => onCommand({ op: 'paste', id_prefix: `paste-${crypto.randomUUID().slice(0, 8)}`, dx: 24, dy: 24 })}><ClipboardPaste /></Tool>
    </div>
    <select aria-label="Align relative to" disabled={disabled} value={relative} onChange={(event) => setRelative(event.target.value as typeof relative)}><option value="selection">Selection</option><option value="page">Page</option></select>
    <div className="selection-tool-group" role="group" aria-label="Format painter">
      <Tool label="Copy format" disabled={disabled || !formatElement || !onCopyFormat} onClick={() => onCopyFormat?.(formatParagraph, formatRun)}><ClipboardCopy /></Tool>
      <Tool label="Apply format" disabled={blocked || !canApplyFormat || !onApplyFormat || elements.some(element => ['group', 'connector'].includes(element.type))} onClick={() => onApplyFormat?.()}><Paintbrush /></Tool>
      {formatParagraphs.length > 1 && <select aria-label="Format source paragraph" disabled={disabled} value={formatParagraph} onChange={event => setFormatSource({ id: formatElement!.id, paragraph: Number(event.target.value), run: 0 })}>{formatParagraphs.map((_, index) => <option key={index} value={index}>Paragraph {index + 1}</option>)}</select>}
      {(formatParagraphs[formatParagraph]?.runs.length ?? 0) > 1 && <select aria-label="Format source run" disabled={disabled} value={formatRun} onChange={event => setFormatSource({ id: formatElement!.id, paragraph: formatParagraph, run: Number(event.target.value) })}>{formatParagraphs[formatParagraph].runs.map((_, index) => <option key={index} value={index}>Run {index + 1}</option>)}</select>}
    </div>
    <div className="selection-tool-group" role="group" aria-label="Align objects">{([['left', AlignStartVertical], ['center', AlignCenterVertical], ['right', AlignEndVertical], ['top', AlignStartHorizontal], ['middle', AlignCenterHorizontal], ['bottom', AlignEndHorizontal]] as const).map(([alignment, Icon]) => <Tool key={alignment} label={`Align objects ${alignment}`} disabled={blocked || relative === 'selection' && ids.length < 2} onClick={() => onCommand({ op: 'align', ids, alignment, relative_to: relative })}><Icon /></Tool>)}</div>
    <div className="selection-tool-group" role="group" aria-label="Distribute and group"><Tool label="Distribute horizontally" disabled={blocked || ids.length < 3} onClick={() => onCommand({ op: 'distribute', ids, axis: 'horizontal', relative_to: relative })}><AlignHorizontalDistributeCenter /></Tool><Tool label="Distribute vertically" disabled={blocked || ids.length < 3} onClick={() => onCommand({ op: 'distribute', ids, axis: 'vertical', relative_to: relative })}><AlignVerticalDistributeCenter /></Tool><Tool label="Group selection" disabled={blocked || ids.length < 2} onClick={() => onCommand({ op: 'group', ids, group_id: `group-${crypto.randomUUID().slice(0, 8)}` })}><Group /></Tool><Tool label="Ungroup selection" disabled={blocked || elements.some((element) => element.type !== 'group')} onClick={() => onCommand({ op: 'ungroup', ids })}><Ungroup /></Tool></div>
    <div ref={combinationTool} className="selection-tool-group">
      <Tool label="Combine shapes" disabled={combinationBlocked || Boolean(combinationMenu)} pressed={Boolean(combinationMenu)} onClick={() => {
        const anchor = combinationTool.current?.querySelector('button')
        if (!anchor) return
        const bounds = anchor.getBoundingClientRect()
        setCombinationMenu(combinationMenu ? null : { x: bounds.left, y: bounds.bottom, anchor })
      }}><Combine /></Tool>
      {combinationMenu && !combinationBlocked && <ContextMenu position={combinationMenu} label="Combine shapes" onClose={closeCombinationMenu} commands={([
        ['union', 'Union shapes', Combine], ['intersect', 'Intersect shapes', Layers2], ['subtract', 'Subtract shapes', Minus], ['xor', 'Exclude overlap (XOR)', Diff], ['fragment', 'Fragment shapes', Layers2],
      ] as const).map(([operation, label, icon]) => ({ label, icon, action: () => onCombine?.({ ids, operation, result_id: `combined-${crypto.randomUUID().slice(0, 8)}` }) }))} />}
    </div>
    <output aria-label="Selection bounds">{bounds ? `${ids.length} selected: ${Math.round(bounds.x)}, ${Math.round(bounds.y)} / ${Math.round(bounds.width)} x ${Math.round(bounds.height)}` : 'No selection'}</output>
  </div>
}